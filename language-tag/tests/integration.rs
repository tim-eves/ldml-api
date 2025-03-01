use std::str::FromStr;

use language_tag::{ExtensionRef, ParseTagError, Tag};

#[test]
fn builder() {
    let tag = Tag::builder()
        .lang("en")
        .script("Latn")
        .region("US")
        .variant("2abc")
        .extensions(["a-bable", "q-babbel"])
        .build();
    assert_eq!(
        tag,
        Tag::from_str("en-Latn-US-2abc-a-bable-q-babbel").unwrap()
    );
}

#[test]
fn parser() {
    use crate::{ParseTagError, Tag};

    let gf_cases = [
        ("-", Err("failed to parse tag: -".into())),
        ("de", Ok(Tag::with_lang("de"))),
        (
            "en-x-priv2",
            Ok(Tag::builder().lang("en").private("x-priv2").build()),
        ),
        ("en-us", Ok(Tag::builder().lang("en").region("us").build())),
        (
            "en-Latn-US",
            Ok(Tag::builder()
                .lang("en")
                .script("Latn")
                .region("US")
                .build()),
        ),
        (
            "ca-valencia",
            Ok(Tag::builder().lang("ca").variant("valencia").build()),
        ),
        (
            "en-Latn-US-2abc-3cde-a2c3e-xwhat-x-priv2",
            Ok(Tag::builder()
                .lang("en")
                .script("Latn")
                .region("US")
                .variants(["2abc", "3cde", "a2c3e", "xwhat"])
                .private("x-priv2")
                .build()),
        ),
        (
            "en-aaa-ccc-Latn-US-2abc-what2-a-bable-test-q-babbel-x-priv1",
            Ok(Tag::builder()
                .lang("en-aaa-ccc")
                .script("Latn")
                .region("US")
                .variants(["2abc", "what2"])
                .extension("a-bable")
                .extension("a-test")
                .extension("q-babbel")
                .private("x-priv1")
                .build()),
        ),
        (
            "x-priv1-priv2-xpriv3",
            Ok(Tag::privateuse("x-priv1-priv2-xpriv3")),
        ),
        (
            "en-gan-yue-Latn",
            Ok(Tag::builder().lang("en-gan-yue").script("Latn").build()),
        ),
    ];
    for (test, result) in &gf_cases {
        assert_eq!(
            test.parse().map_err(|e: ParseTagError| e.to_string()),
            *result
        );
    }
}

#[test]
fn from_str() {
    use std::str::FromStr;
    assert_eq!(
        Tag::from_str("en-Latn-US").expect("should parse langtag"),
        Tag::builder()
            .lang("en")
            .region("US")
            .script("Latn")
            .build()
    );

    assert_eq!(
        Tag::from_str("en-Latn-USA")
            .err()
            .expect("should not parse bad langtag")
            .to_string(),
        "failed to parse tag: en-Latn-USA"
    );
}

#[test]
fn display() {
    let mut tag = Tag::with_lang("en-aaa-ccc");
    tag.set_script("Latn");
    tag.set_region("US");
    tag.push_variant("2abc");
    tag.push_variant("what2");
    tag.set_extensions(["a-bable", "q-babbel"]);
    tag.set_private("x-priv1");
    println!("{tag:?} failed as {tag}");
    assert_eq!(
        tag.to_string(),
        "en-aaa-ccc-Latn-US-2abc-what2-a-bable-q-babbel-x-priv1"
    );
}

#[test]
fn sorting() {
    let aa = Tag::with_lang("aa");
    let aa_et = Tag::builder().lang("aa").region("ET").build();
    let aa_latn = Tag::builder().lang("aa").script("Latn").build();
    let aa_latn_et = Tag::builder()
        .lang("aa")
        .script("Latn")
        .region("ET")
        .build();
    let standard = [&aa, &aa_et, &aa_latn, &aa_latn_et];
    let mut test = [&aa_latn_et, &aa, &aa_et, &aa_latn];
    test.sort();

    assert_eq!(test, standard);
}

#[test]
fn getters() {
    let tag =
        Tag::from_str("en-Latn-US-1abc-2def-3ghi-a-abcdef-b-ghijklmn-c-tester-x-priv").unwrap();
    assert_eq!(tag.lang(), "en");
    assert_eq!(
        tag.script().expect("Tag should have a script subtag"),
        "Latn"
    );
    assert_eq!(tag.region().expect("Tag should have a region subtag"), "US");
    assert_eq!(
        tag.private().expect("Tag should have a private extension"),
        "x-priv"
    );
    assert_eq!(tag.variants().collect::<Vec<_>>(), ["1abc", "2def", "3ghi"]);
    assert_eq!(
        tag.extensions().collect::<Vec<_>>(),
        ["a-abcdef", "b-ghijklmn", "c-tester"]
    );

    let tag = Tag::default();
    assert_eq!(tag.lang(), "");
    assert_eq!(tag.script(), None);
    assert_eq!(tag.region(), None);
    assert_eq!(tag.private(), None);
    assert_eq!(tag.variants().collect::<Vec<_>>(), Vec::<&str>::new());
    assert_eq!(
        tag.extensions().collect::<Vec<_>>(),
        Vec::<ExtensionRef>::new()
    );
}

#[test]
fn setters() {
    // Test each in isolation
    let mut tag = Tag::default();
    tag.set_lang("en");
    assert_eq!(tag, Tag::with_lang("en"));
    tag.set_script("Latn");
    assert_eq!(tag, Tag::from_str("en-Latn").unwrap());
    let mut tag = Tag::with_lang("en");
    tag.set_region("US");
    assert_eq!(tag, Tag::from_str("en-US").unwrap());
    let mut tag = Tag::with_lang("en");
    tag.set_variants(["2abc", "1cde"]);
    assert_eq!(tag, Tag::from_str("en-2abc-1cde").unwrap());
    let mut tag = Tag::with_lang("en");
    tag.set_extensions(["a-vari", "q-abcdef"]);
    assert_eq!(tag, Tag::from_str("en-a-vari-q-abcdef").unwrap());
    let mut tag = Tag::with_lang("en");
    tag.push_variant("2abc");
    assert_eq!(tag, Tag::from_str("en-2abc").unwrap());
    let mut tag = Tag::with_lang("en");
    tag.add_extension("a-var1");
    assert_eq!(tag, Tag::from_str("en-a-var1").unwrap());
    let mut tag = Tag::with_lang("en");
    tag.set_private("x-priv");
    assert_eq!(tag, Tag::from_str("en-x-priv").unwrap());

    // Test cumlatively
    let mut tag = Tag::with_lang("en");
    tag.set_script("Latn");
    assert_eq!(tag, Tag::from_str("en-Latn").unwrap());
    tag.set_region("US");
    assert_eq!(tag, Tag::from_str("en-Latn-US").unwrap());
    tag.set_variants(["1abc", "2def"]);
    assert_eq!(tag, Tag::from_str("en-Latn-US-1abc-2def").unwrap());
    tag.push_variant("3ghi");
    assert_eq!(tag, Tag::from_str("en-Latn-US-1abc-2def-3ghi").unwrap());
    tag.set_extensions(["a-abcdef", "b-ghijklmn"]);
    assert_eq!(
        tag,
        Tag::from_str("en-Latn-US-1abc-2def-3ghi-a-abcdef-b-ghijklmn").unwrap()
    );
    tag.add_extension("c-tester");
    assert_eq!(
        tag,
        Tag::from_str("en-Latn-US-1abc-2def-3ghi-a-abcdef-b-ghijklmn-c-tester").unwrap()
    );
    tag.set_private("x-priv");
    assert_eq!(
        tag,
        Tag::from_str("en-Latn-US-1abc-2def-3ghi-a-abcdef-b-ghijklmn-c-tester-x-priv").unwrap()
    );

    tag.set_script("");
    assert_eq!(
        tag,
        Tag::from_str("en-US-1abc-2def-3ghi-a-abcdef-b-ghijklmn-c-tester-x-priv").unwrap()
    );

    assert_eq!(
        tag.pop_variant()
            .as_deref()
            .expect("should have popped a variant"),
        "3ghi"
    );
    assert_eq!(
        tag,
        Tag::from_str("en-US-1abc-2def-a-abcdef-b-ghijklmn-c-tester-x-priv").unwrap()
    );

    tag.add_extension("b-opqrstuv");
    assert_eq!(
        tag,
        Tag::from_str("en-US-1abc-2def-a-abcdef-b-ghijklmn-opqrstuv-c-tester-x-priv").unwrap()
    );
    tag.add_extension("b-abcdef");
    assert_eq!(
        tag,
        Tag::from_str("en-US-1abc-2def-a-abcdef-b-abcdef-ghijklmn-opqrstuv-c-tester-x-priv")
            .unwrap()
    );
    assert!(tag.remove_extension("b-abcdef"));

    assert!(tag.has_extension("a-abcdef"));
    assert!(tag.has_extension("c-tester"));
    assert!(tag.has_extension("b-opqrstuv"));
    assert!(!tag.has_extension("d-opqrstuv"));

    assert!(tag.remove_extension("c-tester"));
    assert_eq!(
        tag,
        Tag::from_str("en-US-1abc-2def-a-abcdef-b-ghijklmn-opqrstuv-x-priv").unwrap()
    );

    assert!(tag.remove_extension("b-ghijklmn"));
    assert!(tag.remove_extension("a-abcdef"));
    assert!(tag.remove_extension("b-opqrstuv"));
    assert_eq!(tag, Tag::from_str("en-US-1abc-2def-x-priv").unwrap());
    let mut tag = Tag::from_str("en-Latn-US-1abc-a-abcdef-x-priv").unwrap();
    tag.set_private("");
    tag.set_extensions([]);
    tag.set_variants([]);
    tag.set_region("");
    tag.set_script("");
    assert_eq!(tag, Tag::with_lang("en"));

    tag.set_private("");
    tag.set_extensions([]);
    tag.set_variants([]);
    tag.set_region("");
    tag.set_script("");
    assert_eq!(tag, Tag::with_lang("en"));
}

#[cfg(feature = "serde")]
mod serde {
    use super::Tag;
    use serde_json;
    use std::str::FromStr;

    #[test]
    fn serialize() {
        let tag = Tag::from_str("en-Latn-US-1abc-a-abcdef-x-priv").unwrap();
        assert_eq!(
            "\"en-Latn-US-1abc-a-abcdef-x-priv\"",
            serde_json::to_string(&tag).expect("should serialize Tag")
        )
    }

    #[test]
    fn deserialize() {
        let tag = Tag::from_str("en-Latn-US-1abc-a-abcdef-x-priv").unwrap();
        assert_eq!(
            tag,
            serde_json::from_str("\"en-Latn-US-1abc-a-abcdef-x-priv\"")
                .expect("should deserialize Tag")
        )
    }
}
