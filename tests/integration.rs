use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
    Router,
};
use axum_extra::headers::{CacheControl, HeaderMapExt};
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
    .expect("should parse generated configuration")
    .set_fallback("test")
    .expect(" should set default profile to: \"test\"")
}

static PROFILES: LazyLock<Profiles> = LazyLock::new(|| parse_config("tests/short", "tests"));

#[inline]
fn get_app() -> Router {
    app(PROFILES.clone()).expect("ldml-api::app should return Router")
}

#[tokio::test]
async fn index_page() {
    let mut app = get_app();

    let response = app
        .call(
            Request::builder()
                .uri("/index.html")
                .body(Body::empty())
                .expect("should request index page"),
        )
        .await
        .unwrap();

    let fallback_response = app
        .call(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("should request default page"),
        )
        .await
        .unwrap();

    let query_response = app
        .call(
            Request::builder()
                .uri("/index.html?query=langtags&ext=json")
                .body(Body::empty())
                .expect("should request langtags.json via query"),
        )
        .await
        .unwrap();

    let query_response_staging = app
        .oneshot(
            Request::builder()
                .uri("/index.html?query=langtags&ext=json&staging=1")
                .body(Body::empty())
                .expect("should request langtags.json from staging profile via query"),
        )
        .await
        .unwrap();

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
        .expect("should get Location HTTP header");
    assert_eq!(
        query_response_staging_location
            .to_str()
            .expect("should get Location HTTP header value"),
        "/langtags.json?test=1"
    );
    const INDEX_BODY: &str = include_str!("../src/index.html");
    let body = axum::body::to_bytes(response.into_body(), INDEX_BODY.len())
        .await
        .expect("should extract index page body");
    assert_eq!(std::str::from_utf8(&body), Ok(INDEX_BODY));
}

#[tokio::test]
async fn langtags_ext() {
    let mut app = get_app();

    let json_response = app
        .call(
            Request::builder()
                .uri("/langtags.json")
                .body(Body::empty())
                .expect("should request langtags.json via path"),
        )
        .await
        .unwrap();

    let txt_response = app
        .call(
            Request::builder()
                .uri("/langtags.txt")
                .body(Body::empty())
                .expect("should request langtags.txt via path"),
        )
        .await
        .unwrap();

    let json_response_staging = app
        .call(
            Request::builder()
                .uri("/langtags.json?test=1")
                .body(Body::empty())
                .expect("should request langtags.json from test profile via path"),
        )
        .await
        .unwrap();

    let txt_response_staging = app
        .call(
            Request::builder()
                .uri("/langtags.txt?test=1")
                .body(Body::empty())
                .expect("should request langtags.txt from test profile via path"),
        )
        .await
        .unwrap();

    let notfound_response = app
        .oneshot(
            Request::builder()
                .uri("/langtags.html")
                .body(Body::empty())
                .expect("should request invalid path"),
        )
        .await
        .unwrap();

    #[inline]
    //TODO: #[track_caller] when https://github.com/rust-lang/rust/issues/110011 is stablised.
    async fn response_to_str(resp: Response) -> String {
        String::from_utf8(
            axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .expect("should extract repsonse body text")
                .to_vec(),
        )
        .expect("should be utf8 string")
    }

    assert_eq!(
        json_response.status(),
        StatusCode::OK,
        "Body: {}",
        response_to_str(json_response).await
    );
    assert_eq!(
        txt_response.status(),
        StatusCode::OK,
        "Body: {}",
        response_to_str(txt_response).await
    );
    assert_eq!(
        json_response_staging.status(),
        StatusCode::OK,
        "Body: {}",
        response_to_str(json_response_staging).await
    );
    assert_eq!(
        txt_response_staging.status(),
        StatusCode::OK,
        "Body: {}",
        response_to_str(txt_response_staging).await
    );
    assert_eq!(
        notfound_response.status(),
        StatusCode::NOT_FOUND,
        "Body: {}",
        response_to_str(notfound_response).await
    );
}

#[tokio::test]
async fn status_page() {
    let mut app = get_app();

    let response = app
        .call(
            Request::builder()
                .uri("/status")
                .body(Body::empty())
                .expect("should request status JSON document"),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let profile = PROFILES.fallback().clone();
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
    assert!(response
        .headers()
        .typed_get::<CacheControl>()
        .expect("should have a cache-control header")
        .no_store());

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("should extract Status JSON document");
    assert_eq!(std::str::from_utf8(&body), Ok(status_body.as_str()));
}

async fn request_ldml_file(app: &mut Router, tag: &Tag) -> StatusCode {
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/{tag}"))
                .body(Body::empty())
                .expect(&format!("should request LDML for \"{tag}\" ")),
        )
        .await
        .unwrap();

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
                .expect("should request all tagsets for `frm` langtag"),
        )
        .await
        .unwrap();
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
            let tag = Tag::from_str($tag).expect(concat!("should parse \"", $tag, '"'));
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
        request_ldml_file(
            &mut app,
            &Tag::from_str("en-KP").expect("should parse \"en-KP\"")
        )
        .await,
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
    let mut tags = generate_testing_tag_list(&cfg.fallback().langtags).collect::<Vec<_>>();
    tags.sort();
    let mut app = app(cfg).expect("lb::app should return configured Router");
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
