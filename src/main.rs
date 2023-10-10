use axum::{
    body::StreamBody,
    extract::{Extension, Path, Query, State},
    headers::{ContentType, ETag, HeaderMapExt},
    http::{header::CONTENT_DISPOSITION, HeaderMap, Request, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Router,
};
use clap::Parser;
use language_tag::Tag;
use serde::Deserialize;
use std::{
    collections::HashMap,
    io,
    net::SocketAddr,
    path, str,
    sync::Arc,
};
use tokio::{fs, task};
use tower_http::{compression::CompressionLayer, trace::TraceLayer};

mod config;
mod etag;
mod ldml;
mod toggle;
mod unique_id;

/*
/<ws_id>                => /<ws_id> [Accept:application/x.vnd.sil.ldml.v2+xml]
    [ext=<type>]        => [Accept: application/vnd.sil.ldml.v2+<type>...]
    [flatten=<bool>]    => [flatten=<bool>]
    [inc=<top>[,..]]    => [inc=<top>[,..]]
    [revid=<etag>]      => [If-Not-Match: <etag>][Accept: application/vnd.sil.ldml.v2+<type>...]
    [uid=<uuid>]        => [uid=<uuid>]
    [staging=<bool>]    => [Accept: application/vnd.sil.ldml.v2+<type>+staging,...]
/?query=langtags[&ext=<type>]           => /langtags [Accept: application/vnd.sil.ldml.v2+<type>...]
/<ws_id>?query=tags[&ext=<type>]        => /tagset/<ws_id> [Accept: application/vnd.sil.ldml.v2+txt]
/?ws_id=<ws_id>                         => /<ws_id> [Accept:application/x.vnd.sil.ldml.v2+xml]
*/

use config::{Config, Profiles};
use langtags::json::LangTags;
use toggle::Toggle;
use unique_id::UniqueID;

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Args {
    #[clap(long, default_value = "./ldml-api.json")]
    /// Path to config file
    config: path::PathBuf,

    #[clap(long, default_value = "production")]
    /// Default profile to use when staging argument not set in a request
    profile: String,

    #[clap(short, long, default_value = "0.0.0.0:3000")]
    listen: SocketAddr,
}

#[tokio::main()]
async fn main() -> io::Result<()> {
    //console_subscriber::init();
    // Set the RUST_LOG, if it hasn't been explicitly defined
    #[cfg(debug_assertions)]
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "ldml_api=debug,tower_http=debug")
    }
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    // Load configuraion
    let cfg = config::profiles::from(&args.config, &args.profile)?;
    tracing::info!(
        "loaded profiles: {profiles:?}",
        profiles = cfg.keys().collect::<Vec<_>>()
    );

    // run it with hyper on localhost:3000
    tracing::info!("listening on {addr}", addr = args.listen);
    axum::Server::bind(&args.listen)
        .serve(
            app(cfg)?
                .layer(CompressionLayer::new())
                .layer(TraceLayer::new_for_http())
                .into_make_service(),
        )
        .await
        .unwrap();
    Ok(())
}

fn app(cfg: Profiles) -> io::Result<Router> {
    Ok(Router::new()
        .route("/langtags.:ext", get(langtags))
        .route(
            "/:ws_id",
            get(demux_writing_system)
                .layer(middleware::from_fn(etag::layer))
                .layer(middleware::from_fn(etag::revid::converter)),
        )
        .layer(middleware::from_fn_with_state(cfg.into(), profile_selector))
        .route("/", get(query_only))
        .route("/index.html", get(static_help))
        .fallback(static_help))
}

async fn static_help() -> impl IntoResponse {
    Html(include_str!("index.html"))
}

async fn profile_selector<B>(
    State(profiles): State<Box<Profiles>>,
    mut req: Request<B>,
    next: Next<B>,
) -> Response {
    let staging = req
        .uri()
        .query()
        .and_then(|q| serde_urlencoded::from_str::<HashMap<String, Toggle>>(q).ok())
        .and_then(|qs| qs.get("staging").map(|t| **t))
        .unwrap_or(false);
    let config = if staging {
        profiles["staging"].clone()
    } else {
        profiles[""].clone()
    };

    req.extensions_mut().insert(config);
    next.run(req).await
}

async fn stream_file(path: &path::Path) -> Result<impl IntoResponse, Response> {
    let attachment: &path::Path = path
        .file_name()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, String::default()).into_response())?
        .as_ref();
    stream_file_as(path, attachment).await
}

async fn stream_file_as(
    path: &path::Path,
    filename: &path::Path,
) -> Result<impl IntoResponse, Response> {
    let mime = mime_guess::from_path(filename).first_or_octet_stream();
    let disposition = format!(
        "attachment; filename=\"{name}\"",
        name = filename.to_string_lossy()
    )
    .parse()
    .expect("failed to parse Content-Disposition header value");
    let mut headers = HeaderMap::new();
    headers.typed_insert(ContentType::from(mime));
    headers.insert(CONTENT_DISPOSITION, disposition);
    let file = fs::File::open(path).await.map_err(|err| {
        (
            StatusCode::NOT_FOUND,
            format!(
                "Cannot open: {err}: {}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ),
        )
            .into_response()
    })?;
    if let Some(etag) = etag::from_metadata(path) {
        headers.typed_insert(etag);
    }
    let stream = tokio_util::io::ReaderStream::with_capacity(file, 1 << 14); // 16KiB buffer
    let body = StreamBody::new(stream);

    Ok((headers, body))
}

async fn langtags(
    Path(ext): Path<String>,
    Extension(cfg): Extension<Arc<Config>>,
) -> impl IntoResponse {
    tracing::debug!("langtags.{ext}");
    stream_file(&cfg.langtags_dir.join("langtags").with_extension(ext)).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LDMLQuery {
    AllTags,
    LangTags,
    Tags,
}

#[derive(Deserialize)]
struct QueryParams {
    _ws_id: Option<Tag>,
    query: Option<LDMLQuery>,
    ext: Option<String>,
}

async fn query_only(Query(params): Query<QueryParams>) -> impl IntoResponse {
    match params.query {
        Some(LDMLQuery::AllTags) => Err((
            StatusCode::NOT_FOUND,
            "LDML SERVER ERROR: The alltags file is obsolete. Please use 'query=langtags'.",
        )),
        Some(LDMLQuery::LangTags) => {
            let ext = params.ext.as_deref().unwrap_or("txt");
            Ok(Redirect::permanent(&format!("/langtags.{ext}")))
        }
        Some(LDMLQuery::Tags) => Err((
            StatusCode::BAD_REQUEST,
            "LDML SERVER ERROR: query=tags requires a ws_id",
        )),
        None => Ok(Redirect::temporary("/index.html")),
    }
}

#[derive(Debug, Deserialize)]
struct WSParams {
    query: Option<LDMLQuery>,
    ext: Option<String>,
    flatten: Option<Toggle>,
    #[serde(rename = "inc[]")]
    inc: Option<String>,
    uid: Option<UniqueID>,
}

async fn writing_system_tags(ws: &Tag, cfg: &Config) -> impl IntoResponse {
    query_tags(ws, &cfg.langtags).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("No tagsets found for tag: {ws}"),
        )
    })
}

async fn fetch_writing_system_ldml(ws: &Tag, params: WSParams, cfg: &Config) -> impl IntoResponse {
    let ext = params.ext.as_deref().unwrap_or("xml");
    let flatten = *params.flatten.unwrap_or(Toggle::ON);

    tracing::debug!("find writing system with {params:?}");
    let path = find_ldml_file(ws, &cfg.sldr_path(flatten), &cfg.langtags)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("No LDML for {ws}")).into_response())?;
    let etag = etag::revid::from_ldml(&path).or_else(|| etag::from_metadata(&path));
    let mut headers = HeaderMap::new();

    if let Some(tag) = etag {
        headers.typed_insert(tag);
    }
    if params.inc.is_none() && params.uid.is_none() {
        stream_file_as(
            path.as_ref(),
            path.with_extension(ext)
                .file_name()
                .ok_or_else(|| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Error generating attachment filename",
                    )
                        .into_response()
                })?
                .as_ref(),
        )
        .await
        .map(IntoResponse::into_response)
    } else {
        if let Some(etag) = headers.typed_get::<ETag>() {
            headers.typed_insert(etag::weaken(etag))
        }
        ldml_customisation(path.as_ref(), params.inc, params.uid)
            .await
            .map(IntoResponse::into_response)
    }
    .map(|resp| (headers, resp))
}

async fn demux_writing_system(
    Path(ws): Path<Tag>,
    Query(params): Query<WSParams>,
    Extension(cfg): Extension<Arc<Config>>,
) -> impl IntoResponse {
    tracing::debug!("language tag {ws:?}");
    if let Some(query) = params.query {
        match query {
            LDMLQuery::AllTags | LDMLQuery::LangTags => (
                StatusCode::BAD_REQUEST,
                "query=alltags is only valid without a ws_id.",
            )
                .into_response(),
            LDMLQuery::Tags => writing_system_tags(&ws, &cfg).await.into_response(),
        }
    } else {
        fetch_writing_system_ldml(&ws, params, &cfg)
            .await
            .into_response()
    }
}

fn query_tags(ws: &Tag, langtags: &LangTags) -> Option<String> {
    let predicate: Box<dyn Fn(&Tag) -> bool> = match ws {
        Tag {
            script: None,
            region: Some(_),
            ..
        } => Box::new(|t| t.lang == ws.lang && t.region == ws.region),
        Tag {
            script: Some(_), ..
        } => Box::new(|t| t.lang == ws.lang && t.script == ws.script),
        _ => Box::new(|t| t.lang == ws.lang),
    };
    langtags
        .tagsets()
        .filter_map(|ts| {
            if ts.iter().any(&predicate) {
                Some(ts.to_string() + "\n")
            } else {
                None
            }
        })
        .reduce(|accum, item| accum + &item)
}

fn find_ldml_file(ws: &Tag, sldr_dir: &path::Path, langtags: &LangTags) -> Option<path::PathBuf> {
    // Lookup the tag set and generate a prefered sorted list.
    let mut tagset: Vec<_> = langtags.get(ws)?.iter().collect();
    tagset.sort();
    tagset.push(ws);

    tagset
        .iter()
        .map(|&tag| {
            let mut path = path::PathBuf::from(sldr_dir);
            path.push(&tag.lang[0..1]);
            path.push(tag.to_string().replace("-", "_"));
            path.with_extension("xml")
        })
        .rfind(|path| path.exists())
}

async fn ldml_customisation(
    path: &path::Path,
    xpaths: Option<String>,
    uid: Option<UniqueID>,
) -> Result<impl IntoResponse, Response> {
    task::block_in_place(|| {
        let mut doc = ldml::Document::new(path)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
        if let Some(xpaths) = xpaths {
            let xpaths = xpaths.split(',').collect::<Vec<_>>();
            doc.subset(&xpaths)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
        }
        if let Some(uid) = uid {
            doc.set_uid(*uid)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
        }
        Ok(doc.to_string())
    })
}

#[cfg(test)]
mod test {
    use crate::find_ldml_file;

    use super::{app, config, Profiles, Tag};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        Router,
    };
    use std::str::FromStr;
    use std::{
        fs::File,
        io::{BufRead, BufReader},
    };
    use tower::{Service, ServiceExt}; // for `oneshot` and `ready`

    fn get_profiles() -> &'static Profiles {
        use std::sync::OnceLock;
        static SHARED_PROFILES: OnceLock<Profiles> = OnceLock::new();
        SHARED_PROFILES.get_or_init(|| {
            config::profiles::from("./ldml-api.json", "production").expect("test config")
        })
    }

    fn get_app() -> Router {
        app(get_profiles().clone()).expect("Router")
    }

    #[tokio::test]
    async fn index_page() {
        let mut app = get_app();

        let response = app
            .call(
                Request::builder()
                    .uri("/index.html")
                    .body(Body::empty())
                    .expect("Request"),
            )
            .await
            .expect("Response");

        let fallback_response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .expect("Request"),
            )
            .await
            .expect("Response");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(fallback_response.status(), StatusCode::TEMPORARY_REDIRECT);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(&body[..], include_str!("index.html").as_bytes());
    }

    async fn request_ldml_file(tag: &Tag) -> StatusCode {
        let app = get_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/{tag}"))
                    .body(Body::empty())
                    .expect("Request"),
            )
            .await
            .expect("Response");

        response.status()
    }


    #[tokio::test]
    async fn simple_writing_system_request() {
        assert_eq!(
            request_ldml_file(&Tag::from_str("en-US").expect("Tag")).await,
            StatusCode::OK
        );
        assert_eq!(
            request_ldml_file(&Tag::from_str("en-KP").expect("Tag")).await,
            StatusCode::NOT_FOUND
        );
    }

}
