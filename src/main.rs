use axum::{
    AddExtensionLayer,
    extract::{ Extension, Query, Path },
    http::StatusCode,
    response::{ IntoResponse },
    routing::{ get, get_service },
    Router,
};
use serde::Deserialize;
use std::{
    fmt::Display,
    io,
};
use tower_http::{
    services::ServeFile,
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

    // build our application with a single route
    let static_help = get_service(ServeFile::new("static/index.html"))
                        .handle_error(internal_error);
    let app = Router::new()
        .route("/index.html", static_help)
        .route("/:ws_id", get(writing_system_endpoint))
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


async fn internal_error(error: impl Display) -> impl IntoResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Unhandled internal error: {error}")
    )
}


type APIResponse = (StatusCode, &'static str);


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
    staging: Option<Toggle>,
    uid: Option<u32>,
}

async fn writing_system_endpoint(Path(ws): Path<Vec<Tag>>, Query(params): Query<WSParams>) -> APIResponse {
    tracing::debug!("language tag {ws:?}");
    if let Some(query) = params.query {
        process_query(
            query, 
            params.ext.as_deref().unwrap_or("txt"),
            *params.staging.unwrap_or_default()).await
    } else if let Some(ws) = ws.get(0) {
        process_writing_system(
            ws,
            params.ext.as_deref().unwrap_or("xml"),
            *params.flatten.unwrap_or(Toggle::ON),
            params.inc.as_deref().unwrap_or_default(),
            params.revid.as_deref().unwrap_or_default(),
            *params.staging.unwrap_or_default(),
            params.uid.unwrap_or_default()).await
    } else {
        (StatusCode::NOT_FOUND,"")
    }
}


async fn process_writing_system(_ws: &Tag,
                                _ext: &str, 
                                _flatten: bool, 
                                _inc: &str, 
                                _revid: &str, 
                                _staging: bool,
                                _uid: u32) -> APIResponse 
{
    todo!()
}
