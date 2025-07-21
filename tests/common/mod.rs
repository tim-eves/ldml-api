use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use language_tag::Tag;
use ldml_api::config::{self, Profiles};
use serde_json::json;
use std::path::Path;
use tower::util::ServiceExt;

pub fn parse_config(langtags: impl AsRef<Path>, sldr: impl AsRef<Path>) -> Profiles {
    config::Profiles::from_reader(
        json!({"test": {"langtags": langtags.as_ref(), "sldr": sldr.as_ref()}})
            .to_string()
            .as_bytes(),
    )
    .expect("should parse generated configuration")
    .set_fallback("test")
    .expect(" should set default profile to: \"test\"")
}

pub async fn request_ldml_file(app: &mut Router, tag: &Tag) -> StatusCode {
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
