use crate::{langtags::LangTags as CoreLangTags, StringRepr};
use serde::Deserialize;
use std::{
    borrow::Borrow,
    collections::HashSet as Set,
    fmt::Display,
    io::{BufRead, Read, Seek},
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

#[derive(Debug)]
enum ErrorKind {
    Json(serde_json::Error),
    MissingHeader { line: usize, column: usize, header: String },
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(err) => write!(f, "{err}"),
            Self::MissingHeader{ line, column, header}  => write!(f, r#"expected header object "{header}" at line {line}, column {column}"#),
        }
    }
}

#[derive(Debug)]
pub struct Error(ErrorKind);

impl Error {
    #[cold]
    fn missing_header<R: Read + Seek>(header: &str, reader: &mut R) -> Self {
        let prefix_len = reader.stream_position().expect("could not get file read offset") as usize;
        reader.rewind().expect("could not seek to start of file");
        let mut prefix = String::with_capacity(prefix_len);
        reader.take(prefix_len as u64).read_to_string(&mut prefix).expect("could nod read headers");
        let line = prefix.lines().count();
        let column = prefix_len - prefix.rfind('\n').unwrap_or_default();
        return Self(ErrorKind::MissingHeader { line, column, header: header.into() }.into());
    }
}

impl From<serde_json::Error> for Error {
    #[inline(always)]
    fn from(value: serde_json::Error) -> Self {
        Error(ErrorKind::Json(value))
    }
}

impl From<ErrorKind> for Error {
    #[inline(always)]
    fn from(value: ErrorKind) -> Self {
        Error(value)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not parse langtags.json data: {}", self.0)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.0 {
            ErrorKind::Json(ref err) => Some(err),
            ErrorKind::MissingHeader { .. } => None,
        }
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
    Version {
        #[serde(default)]
        api: StringRepr,
        #[serde(default)]
        date: StringRepr,
    },
    #[serde(rename = "_conformance")]
    Conformance {
        scripts: Vec<StringRepr>,
        regions: Vec<StringRepr>,
    },
}

impl LangTags {
    pub fn from_reader<R: Read + BufRead + Seek>(mut reader: R) -> Result<Self, Error> {
        use serde_json::Value;

        let mut values: Vec<Value> = serde_json::from_reader(reader.by_ref())?;
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

        match (&langtags.version.is_empty(), &langtags.date.is_empty()) {
            (false, false) => (),
            (true, true)   => return Err(Error::missing_header("_version", &mut reader)),
            (true, false)  => return Err(Error::missing_header("_version/api", &mut reader)),
            (false, true)  => return Err(Error::missing_header("_version/date", &mut reader)),
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
    use std::io::Cursor;

    use crate::json::LangTags;

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

    #[test]
    fn missing_headers() {
        let src = serde_json::to_string_pretty(&json!([
            {
                "api": "1.2.1",
                // "date": "2021-06-29",
                "tag": "_version"
            }
        ])).expect("could not pretty print JSON Value");
        let langtags = LangTags::from_reader(Cursor::new(src.as_bytes()));
        assert_eq!(
            langtags.unwrap_err().to_string(),
            "Could not parse langtags.json data: expected header object \"_version/date\" at line 6, column 2"
        );
    }

    #[test]
    fn bad_json() {
        let src = br#"[
            {
                "api": "1.2.1"
                "date": "2021-06-29",
                "tag": "_version"
            }
        ]"#;

        let langtags = LangTags::from_reader(Cursor::new(src.as_slice()));
        assert_eq!(
            langtags.unwrap_err().to_string(),
            "Could not parse langtags.json data: expected `,` or `}` at line 4 column 17"
        );
    }
}
