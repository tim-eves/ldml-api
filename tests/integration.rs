use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use hyper::header::LOCATION;
use langtags::json::LangTags;
use language_tag::Tag;
use ldml_api::{
    app,
    config::{self, Profiles},
};
use serde_json::json;
use std::{path::Path, str::FromStr, sync::LazyLock};
use tower::{util::ServiceExt, Service};

fn parse_config(langtags: impl AsRef<Path>, sldr: impl AsRef<Path>) -> Profiles {
    config::Profiles::from_reader(
        json!({"test": {"langtags": langtags.as_ref(), "sldr": sldr.as_ref()}})
            .to_string()
            .as_bytes(),
    )
    .expect("profiles")
    .set_default("test")
}

static PROFILES: LazyLock<Profiles> = LazyLock::new(|| parse_config("tests/short", "tests"));

#[inline]
fn get_app() -> Router {
    app(PROFILES.clone()).expect("Router")
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
        .call(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("Request"),
        )
        .await
        .expect("Response");

    let query_response = app
        .call(
            Request::builder()
                .uri("/index.html?query=langtags&ext=json")
                .body(Body::empty())
                .expect("Request"),
        )
        .await
        .expect("Response");

    let query_response_staging = app
        .oneshot(
            Request::builder()
                .uri("/index.html?query=langtags&ext=json&staging=1")
                .body(Body::empty())
                .expect("Request"),
        )
        .await
        .expect("Response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(fallback_response.status(), StatusCode::OK);
    assert_eq!(query_response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        query_response_staging.status(),
        StatusCode::PERMANENT_REDIRECT
    );
    let query_response_staging_location = query_response_staging
        .headers()
        .get(LOCATION)
        .expect("Location HTTP header");
    assert_eq!(
        query_response_staging_location
            .to_str()
            .expect("Location HTTP header value"),
        "/langtags.json?staging=1"
    );
    const INDEX_BODY: &str = include_str!("../src/index.html");
    let body = axum::body::to_bytes(response.into_body(), INDEX_BODY.len())
        .await
        .expect("Index page body");
    assert_eq!(std::str::from_utf8(&body), Ok(INDEX_BODY));
}

#[tokio::test]
async fn status_page() {
    let mut app = get_app();

    let response = app
        .call(
            Request::builder()
                .uri("/status")
                .body(Body::empty())
                .expect("Request"),
        )
        .await
        .expect("Response");

    assert_eq!(response.status(), StatusCode::OK);

    let profile = PROFILES["test"].clone();
    let status_body = json!({
        "service": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
        "profiles": {
            "test": { "langtags": {
                    "api": profile.langtags.api_version(),
                    "date": profile.langtags.date(),
                    "tagsets": profile.langtags.len()
                }}
        }
    })
    .to_string();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Status body");
    assert_eq!(std::str::from_utf8(&body), Ok(status_body.as_str()));
}

async fn request_ldml_file(app: &mut Router, tag: &Tag) -> StatusCode {
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
async fn query_tags() {
    let app = get_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/frm?query=tags"))
                .body(Body::empty())
                .expect("Request"),
        )
        .await
        .expect("Response");
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .unwrap();
    assert_eq!(
        &body[..],
        b"frm=frm-FR=frm-Latn=frm-Latn-FR\n\
          frm-BE=frm-Latn-BE\n\
          frm-1606nict=frm-FR-1606nict=frm-Latn-1606nict=frm-Latn-FR-1606nict\n\
          frm-BE-1606nict=frm-Latn-BE-1606nict"
    );
}

#[tokio::test]
async fn simple_writing_system_request() {
    let mut app = get_app();

    macro_rules! assert_tag_exists {
        ($tag:literal) => {
            let tag = Tag::from_str($tag).expect("Tag");
            assert_eq!(
                request_ldml_file(&mut app, &tag).await,
                StatusCode::OK,
                "NotFound: {tag}"
            );
        };
    }

    assert_tag_exists!("thv-Latn-DZ-x-ahaggar");
    assert_tag_exists!("eka-Latn-NG-x-ekajuk");
    assert_tag_exists!("thv-DZ-x-ahaggar");
    assert_tag_exists!("eka-NG-x-ekajuk");
    assert_tag_exists!("eka-NG-x-ekajuk");
    assert_eq!(
        request_ldml_file(&mut app, &Tag::from_str("en-KP").expect("Tag")).await,
        StatusCode::NOT_FOUND
    );
}

fn generate_testing_tag_list(langtags: &LangTags) -> impl Iterator<Item = Tag> + '_ {
    langtags
        .tagsets()
        .filter_map(|ts| ts.sldr.then(|| ts.iter()))
        .flatten()
        .cloned()
}

#[ignore = "requires production data set."]
#[tokio::test]
async fn palaso_writing_systems_list_production() {
    palaso_writing_systems_list("production").await
}

#[ignore = "requires staging data set."]
#[tokio::test]
async fn palaso_writing_systems_list_staging() {
    palaso_writing_systems_list("staging").await
}

async fn palaso_writing_systems_list(profile: &str) {
    let src_top_level = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cfg = parse_config(
        src_top_level.join("data/langtags").join(profile),
        src_top_level.join("data/sldr").join(profile),
    );
    let mut tags = generate_testing_tag_list(&cfg["test"].langtags).collect::<Vec<_>>();
    tags.sort();
    let mut app = app(cfg).expect("Router");
    for (l, tag) in tags.into_iter().enumerate() {
        let status = request_ldml_file(&mut app, &tag).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{profile}: Tag {tag} at line {line}: not found",
            line = l + 1
        );
    }
}
