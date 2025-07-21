use axum::http::StatusCode;
use std::path::Path;

use langtags::json::LangTags;
use language_tag::Tag;
use ldml_api::app;

mod common;

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

fn generate_testing_tag_list(langtags: &LangTags) -> impl Iterator<Item = Tag> + '_ {
    langtags
        .tagsets()
        .filter_map(|ts| ts.sldr.then(|| ts.iter()))
        .flatten()
        .cloned()
}

async fn palaso_writing_systems_list(profile: &str) {
    let src_top_level = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cfg = common::parse_config(
        src_top_level.join("data/langtags").join(profile),
        src_top_level.join("data/sldr").join(profile),
    );
    let mut tags = generate_testing_tag_list(&cfg.fallback().langtags).collect::<Vec<_>>();
    tags.sort();
    let mut app = app(cfg).expect("lb::app should return configured Router");
    for _ in 0..1 {
        for (l, tag) in tags.iter().enumerate() {
            let status = common::request_ldml_file(&mut app, &tag).await;
            assert_eq!(
                status,
                StatusCode::OK,
                "{profile}: Tag {tag} at line {line}: not found",
                line = l + 1
            );
        }
    }
}
