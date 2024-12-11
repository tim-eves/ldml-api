use axum::{
    body::Body,
    extract::{Extension, Path, Query, Request, State},
    http::{header::CONTENT_DISPOSITION, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Router,
};
use axum_extra::headers::{ContentType, ETag, HeaderMapExt};
use language_tag::Tag;
use serde::Deserialize;
use std::{collections::HashMap, io, iter, path, str, sync::Arc};
use tokio::{fs, task};
use tracing::instrument;

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
        .route("/index.html", get(query_only))
        .fallback(query_only))
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
                .find_map(|(k, v)| qs.get(k).and_then(|&t| if *t { Some(v) } else { None }))
        })
        .unwrap_or_else(|| &profiles[""])
        .clone();

    req.extensions_mut().insert(config);
    next.run(req).await
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

#[instrument]
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

    Ok((headers, Body::from_stream(stream)))
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

#[derive(Deserialize, Debug)]
struct QueryParams {
    _ws_id: Option<Tag>,
    query: Option<LDMLQuery>,
    ext: Option<String>,
    staging: Option<Toggle>,
}

#[instrument(ret)]
async fn query_only(Query(params): Query<QueryParams>) -> impl IntoResponse {
    match params.query {
        Some(LDMLQuery::AllTags) => Err((
            StatusCode::NOT_FOUND,
            "LDML SERVER ERROR: The alltags file is obsolete. Please use 'query=langtags'.",
        )),
        Some(LDMLQuery::LangTags) => {
            let ext = params.ext.as_deref().unwrap_or("txt");
            let mut target = format!("/langtags.{ext}");
            if *params.staging.unwrap_or_default() {
                target += "?staging=1";
            }
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

#[instrument(skip(cfg))]
async fn writing_system_tags(ws: &Tag, cfg: &Config) -> impl IntoResponse {
    query_tags(ws, &cfg.langtags).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("No tagsets found for tag: {ws}"),
        )
    })
}

#[instrument(skip(cfg))]
async fn fetch_writing_system_ldml(ws: &Tag, params: WSParams, cfg: &Config) -> impl IntoResponse {
    let ext = params.ext.as_deref().unwrap_or("xml");
    let flatten = *params.flatten.unwrap_or(Toggle::ON);

    tracing::debug!(
        "find writing system in {path} with {params:?}",
        path = cfg.sldr_path(flatten).to_string_lossy()
    );
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

#[instrument(skip(cfg))]
async fn demux_writing_system(
    Path(ws): Path<Tag>,
    Query(params): Query<WSParams>,
    Extension(cfg): Extension<Arc<Config>>,
) -> impl IntoResponse {
    tracing::debug!("language tag {ws}");
    if let Some(query) = params.query {
        match query {
            LDMLQuery::AllTags | LDMLQuery::LangTags => (
                StatusCode::BAD_REQUEST,
                "query=alltags, or query=langtags is only valid without a ws_id.",
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

#[instrument(skip(langtags))]
fn query_tags(ws: &Tag, langtags: &LangTags) -> Option<String> {
    use langtags::tagset::render_equivalence_set;

    let tagset = langtags.orthographic_normal_form(ws)?;
    let regionsets = tagset.region_sets().map(render_equivalence_set);
    let variantsets = tagset.variant_sets().map(render_equivalence_set);
    iter::once(tagset.to_string())
        .chain(regionsets)
        .chain(variantsets)
        .reduce(|resp, ref set| resp + "\n" + set)
}

#[instrument(ret, skip(langtags))]
fn find_ldml_file(ws: &Tag, sldr_dir: &path::Path, langtags: &LangTags) -> Option<path::PathBuf> {
    // Lookup the tag set and generate a prefered sorted list.
    let tagset = langtags.orthographic_normal_form(ws)?;
    let tags: Vec<_> = tagset.iter().collect();

    let mut path = sldr_dir.to_path_buf();
    path.push(&tagset.lang()[0..1]);

    tags.iter()
        .map(|&tag| {
            path.join(tag.as_ref().replace('-', "_"))
                .with_extension("xml")
        })
        .rfind(|path| path.exists())
}

#[instrument]
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
