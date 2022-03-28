use axum::{
    body::StreamBody,
    extract::{Extension, Path, Query},
    headers::HeaderMapExt,
    http::{header, HeaderMap, HeaderValue, Request, StatusCode},
    middleware::{self, Next},
    response::{Headers, Html, IntoResponse, Redirect, Response},
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
use tokio::fs;
use tower_http::trace::TraceLayer;

mod config;
mod etag;
mod langtags;
mod toggle;

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

use crate::config::{Config, Profiles};
use crate::langtags::LangTags;
use crate::toggle::Toggle;

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

#[tokio::main]
async fn main() -> io::Result<()> {
    //console_subscriber::init();
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "ldml_api=debug,tower_http=debug")
    }
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    let default_profile = Arc::new(args.profile);

    // Load configuraion
    let cfg = config::profiles::from(args.config)?;
    tracing::info!(
        "loaded profiles: {profiles:?}",
        profiles = cfg.keys().collect::<Vec<_>>()
    );

    async fn static_help() -> Html<&'static str> {
        Html(include_str!("index.html"))
    }
    let app = Router::new()
        .route("/langtags.:ext", get(langtags))
        .route(
            "/:ws_id",
            get(demux_writing_system)
                .layer(middleware::from_fn(etag::layer))
                .layer(middleware::from_fn(etag::revid::converter)),
        )
        .route("/", get(query_only))
        .route("/index.html", get(static_help))
        .layer(middleware::from_fn(move |req, next| {
            profile_selector(req, next, cfg.clone(), default_profile.clone())
        }))
        .layer(TraceLayer::new_for_http());

    // run it with hyper on localhost:3000
    // let addr = "0.0.0.0:3000".parse().expect("localhost listening address");
    tracing::info!("listening on {addr}", addr = args.listen);
    axum::Server::bind(&args.listen)
        .serve(app.into_make_service())
        .await
        .unwrap();
    Ok(())
}

async fn profile_selector<B>(
    mut req: Request<B>,
    next: Next<B>,
    cfg: Arc<Profiles>,
    default_profile: Arc<String>,
) -> Response {
    let staging = req
        .uri()
        .query()
        .and_then(|q| serde_urlencoded::from_str::<HashMap<String, Toggle>>(q).ok())
        .and_then(|qs| qs.get("staging").map(|t| **t))
        .unwrap_or(false);
    let profile = if staging {
        "staging"
    } else {
        default_profile.as_str()
    };
    tracing::debug!("using profile: {profile}");
    req.extensions_mut().insert(cfg[profile].clone());
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
    let guess = mime_guess::from_path(filename);
    let mime = guess.first_or_octet_stream();
    let headers = Headers([
        (
            header::CONTENT_TYPE,
            HeaderValue::from_str(mime.as_ref()).expect("failed to parse mimetype"),
        ),
        (
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!(
                "attachment; filename=\"{name}\"",
                name = filename.to_string_lossy()
            ))
            .expect("failed to parse Content-Disposition header value"),
        ),
    ]);
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

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum LDMLQuery {
    AllTags,
    LangTags,
}

#[derive(Deserialize)]
struct QueryParams {
    query: LDMLQuery,
    ext: Option<String>,
}

async fn query_only(Query(params): Query<QueryParams>) -> impl IntoResponse {
    match params.query {
        LDMLQuery::AllTags => Err((
            StatusCode::NOT_FOUND,
            "LDML SERVER ERROR: The alltags file is obsolete. Please use 'query=langtags'.",
        )),
        LDMLQuery::LangTags => {
            let ext = params.ext.as_deref().unwrap_or("txt");
            Ok(Redirect::permanent(
                format!("/langtags.{ext}")
                    .parse()
                    .expect("langtags relative path"),
            ))
        }
    }
}

#[derive(Deserialize)]
struct WSParams {
    query: Option<String>,
    ext: Option<String>,
    flatten: Option<Toggle>,
    inc: Option<String>,
    revid: Option<String>,
    uid: Option<u32>,
}

async fn writing_system_tags(ws: &Tag, cfg: &Config) -> impl IntoResponse {
    query_tags(&ws, &cfg.langtags)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Invalid language tag: {ws}")))
}

async fn fetch_wirting_system_ldml(ws: &Tag, params: &WSParams, cfg: &Config) -> impl IntoResponse {
    let ext = params.ext.as_deref().unwrap_or("xml");
    let flatten = *params.flatten.unwrap_or(Toggle::ON);
    let _xpath = params.inc.as_deref().unwrap_or_default();
    let _revid = params.revid.as_deref().unwrap_or_default();
    let _uid = params.uid.unwrap_or_default();

    let path = find_ldml_file(&ws, &cfg.sldr_path(flatten), &cfg.langtags)
    .ok_or_else(|| (StatusCode::NOT_FOUND, format!("No LDML for {ws}")).into_response())?;
    let etag = etag::revid::from_ldml(&path)
    .or_else(|| etag::from_metadata(&path));
    let mut headers = HeaderMap::new();
    
    if let Some(tag) = etag {
        headers.typed_insert(tag);
    }
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
    .map(|resp| (headers, resp))
}

async fn demux_writing_system(
    Path(ws): Path<Tag>,
    Query(params): Query<WSParams>,
    Extension(cfg): Extension<Arc<Config>>,
) -> impl IntoResponse {
    tracing::debug!("language tag {ws:?}");
    if let Some("tags") = params.query.as_deref() {
        writing_system_tags(&ws, &cfg).await.into_response()
    } else {
        fetch_wirting_system_ldml(&ws, &params, &cfg)
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
