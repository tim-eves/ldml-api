use axum::{
    AddExtensionLayer,
    body::StreamBody,
    extract::{ Extension, Query, Path },
    http::{ header, HeaderValue, StatusCode },
    response::{ Headers, IntoResponse },
    routing::{ get },
    Router,
};
use serde::Deserialize;
use std::{
    io,
    path,
    sync::Arc,
};
use tokio::fs;
use tower_http::{
    trace::TraceLayer,
};

mod config;
mod tag;
mod toggle;
mod langtags;

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

use crate::tag::Tag;
use crate::langtags::{ LangTags, TagSet };
use crate::config::Config;
use crate::toggle::Toggle;


#[tokio::main]
async fn main() -> io::Result<()> {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var(
            "RUST_LOG",
            "ldml_api=debug,tower_http=debug",
        )
    }
    tracing_subscriber::fmt::init();
    
    // Load configuraion
    let cfg = config::profiles::default()?;
    tracing::debug!("loaded profiles: {profiles:?}", profiles = cfg.keys().collect::<Vec<_>>());

    async fn static_help() -> &'static str {
        include_str!("index.html")
    }
    let app = Router::new()
        .route("/langtags.:ext", get(langtags))
        .route("/:ws_id", get(writing_system_endpoint))
        .route("/index.html", get(static_help))
        .layer(AddExtensionLayer::new(cfg["staging"].clone()))
        .layer(TraceLayer::new_for_http());

        // run it with hyper on localhost:3000
        let addr = "127.0.0.1:3000".parse().unwrap();
        tracing::debug!("listening on {addr}");
        axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
    Ok(())
}


async fn stream_file(path: &path::Path, ) -> impl IntoResponse {
    // Let's avoid path traversal attacks, or other shenanigans.
    let file_name = path.file_name()
        .ok_or((StatusCode::BAD_REQUEST, String::default()))?
        .to_string_lossy();

    let file = fs::File::open(path).await;
    let file = match file {
        Ok(file) => file,
        Err(err) => return Err(
            (
                StatusCode::NOT_FOUND, 
                format!("Cannot open: {err}: {}", 
                        path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                )
            )
        ),
    };
    let guess = mime_guess::from_path(path);
    let mime = guess
        .first_raw()
        .map(HeaderValue::from_static)
        .unwrap_or_else(|| {
            HeaderValue::from_str(mime::APPLICATION_OCTET_STREAM.as_ref()).unwrap()
        });
    let stream = tokio_util::io::ReaderStream::new(file);
    let body = StreamBody::new(stream);
    let headers = Headers([
        (header::CONTENT_TYPE, mime),
        (
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!("attachment; filename=\"{file_name}\"")).expect(""),
        ),
    ]);

    Ok((headers, body))
}

    
async fn langtags(Path(ext): Path<String>, Extension(cfg): Extension<Arc<Config>>) -> impl IntoResponse
{
    tracing::debug!("langtags.{ext}");
    stream_file(&cfg.langtags_dir.join("langtags").with_extension(ext)).await
}    


#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum LDMLQuery {
    AllTags,
    LangTags,
}

async fn process_query(query: LDMLQuery, ext: &str, staging: bool) -> APIResponse {
    match query {
        LDMLQuery::AllTags => {
            (
                StatusCode::NOT_FOUND, 
                "LDML SERVER ERROR: The alltags file is obsolete. Please use 'query=langtags'."
            )
        }
        LDMLQuery::LangTags => {
            tracing::debug!("load {staging:?}/langtags.{ext}");
            todo!()
        }
    }
}


#[derive(Deserialize)]
struct WSParams {
    query: Option<LDMLQuery>,
    ext: Option<String>,
    flatten: Option<Toggle>,
    inc: Option<String>,
    revid: Option<String>,
    uid: Option<u32>,
}

async fn writing_system_endpoint(Path(ws): Path<Tag>, Query(params): Query<WSParams>, Extension(cfg): Extension<Arc<Config>>) -> impl IntoResponse {
    tracing::debug!("language tag {ws:?}");
    let _ext = params.ext.as_deref().unwrap_or("xml");
    let flatten = *params.flatten.unwrap_or(Toggle::ON);
    let _xpath = params.inc.as_deref().unwrap_or_default();
    let _revid = params.revid.as_deref().unwrap_or_default();
    let _uid = params.uid.unwrap_or_default();

    let path = find_ldml_file(&ws, &cfg.sldr_path(flatten), &cfg.langtags)
        .ok_or((StatusCode::NOT_FOUND, format!("No LDML for {ws}")));
    match path {
        Ok(path) => stream_file(path.as_ref()).await.into_response(),
        Err(err) => err.into_response()
    }
}


fn find_ldml_file(
    ws: &Tag, 
    sldr_dir: &path::Path, langtags: 
    &LangTags) -> Option<path::PathBuf> 
{
    // Lookup the tag set and generate a prefered sorted list.
    let mut tagset: Vec<_> = langtags.get(ws)?
        .iter()
        .collect();
    tagset.sort_by(|a, b| a.partial_cmp(b).unwrap());
    tagset.push(ws);

    tagset.iter()
        .map(|&tag| {
            let mut path = path::PathBuf::from(sldr_dir);
            path.push(&tag.lang[0..1]);
            path.push(tag.to_string().replace("-","_"));
            path.with_extension("xml")
        })
        .rfind(|path| path.exists())
}

