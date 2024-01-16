use crate::tagset::TagSet;
use language_tag::{ExtensionRef, Tag};
use serde::Deserialize;
use std::{
    collections::{HashMap as Map, HashSet as Set},
    io::{self, BufRead},
};

#[derive(Debug, Default, PartialEq)]
pub struct LangTags {
    version: String,
    date: String,
    scripts: Set<String>,
    regions: Set<String>,
    variants: Set<String>,
    latn_variants: Set<String>,
    tagsets: Vec<TagSet>,
    full: Map<String, u32>,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "tag")]
enum Header {
    #[serde(rename = "_globalvar")]
    GlobalVar { variants: Set<String> },
    #[serde(rename = "_phonvar")]
    PhonVar { variants: Set<String> },
    #[serde(rename = "_version")]
    Version { api: String, date: String },
    #[serde(rename = "_conformance")]
    Conformance {
        scripts: Vec<String>,
        regions: Vec<String>,
    },
}

impl LangTags {
    pub fn from_reader<R: BufRead>(reader: R) -> io::Result<Self> {
        use serde_json::Value;

        let mut values: Vec<Value> = serde_json::from_reader(reader)?;
        // This processes the everything at the start of the langtags.json file
        // that looks like a header, stopping at the first TagSet.
        let mut tagset_start = 0usize;
        let mut langtags = values
            .iter()
            .cloned()
            // Convert JSON values into Header values until they stop being headers.
            .map_while(|v| serde_json::from_value(v).ok())
            // Process the Header values updating the LangTags struct members as each header directs.
            .fold(Default::default(), |mut lts, header| {
                tagset_start += 1;
                match header {
                    Header::GlobalVar { variants } => LangTags { variants, ..lts },
                    Header::PhonVar {
                        variants: latn_variants,
                    } => LangTags {
                        latn_variants,
                        ..lts
                    },
                    Header::Version { api, date } => LangTags {
                        version: api,
                        date,
                        ..lts
                    },
                    Header::Conformance { scripts, regions } => {
                        lts.scripts.extend(scripts);
                        lts.regions.extend(regions);
                        lts
                    }
                }
            });
        // Remove the values that were headers, leaving only the valid TagSets.
        values.drain(..tagset_start);
        langtags.tagsets = serde_json::from_value(Value::Array(values))?;
        langtags.build_caches();
        langtags.shrink_to_fit();
        Ok(langtags)
    }

    fn build_caches(&mut self) {
        for (i, ts) in self.tagsets.iter().enumerate() {
            self.full
                .extend(ts.iter().map(|tag| (tag.to_string(), i as u32)));
            self.scripts.insert(ts.script().unwrap().to_owned());
            self.regions.insert(ts.region().unwrap().to_owned());
            self.regions.extend(ts.regions.iter().cloned());
        }
    }

    fn shrink_to_fit(&mut self) {
        self.scripts.shrink_to_fit();
        self.regions.shrink_to_fit();
        self.variants.shrink_to_fit();
        self.latn_variants.shrink_to_fit();
        self.tagsets.shrink_to_fit();
    }

    pub fn conformant(&self, tag: &Tag) -> bool {
        let valid_script = tag
            .script()
            .map(|s| self.scripts.contains(s))
            .unwrap_or(true);
        let valid_region = tag
            .region()
            .map(|s| self.regions.contains(s))
            .unwrap_or(true);
        valid_script && valid_region
    }

    fn valid_region(ts: &TagSet, region: Option<&str>) -> bool {
        if let Some(region) = region {
            ts.region() == Some(region) || ts.regions.contains(&region.to_owned())
        } else {
            true
        }
    }

    fn valid_variants(&self, ts: &TagSet, tag: &Tag) -> bool {
        !tag.has_variants()
            || tag.variants().all(|v| {
                let v = v.to_owned();
                ts.variants.contains(&v)
                    || self.variants.contains(&v)
                    || (!ts.nophonvars
                        && (ts.tag.script().is_none() || ts.script() == Some("Latn"))
                        && self.latn_variants.contains(&v))
            })
    }

    fn valid_extensions<'a>(
        ts: &TagSet,
        extensions: impl IntoIterator<Item = ExtensionRef<'a>>,
    ) -> bool {
        let supplied_extensions: Set<_> = extensions.into_iter().collect();
        supplied_extensions.is_empty() || {
            let mut candidate_extensions = ts.iter().map(|t| t.extensions().collect::<Set<_>>());
            candidate_extensions.any(|tc| !tc.is_subset(&supplied_extensions))
        }
    }

    pub fn orthographic_normal_form(&self, tag: &Tag) -> Option<&TagSet> {
        let mut key = tag.clone();
        let idx = self
            .full
            .get(&key.to_string())
            .or_else(|| {
                key.set_private("");
                self.full.get(&key.to_string())
            })
            .or_else(|| {
                key.set_extensions([]);
                self.full.get(&key.to_string())
            })
            .or_else(|| {
                key.set_variants([]);
                self.full.get(&key.to_string())
            })
            .or_else(|| {
                key.set_region("");
                self.full.get(&key.to_string())
            });
        let idx = *idx? as usize;
        let ts = self.tagsets.get(idx)?;

        if key == *tag
            || LangTags::valid_region(ts, tag.region())
                && self.valid_variants(ts, tag)
                && LangTags::valid_extensions(ts, tag.extensions())
                && ts.full.private() == tag.private()
        {
            Some(ts)
        } else {
            None
        }
    }

    pub fn locale_normal_form(&self, tag: &Tag) -> Option<TagSet> {
        self.orthographic_normal_form(tag).map(|ortho_tagset| {
            let mut ts = ortho_tagset.clone();
            if let Some(region) = tag.region() {
                let ri = ts.regions.iter().position(|x| x == region).unwrap();
                ts.regions[ri] = ts.region().unwrap().to_owned();
                ts.full.set_region(region);
                ts.tag.set_region(region);
                for i in (0..ts.tags.len()).rev() {
                    if ts.tags[i].region().is_none() {
                        ts.tags.remove(i);
                    }
                }
                for t in &mut ts.tags {
                    t.set_region(region);
                }
            }
            ts
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Tag, &TagSet)> + DoubleEndedIterator + Clone {
        self.tagsets
            .iter()
            .flat_map(|ts| ts.iter().map(move |t| (t, ts)))
    }

    pub fn tagsets(&self) -> impl Iterator<Item = &TagSet> + DoubleEndedIterator + Clone {
        self.tagsets.iter()
    }
}

#[cfg(test)]
mod test {
    use super::{Header, LangTags, TagSet};
    use language_tag::Tag;
    use serde_json::{json, Value};
    use std::{fs::File, io::BufReader, iter::once, str::FromStr};

    #[test]
    fn langtags_header() {
        let src = json!([
                        {
                            "regions": [ "AA", "BU", "CP", "DD", "EU", "FX", "GS", "HM", "NT", "QM", "SU", "TA", "UN", "XA", "YD", "ZR" ],
                            "scripts": [ "Aran", "Cpmn", "Egyd", "Hira", "Inds", "Jamo", "Mero", "Moon", "Pcun", "Qaaa", "Roro", "Shui", "Visp", "Zinh" ],
                            "tag": "_conformance"
                        },
                        {
                            "tag": "_globalvar",
                            "variants": [ "simple" ]
                        },
                        {
                            "tag": "_phonvar",
                            "variants": [ "alalc97", "fonipa", "fonkirsh", "fonnapa", "fonupa", "fonxsamp" ]
                        },
                        {
                            "api": "1.2.1",
                            "date": "2021-06-29",
                            "tag": "_version"
                        },
                        {
                            "full": "aa-Latn-ET",
                            "iana": [ "Afar" ],
                            "iso639_3": "aar",
                            "localname": "Qafar",
                            "localnames": [ "Qafar af" ],
                            "name": "Afar",
                            "region": "ET",
                            "regionname": "Ethiopia",
                            "script": "Latn",
                            "sldr": true,
                            "tag": "aa",
                            "tags": [ "aa-ET", "aa-Latn" ],
                            "windows": "aa-Latn-ET"
                        }
                ]).to_string();
        let db: Vec<Value> = serde_json::from_str(&src).expect("Array of JSON values");
        assert_eq!(
            serde_json::from_value::<Vec<Header>>(Value::Array(db[0..4].into()))
                .expect("Special tag value"),
            vec![
                Header::Conformance {
                    regions: vec![
                        "AA".into(),
                        "BU".into(),
                        "CP".into(),
                        "DD".into(),
                        "EU".into(),
                        "FX".into(),
                        "GS".into(),
                        "HM".into(),
                        "NT".into(),
                        "QM".into(),
                        "SU".into(),
                        "TA".into(),
                        "UN".into(),
                        "XA".into(),
                        "YD".into(),
                        "ZR".into(),
                    ],
                    scripts: vec![
                        "Aran".into(),
                        "Cpmn".into(),
                        "Egyd".into(),
                        "Hira".into(),
                        "Inds".into(),
                        "Jamo".into(),
                        "Mero".into(),
                        "Moon".into(),
                        "Pcun".into(),
                        "Qaaa".into(),
                        "Roro".into(),
                        "Shui".into(),
                        "Visp".into(),
                        "Zinh".into(),
                    ]
                },
                Header::GlobalVar {
                    variants: vec!["simple".into()].into_iter().collect()
                },
                Header::PhonVar {
                    variants: vec![
                        "alalc97".into(),
                        "fonipa".into(),
                        "fonkirsh".into(),
                        "fonnapa".into(),
                        "fonupa".into(),
                        "fonxsamp".into()
                    ]
                    .into_iter()
                    .collect()
                },
                Header::Version {
                    api: "1.2.1".into(),
                    date: "2021-06-29".into()
                }
            ]
        );
        assert_eq!(
            serde_json::from_value::<TagSet>(db[4].clone()).expect("TagSet value"),
            TagSet {
                full: Tag::builder()
                    .lang("aa")
                    .script("Latn")
                    .region("ET")
                    .build(),
                iana: vec!["Afar".into()],
                iso639_3: "aar".into(),
                localname: "Qafar".into(),
                localnames: vec!["Qafar af".into()],
                name: "Afar".into(),
                regionname: "Ethiopia".into(),
                sldr: true,
                tag: Tag::with_lang("aa"),
                tags: vec![
                    Tag::builder().lang("aa").region("ET").build(),
                    Tag::builder().lang("aa").script("Latn").build()
                ],
                windows: Tag::builder()
                    .lang("aa")
                    .script("Latn")
                    .region("ET")
                    .build(),
                ..Default::default()
            }
        )
    }

    // Load and cache the langtags.json database on demand, we use OnceLock to
    // ensure only one thread ever tries to load the db, and the rest get the
    // cached copy.
    fn load_langtags_from_reader() -> &'static LangTags {
        use std::sync::OnceLock;
        static SHARED_LTDB: OnceLock<LangTags> = OnceLock::new();
        SHARED_LTDB.get_or_init(|| {
            let file = File::open("langtags/test/langtags.json")
                .or(File::open("test/langtags.json"))
                .expect("open langtags.json");
            LangTags::from_reader(BufReader::new(file)).expect("read langtags.json")
        })
    }

    #[test]
    fn sanity_check_keyspace() {
        let ltdb = load_langtags_from_reader();
        // let n_globvars: usize = ltdb.variants.len();
        // let n_phonvars: usize = ltdb.latn_variants.len();
        let counts = ltdb.tagsets.iter().map(|ts| {
            (2 + ts.tags.len()
                + ts.iter().filter(|t| t.region().is_some()).count() * ts.regions.len())
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
            len = ltdb.tagsets.len(),
            n_tags = counts.clone().sum::<usize>()
        );
        let n_tags: usize = ltdb.tagsets.iter().zip(counts).map(|(ts, nc)| {
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
            len = ltdb.tagsets.len()
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
        use super::Set;

        for ts in &load_langtags_from_reader().tagsets {
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
        use super::Set;

        for ts in &load_langtags_from_reader().tagsets {
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
        use super::Set;

        for ts in &load_langtags_from_reader().tagsets {
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
}
