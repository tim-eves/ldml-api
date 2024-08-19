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

        let private_is_valid = ts.full.private().map_or(true, |ts_priv| {
            tag.private().is_some_and(|tag_priv| tag_priv == ts_priv)
        });
        if key == *tag
            || LangTags::valid_region(ts, tag.region())
                && self.valid_variants(ts, tag)
                && LangTags::valid_extensions(ts, tag.extensions())
                && private_is_valid
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

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&Tag, &TagSet)> + Clone {
        self.tagsets
            .iter()
            .flat_map(|ts| ts.iter().map(move |t| (t, ts)))
    }

    pub fn tagsets(&self) -> impl DoubleEndedIterator<Item = &TagSet> + Clone {
        self.tagsets.iter()
    }
}

#[cfg(test)]
mod test {
    use super::{Header, TagSet};
    use language_tag::Tag;
    use serde_json::{json, Value};

    #[test]
    fn headers() {
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
    }

    #[test]
    fn tagset() {
        let src = json!(
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
        )
        .to_string();
        assert_eq!(
            serde_json::from_str::<TagSet>(&src).expect("TagSet value"),
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
}
