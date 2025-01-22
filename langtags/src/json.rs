use crate::{langtags::LangTags as CoreLangTags, StringRepr};
use serde::Deserialize;
use std::{
    borrow::Borrow,
    collections::HashSet as Set,
    io::{self, BufRead},
    ops::{Deref, DerefMut},
};

#[derive(Debug, Default, PartialEq)]
pub struct LangTags {
    inner: CoreLangTags,
    version: StringRepr,
    date: StringRepr,
}

impl Deref for LangTags {
    type Target = CoreLangTags;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for LangTags {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Borrow<CoreLangTags> for LangTags {
    fn borrow(&self) -> &CoreLangTags {
        &self.inner
    }
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "tag")]
enum Header {
    #[serde(rename = "_globalvar")]
    GlobalVar { variants: Set<StringRepr> },
    #[serde(rename = "_phonvar")]
    PhonVar { variants: Set<StringRepr> },
    #[serde(rename = "_version")]
    Version { api: StringRepr, date: StringRepr },
    #[serde(rename = "_conformance")]
    Conformance {
        scripts: Vec<StringRepr>,
        regions: Vec<StringRepr>,
    },
}

impl LangTags {
    pub fn from_reader<R: BufRead>(reader: R) -> io::Result<Self> {
        use serde_json::Value;

        let mut values: Vec<Value> = serde_json::from_reader(reader)?;
        // This processes everything at the start of the langtags.json file
        // that matches a header, stopping at the first TagSet.
        let mut tagset_start = 0usize;
        let mut langtags = LangTags::default();

        // Convert JSON values into Header values until they stop being
        // headers, and process the Header values updating the LangTags struct
        // members as each header directs.
        for header in values
            .iter()
            .cloned()
            .map_while(|v| serde_json::from_value(v).ok())
        {
            tagset_start += 1;
            match header {
                Header::GlobalVar { variants } => langtags.variants = variants,
                Header::PhonVar { variants } => langtags.latn_variants = variants,
                Header::Version { api, date } => {
                    langtags.version = api;
                    langtags.date = date;
                }
                Header::Conformance { scripts, regions } => {
                    langtags.scripts.extend(scripts);
                    langtags.regions.extend(regions);
                }
            }
        }

        // Remove the values that were headers, leaving only the valid TagSets.
        values.drain(..tagset_start);
        langtags.tagsets = serde_json::from_value(Value::Array(values))?;
        langtags.build_caches();
        langtags.shrink_to_fit();
        Ok(langtags)
    }

    #[inline(always)]
    pub fn api_version(&self) -> &str {
        &self.version
    }

    #[inline(always)]
    pub fn date(&self) -> &str {
        &self.date
    }
}

#[cfg(test)]
mod test {
    use super::Header;
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
}
