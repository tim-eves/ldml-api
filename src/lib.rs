use axum::{
    body::Body,
    extract::{ConnectInfo, Extension, Path, Query, Request, State},
    http::{header::CONTENT_DISPOSITION, HeaderMap, HeaderName, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Json, Router,
};
use axum_extra::headers::{ContentType, ETag, HeaderMapExt};
use language_tag::Tag;
use serde::Deserialize;
use std::{collections::HashMap, io, iter, net::SocketAddr, path, sync::Arc};
use tokio::{fs, task};
use tracing::{instrument, Instrument};

pub mod config;
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

pub fn app(cfg: Profiles) -> io::Result<Router> {
    let status_response = status(&cfg);
    Ok(Router::new()
        .route("/langtags.{ext}", get(langtags))
        .layer(middleware::from_fn(etag::layer))
        .route(
            "/{ws_id}",
            get(demux_writing_system)
                .layer(middleware::from_fn(etag::layer))
                .layer(middleware::from_fn(etag::revid::converter)),
        )
        .route("/", get(query_only))
        .route("/index.html", get(query_only))
        .layer(middleware::from_fn_with_state(cfg.into(), profile_selector))
        .route("/status", get(move || async { status_response }))
        .fallback(query_only))
}

fn status(profiles: &Profiles) -> impl IntoResponse + Clone {
    use serde_json::{json, Value};

    let profiles = Value::from_iter(profiles.iter().map(|config| {
        let mut obj = json!({"langtags": {
            "api": config.langtags.api_version(),
            "date": config.langtags.date(),
            "tagsets": config.langtags.len()
        }});
        if let Some(method) = config.sendfile_method.as_deref() {
            obj.as_object_mut()
                .unwrap()
                .insert("sendfile".into(), method.into());
        }
        (&config.name, obj)
    }));
    Json(json!({
        "service": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
        "profiles": profiles
    }))
}

async fn static_help() -> impl IntoResponse {
    Html(include_str!("index.html"))
}

async fn profile_selector(
    State(profiles): State<Box<Profiles>>,
    mut req: Request,
    next: Next,
) -> Response {
    let config = req
        .uri()
        .query()
        .and_then(|q| serde_urlencoded::from_str::<HashMap<String, Toggle>>(q).ok())
        .and_then(|qs| {
            profiles
                .iter()
                .find(|cfg| qs.get(&cfg.name).is_some_and(|&t| *t))
        })
        .unwrap_or_else(|| profiles.fallback())
        .clone();

    let span = tracing::info_span!(
        "request",
        profile = config.name,
        client = get_client_addr(&req),
        uri = req.uri().to_string(),
        agent = get_user_agent(&req)
    );
    req.extensions_mut().insert(config);
    let rsp = next.run(req).instrument(span.clone()).await;
    tracing::info!(parent: &span, status = %rsp.status());
    rsp
}

fn get_client_addr(req: &Request) -> Option<String> {
    let headers = req.headers();
    let forwarded_for = headers
        .get(HeaderName::from_static("x-forwarded-for"))
        .and_then(|v| v.to_str().ok()?.split(',').next().map(str::to_string));
    let real_ip = headers
        .get(HeaderName::from_static("x-real-ip"))
        .and_then(|v| v.to_str().ok().map(str::to_string));
    let forwarded = headers
        .get(HeaderName::from_static("forwarded"))
        .and_then(|value| {
            let rest = value.to_str().ok()?.split_once("for=")?.1;
            rest.split([';', ',']).next().map(str::to_string)
        });
    let remote = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(s)| s.ip().to_string());
    forwarded.or(forwarded_for).or(real_ip).or(remote)
}

#[inline]
fn get_user_agent(req: &Request) -> Option<String> {
    req.headers()
        .get(HeaderName::from_static("user-agent"))
        .and_then(|v| v.to_str().ok().map(str::to_string))
}

// struct ServiceError(StatusCode, String);

// impl Display for ServiceError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.write_fmt(format_args!("{{\"status\": {status}, \"detail\": {detail:?}}}", status = self.0, detail = self.1))
//     }
// }

// impl<M: AsRef<str> + ToOwned> From<(StatusCode, M)> for ServiceError {
//     fn from((code, msg): (StatusCode, M)) -> Self {
//         ServiceError(code, msg.as_ref().to_owned())
//     }
// }

// impl IntoResponse for ServiceError {
//     fn into_response(self) -> Response {
//         (self.0,self.1).into_response()
//     }
// }

// type ServiceResult<T> = Result<T, ServiceError>;

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
    let disposition = format!("attachment; filename=\"{name}\"", name = filename.display())
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

    Ok((headers, Body::from_stream(stream)))
}

async fn langtags(
    Path(ext): Path<String>,
    Extension(cfg): Extension<Arc<Config>>,
) -> impl IntoResponse {
    let path = cfg.langtags_dir.join("langtags").with_extension(ext);
    tracing::info!("streaming \"{}\"", path.display());
    stream_file(&path).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LDMLQuery {
    AllTags,
    LangTags,
    Tags,
}

#[derive(Deserialize, Debug)]
struct QueryParams {
    _ws_id: Option<Tag>,
    query: Option<LDMLQuery>,
    ext: Option<String>,
}

#[instrument(ret, skip_all)]
async fn query_only(
    Query(params): Query<QueryParams>,
    Extension(cfg): Extension<Arc<Config>>,
) -> impl IntoResponse {
    match params.query {
        Some(LDMLQuery::AllTags) => Err((
            StatusCode::NOT_FOUND,
            "LDML SERVER ERROR: The alltags file is obsolete. Please use 'query=langtags'.",
        )),
        Some(LDMLQuery::LangTags) => {
            let ext = params.ext.as_deref().unwrap_or("txt");
            let target = format!("/langtags.{ext}?{profile}", profile = &cfg.name);
            Ok(Redirect::permanent(&target).into_response())
        }
        Some(LDMLQuery::Tags) => Err((
            StatusCode::BAD_REQUEST,
            "LDML SERVER ERROR: query=tags requires a ws_id",
        )),
        None => Ok(static_help().await.into_response()),
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

async fn fetch_writing_system_ldml(ws: &Tag, params: WSParams, cfg: &Config) -> impl IntoResponse {
    let ext = params.ext.as_deref().unwrap_or("xml");
    let flatten = *params.flatten.unwrap_or(Toggle::ON);

    tracing::debug!(
        "find writing system in {path} with {params:?}",
        path = cfg.sldr_path(flatten).display()
    );
    let path = find_ldml_file(ws, &cfg.sldr_path(flatten), &cfg.langtags)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("No LDML for {ws}")).into_response())?;
    let etag = etag::revid::from_ldml(&path).or_else(|| etag::from_metadata(&path));
    let mut headers = HeaderMap::new();

    if let Some(tag) = etag {
        headers.typed_insert(tag);
    }
    if params.inc.is_none() && params.uid.is_none() {
        tracing::info!(
            "streaming {}\"{}\"",
            if flatten { "flat " } else { "" },
            path.display()
        );
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
        tracing::info!(
            "customising {}\"{}\" with xpaths=\"{:?}\" and uid=\"{:?}\"",
            if flatten { "flat " } else { "" },
            path.display(),
            params.inc,
            params.uid
        );
        ldml_customisation(&path, params.inc, params.uid)
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
    if let Some(query) = params.query {
        match query {
            LDMLQuery::AllTags | LDMLQuery::LangTags => (
                StatusCode::BAD_REQUEST,
                "query=alltags, or query=langtags is only valid without a ws_id.",
            )
                .into_response(),
            LDMLQuery::Tags => query_tags(&ws, &cfg.langtags)
                .ok_or_else(|| {
                    (
                        StatusCode::NOT_FOUND,
                        format!("No tagsets found for tag: {ws}"),
                    )
                })
                .into_response(),
        }
    } else {
        fetch_writing_system_ldml(&ws, params, &cfg)
            .await
            .into_response()
    }
}

fn query_tags(ws: &Tag, langtags: &LangTags) -> Option<String> {
    use langtags::tagset::render_equivalence_set;

    let tagset = langtags.orthographic_normal_form(ws)?;
    let regionsets = tagset.region_sets().map(render_equivalence_set);
    let variantsets = tagset.variant_sets().map(render_equivalence_set);
    tracing::info!("tagset for \"{ws}\": {tagset}");
    iter::once(tagset.to_string())
        .chain(regionsets)
        .chain(variantsets)
        .reduce(|resp, ref set| resp + "\n" + set)
}

fn find_ldml_file(ws: &Tag, sldr_dir: &path::Path, langtags: &LangTags) -> Option<path::PathBuf> {
    // Lookup the tag set and generate a prefered sorted list.
    let tagset = langtags.orthographic_normal_form(ws)?;
    let tags: Vec<_> = tagset.iter().collect();
    let path = sldr_dir.join(&tagset.lang()[0..1]);

    tags.iter()
        .map(|&tag| {
            path.join(tag.as_ref().replace('-', "_"))
                .with_extension("xml")
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
