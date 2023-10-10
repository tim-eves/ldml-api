#![cfg(test)]
use crate::tag::ExtensionRef;

use super::Tag;

#[test]
fn constructors() {
    assert_eq!(
        Tag::default(),
        Tag::from_parts("", None, None, [], [], None)
    );
    assert_eq!(
        Tag::with_lang("en"),
        Tag::from_parts("en", None, None, [], [], None)
    );
    assert_eq!(
        Tag::privateuse("x-priv"),
        Tag::from_parts("", None, None, [], [], "x-priv")
    );
}

#[test]
fn setters() {
    // Test each in isolation
    let mut tag = Tag::default();
    tag.set_lang("en");
    assert_eq!(tag, Tag::from_parts("en", None, None, [], [], None));
    let mut tag = Tag::with_lang("en");
    tag.set_script("Latn");
    assert_eq!(tag, Tag::from_parts("en", "Latn", None, [], [], None));
    let mut tag = Tag::with_lang("en");
    tag.set_region("US");
    assert_eq!(tag, Tag::from_parts("en", None, "US", [], [], None));
    let mut tag = Tag::with_lang("en");
    tag.set_variants(["2abc", "1cde"]);
    assert_eq!(
        tag,
        Tag::from_parts("en", None, None, ["2abc", "1cde"], [], None)
    );
    let mut tag = Tag::with_lang("en");
    tag.set_extensions(["a-vari", "q-abcdef"]);
    assert_eq!(
        tag,
        Tag::from_parts("en", None, None, [], ["a-vari", "q-abcdef"], None)
    );
    let mut tag = Tag::with_lang("en");
    tag.push_variant("2abc");
    assert_eq!(tag, Tag::from_parts("en", None, None, ["2abc"], [], None));
    let mut tag = Tag::with_lang("en");
    tag.add_extension("a-var1");
    assert_eq!(tag, Tag::from_parts("en", None, None, [], ["a-var1"], None));
    let mut tag = Tag::with_lang("en");
    tag.set_private("x-priv");
    assert_eq!(tag, Tag::from_parts("en", None, None, [], [], "x-priv"));

    // Test cumlatively
    let mut tag = Tag::with_lang("en");
    tag.set_script("Latn");
    assert_eq!(tag, Tag::from_parts("en", "Latn", None, [], [], None));
    tag.set_region("US");
    assert_eq!(tag, Tag::from_parts("en", "Latn", "US", [], [], None));
    tag.set_variants(["1abc", "2def"]);
    assert_eq!(
        tag,
        Tag::from_parts("en", "Latn", "US", ["1abc", "2def"], [], None)
    );
    tag.push_variant("3ghi");
    assert_eq!(
        tag,
        Tag::from_parts("en", "Latn", "US", ["1abc", "2def", "3ghi"], [], None)
    );
    tag.set_extensions(["a-abcdef", "b-ghijklmn"]);
    assert_eq!(
        tag,
        Tag::from_parts(
            "en",
            "Latn",
            "US",
            ["1abc", "2def", "3ghi"],
            ["a-abcdef", "b-ghijklmn"],
            None
        )
    );
    tag.add_extension("c-tester");
    assert_eq!(
        tag,
        Tag::from_parts(
            "en",
            "Latn",
            "US",
            ["1abc", "2def", "3ghi"],
            ["a-abcdef", "b-ghijklmn", "c-tester"],
            None
        )
    );
    tag.set_private("x-priv");
    assert_eq!(
        tag,
        Tag::from_parts(
            "en",
            "Latn",
            "US",
            ["1abc", "2def", "3ghi"],
            ["a-abcdef", "b-ghijklmn", "c-tester"],
            "x-priv"
        )
    );

    tag.set_script("");
    assert_eq!(
        tag,
        Tag::from_parts(
            "en",
            None,
            "US",
            ["1abc", "2def", "3ghi"],
            ["a-abcdef", "b-ghijklmn", "c-tester"],
            "x-priv"
        )
    );

    assert_eq!(
        tag.pop_variant().as_deref().expect("Popped variant"),
        "3ghi"
    );
    assert_eq!(
        tag,
        Tag::from_parts(
            "en",
            None,
            "US",
            ["1abc", "2def"],
            ["a-abcdef", "b-ghijklmn", "c-tester"],
            "x-priv"
        )
    );

    tag.add_extension("b-opqrstuv");
    assert_eq!(
        tag,
        Tag::from_parts(
            "en",
            None,
            "US",
            ["1abc", "2def"],
            ["a-abcdef", "b-ghijklmn", "b-opqrstuv", "c-tester"],
            "x-priv"
        )
    );
    tag.add_extension("b-abcdef");
    assert_eq!(
        tag,
        Tag::from_parts(
            "en",
            None,
            "US",
            ["1abc", "2def"],
            [
                "a-abcdef",
                "b-abcdef",
                "b-ghijklmn",
                "b-opqrstuv",
                "c-tester"
            ],
            "x-priv"
        )
    );
    assert!(tag.remove_extension("b-abcdef"));

    assert!(tag.has_extension("a-abcdef"));
    assert!(tag.has_extension("c-tester"));
    assert!(tag.has_extension("b-opqrstuv"));
    assert!(!tag.has_extension("d-opqrstuv"));

    assert!(tag.remove_extension("c-tester"));
    assert_eq!(
        tag,
        Tag::from_parts(
            "en",
            None,
            "US",
            ["1abc", "2def"],
            ["a-abcdef", "b-ghijklmn", "b-opqrstuv"],
            "x-priv"
        )
    );

    assert!(tag.remove_extension("b-ghijklmn"));
    assert!(tag.remove_extension("a-abcdef"));
    assert!(tag.remove_extension("b-opqrstuv"));
    assert_eq!(
        tag,
        Tag::from_parts("en", None, "US", ["1abc", "2def"], [], "x-priv")
    );
    let mut tag = Tag::from_parts("en", "Latn", "US", ["1abc"], ["a-abcdef"], "x-priv");
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

#[test]
fn getters() {
    let tag = Tag::from_parts(
        "en",
        "Latn",
        "US",
        ["1abc", "2def", "3ghi"],
        ["a-abcdef", "b-ghijklmn", "c-tester"],
        "x-priv",
    );
    assert_eq!(tag.lang(), "en");
    assert_eq!(tag.script().expect("script"), "Latn");
    assert_eq!(tag.region().expect("region"), "US");
    assert_eq!(tag.private().expect("private"), "x-priv");
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
        Tag::from_parts("en", "Latn", "US", ["2abc"], ["a-bable", "q-babbel"], None),
    );
}

#[test]
fn from_str() {
    use std::str::FromStr;
    assert_eq!(
        Tag::from_str("en-Latn-US").expect("Ok value not found"),
        Tag::builder()
            .lang("en")
            .region("US")
            .script("Latn")
            .build()
    );

    assert_eq!(
        Tag::from_str("en-Latn-USA")
            .err()
            .expect("Err value not found")
            .to_string(),
        "error Tag at: en-Latn-USA"
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
