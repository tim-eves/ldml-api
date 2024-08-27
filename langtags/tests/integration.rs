use std::{
    collections::HashSet as Set, fs::File, io::BufReader, iter::once, path::PathBuf, str::FromStr,
};

use langtags::{self, json::LangTags};
use language_tag::Tag;

// Load and cache the langtags.json database on demand, we use OnceLock to
// ensure only one thread ever tries to load the db, and the rest get the
// cached copy.
fn load_langtags_from_reader() -> &'static LangTags {
    use std::sync::OnceLock;
    static SHARED_LTDB: OnceLock<LangTags> = OnceLock::new();
    SHARED_LTDB.get_or_init(|| {
        let file = File::open(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("langtags.json"),
        )
        .expect("open langtags.json");
        LangTags::from_reader(BufReader::new(file)).expect("read langtags.json")
    })
}

#[test]
fn sanity_check_keyspace() {
    let ltdb = load_langtags_from_reader();
    // let n_globvars: usize = ltdb.variants.len();
    // let n_phonvars: usize = ltdb.latn_variants.len();
    let counts = ltdb.tagsets().map(|ts| {
        (2 + ts.tags.len() + ts.iter().filter(|t| t.region().is_some()).count() * ts.regions.len())
            * (1 + ts
                .variants
                .iter()
                .map(String::as_str)
                .filter(|&v| ts.iter().all(|t| !t.variants().any(|x| x == v)))
                .count())
        // * (1 + n_globvars)
        // * (1 + if ts.script().as_deref() == Some("Latn") {n_phonvars} else {0})
    });
    println!(
        "{len} records found in DB. {n_tags} tags calculated",
        len = ltdb.tagsets().count(),
        n_tags = counts.clone().sum::<usize>()
    );
    let n_tags: usize = ltdb.tagsets().zip(counts).map(|(ts, nc)| {
        let all_tags = ts.all_tags();
        let n = all_tags.clone().count();
        assert_eq!(nc, n, "TagSet {{ full: \"{}\", tag: \"{}\", tags: {:?}, regions: {:?}, variants: {:?} }}\n{}",
            ts.full,
            ts.tag,
            ts.tags.iter().map(Tag::to_string).collect::<Vec<_>>(),
            ts.regions,
            ts.variants,
            all_tags.map(|t| t.to_string() + "\n").collect::<Vec<_>>().concat());
        n
    })
    .sum();
    println!(
        "{len} records found in DB. {n_tags} tags counted",
        len = ltdb.tagsets().count()
    );
}

#[test]
fn conformant_tag() {
    let ltdb = load_langtags_from_reader();
    assert_eq!(ltdb.conformant(&Tag::with_lang("en")), true);
    assert_eq!(
        ltdb.conformant(&Tag::builder().lang("en").region("RU").build()),
        true
    );
    assert_eq!(
        ltdb.conformant(&Tag::builder().lang("en").script("Thai").build()),
        true
    );
    assert_eq!(
        ltdb.conformant(
            &Tag::builder()
                .lang("en")
                .script("Thai")
                .region("RU")
                .build()
        ),
        true
    );
    assert_eq!(
        ltdb.conformant(
            &Tag::builder()
                .lang("en")
                .script("Moon")
                .region("EU")
                .build()
        ),
        true
    );
    assert_eq!(
        ltdb.conformant(
            &Tag::builder()
                .lang("en")
                .script("Thai")
                .region("__")
                .build()
        ),
        false
    );
    assert_eq!(
        ltdb.conformant(
            &Tag::builder()
                .lang("en")
                .script("____")
                .region("RU")
                .build()
        ),
        false
    );
}

#[test]
fn normal_forms() {
    let ltdb = load_langtags_from_reader();
    let ts = ltdb.orthographic_normal_form(&Tag::from_str("en-US").expect("Tag value"));
    assert_eq!(
        ts.expect("TagSet").full,
        Tag::from_str("en-Latn-US").unwrap()
    );
    let ts = ltdb.orthographic_normal_form(&Tag::from_str("aeb-TN").expect("Tag value"));
    assert_eq!(
        ts.expect("TagSet").full,
        Tag::from_str("aeb-Arab-TN").unwrap()
    );
    let ts = ltdb.orthographic_normal_form(&Tag::from_str("aeb-Arab").expect("Tag value"));
    assert_eq!(
        ts.expect("TagSet").full,
        Tag::from_str("aeb-Arab-TN").unwrap()
    );
    let ts = ltdb.orthographic_normal_form(&Tag::from_str("aeb-Hebr").expect("Tag value"));
    assert_eq!(
        ts.expect("TagSet").full,
        Tag::from_str("aeb-Hebr-IL").unwrap()
    );
    let ts = ltdb.orthographic_normal_form(&Tag::from_str("aeb-IL").expect("Tag value"));
    assert_eq!(
        ts.expect("TagSet").full,
        Tag::from_str("aeb-Hebr-IL").unwrap()
    );
    let ts = ltdb.orthographic_normal_form(&Tag::from_str("aeb").expect("Tag value"));
    assert_eq!(
        ts.expect("TagSet").full,
        Tag::from_str("aeb-Arab-TN").unwrap()
    );
    let ts = ltdb.orthographic_normal_form(&Tag::from_str("en-TW").expect("Tag value"));
    assert_eq!(
        ts.expect("TagSet").full,
        Tag::from_str("en-Latn-US").unwrap()
    );
    let ts = ltdb.orthographic_normal_form(&Tag::from_str("en-TW-simple").expect("Tag value"));
    assert_eq!(
        ts.expect("TagSet").full,
        Tag::from_str("en-Latn-US").unwrap()
    );
    let ts = ltdb.locale_normal_form(&Tag::from_str("en-TW").expect("Tag value"));
    assert_eq!(
        ts.expect("TagSet").full,
        Tag::from_str("en-Latn-TW").unwrap()
    );
    let ts = ltdb.locale_normal_form(&Tag::from_str("dgl-Copt").expect("Tag value"));
    assert_eq!(
        ts.expect("TagSet").full,
        Tag::from_str("dgl-Copt-SD-x-olnubian").unwrap()
    );
    // let ts = ltdb.locale_normal_form(&Tag::from_str("dgl-Copt-SD-a-test").expect("Tag value"));
    // assert_eq!(
    //     ts.expect("TagSet").full,
    //     Tag::from_str("dgl-Copt-SD-x-olnubian").unwrap()
    // );
}

#[test]
fn sanity_check_script() {
    for ts in load_langtags_from_reader().tagsets() {
        // Sanity check script
        let mut computed_scripts: Set<&str> = once(&ts.tag)
            .chain(ts.tags.iter())
            .flat_map(|t| t.script())
            .collect();
        computed_scripts.remove(ts.script().as_ref().expect("script missing."));
        assert_eq!(
            computed_scripts.len(),
            0,
            "Extra scripts in tagset {name} tags list: {computed_scripts:?}",
            name = ts.full.to_string()
        );
    }
}

#[test]
fn sanity_check_regions() {
    for ts in load_langtags_from_reader().tagsets() {
        // Sanity check regions
        assert!(!ts
            .regions
            .contains(&ts.region().expect("region missing.").to_owned()));
        let regions: Set<&str> = ts
            .regions
            .iter()
            .map(String::as_str)
            .chain(ts.region())
            .collect();
        let computed_regions: Set<&str> = ts.iter().flat_map(|t| t.region()).collect();
        assert_eq!(
            computed_regions.difference(&regions).count(),
            0,
            "Extra regions mentioned in tagset {name}: {:?}",
            computed_regions.difference(&regions),
            name = ts.full.to_string()
        );
    }
}

#[test]
fn sanity_check_variants() {
    for ts in load_langtags_from_reader().tagsets() {
        // Sanity check variants
        let name = ts.full.to_string();
        let variants: Set<&str> = ts
            .variants
            .iter()
            .map(String::as_str)
            .chain(ts.full.variants())
            .collect();

        // Check no full tag variants are in the tagset variants list.
        assert_eq!(
            variants.len(),
            ts.variants.len() + ts.full.variants().count(),
            "Ovelapping variants in tagset {name} between full tag & varaints list: {:?}",
            ts.variants
                .iter()
                .map(String::as_str)
                .collect::<Set<&str>>()
                .intersection(&ts.full.variants().collect())
        );

        // Check only variants from full tag and the variants list are used in the tags.
        let computed_variants: Set<&str> = ts.iter().flat_map(|t| t.variants()).collect();
        assert_eq!(
            computed_variants.difference(&variants).count(),
            0,
            "Extra variants mentioned in tagset {name}: {:?}",
            computed_variants.difference(&variants)
        );
    }
}
