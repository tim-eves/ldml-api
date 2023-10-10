use langtags::json::LangTags;
use std::{collections::HashMap, path::PathBuf, sync::Arc};

#[derive(Debug, PartialEq)]
pub struct Config {
    pub sendfile_method: Option<String>,
    pub langtags: LangTags,
    pub langtags_dir: PathBuf,
    pub sldr_dir: PathBuf,
}

impl Config {
    pub fn sldr_path(&self, flat: bool) -> PathBuf {
        self.sldr_dir.join(if flat { "flat" } else { "unflat" })
    }
}

pub type Profiles = HashMap<String, Arc<Config>>;

pub mod profiles {
    use super::{Config, LangTags, Profiles};
    use serde_json::Value;
    use std::{
        fs::File,
        io::{self, BufReader, Read},
        path::{Path, PathBuf},
    };

    pub fn from<P, S>(path: P, default: S) -> io::Result<Profiles>
    where
        P: AsRef<Path>,
        S: AsRef<str>,
    {
        let mut profiles = from_reader(File::open(path)?)?;
        let default = default.as_ref();
        if !default.is_empty() {
            profiles.insert("".into(), profiles[default].clone());
        }
        Ok(profiles)
    }

    fn into_parse_error(msg: &str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, format!("parse failed: {msg}"))
    }

    pub fn from_reader<R: Read>(reader: R) -> io::Result<Profiles> {
        let cfg: Value = serde_json::from_reader(reader)?;

        let profiles = cfg
            .as_object()
            .ok_or_else(|| into_parse_error("profile map"))?;
        let mut configs = Profiles::with_capacity(profiles.len());
        // Read defined profiles
        for (name, v) in profiles.iter() {
            let mut sendfile_method = Default::default();
            let mut langtags_dir = Default::default();
            let mut sldr_dir = Default::default();

            v.as_object()
                .ok_or_else(|| into_parse_error("config object"))
                .and_then(|tbl| {
                    sendfile_method = tbl
                        .get("sendfile_method")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    sldr_dir = tbl["sldr"]
                        .as_str()
                        .map(PathBuf::from)
                        .ok_or_else(|| into_parse_error("sldr path"))?;
                    langtags_dir = tbl["langtags"]
                        .as_str()
                        .map(PathBuf::from)
                        .ok_or_else(|| into_parse_error("sldr path"))?;
                    Ok(())
                })?;

            let reader = BufReader::new(File::open(langtags_dir.join("langtags.json"))?);
            let langtags = LangTags::from_reader(reader)?;

            configs.insert(
                name.to_owned(),
                Config {
                    sendfile_method,
                    langtags,
                    langtags_dir,
                    sldr_dir,
                }
                .into(),
            );
        }

        Ok(configs)
    }
}

#[cfg(test)]
mod test {
    use super::{profiles, Arc, Config, LangTags, Profiles};

    #[test]
    fn missing_config() {
        let res = profiles::from("test/missing-config.json", "");
        assert_eq!(
            res.err().expect("io::Error: Not found.").kind(),
            std::io::ErrorKind::NotFound
        );
    }

    #[test]
    fn unreadable_config() {
        let res = profiles::from_reader(&br"hang this isn't JSON!"[..])
            .err()
            .expect("io::Error: Invlalid data.");
        assert_eq!(res.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(res.to_string(), "expected value at line 1 column 1");
    }

    #[test]
    fn missing_langtags() {
        let res = profiles::from_reader(
            &br#"{
                    "staging": {
                        "langtags": "/staging/data/",
                        "sldr": "/staging/data/sldr/"
                    },
                    "production": {
                        "sendfile_method": "X-Accel-Redirect",
                        "langtags": "/data/",
                        "sldr": "/data/sldr/"
                    }
                 }"#[..],
        )
        .err()
        .expect("io:Error: Not found during profiles::from_reader.");
        assert_eq!(res.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn valid_langtags() {
        let res = profiles::from_reader(
            &br#"{
                     "staging": {
                         "langtags": "test/short/",
                         "sldr": "/staging/data/sldr/"
                     },
                     "production": {
                         "sendfile_method": "X-Accel-Redirect",
                         "langtags": "test/short/",
                         "sldr": "/data/sldr/"
                     }
                 }"#[..],
        )
        .expect("Profiles value.");
        let langtags_json = &br#"
            [
                {
                    "regions": [ "AA", "AC", "AN", "AQ", "BU", "BV", "CP", "CS", "DD", "EU", "EZ", "FX", "GS", "HM", "NT", "QM", "QN", "QO", "QP", "QQ", "QR", "QS", "QT", "QU", "QV", "QW", "QX", "QY", "QZ", "SU", "TA", "TF", "TP", "UN", "XA", "XB", "XC", "XD", "XE", "XF", "XG", "XH", "XI", "XJ", "XL", "XM", "XN", "XO", "XP", "XQ", "XR", "XS", "XT", "XU", "XV", "XW", "XY", "XZ", "YD", "YU", "ZR", "ZZ" ],
                    "scripts": [ "Aran", "Cpmn", "Egyd", "Egyh", "Hira", "Hrkt", "Inds", "Jamo", "Mero", "Moon", "Pcun", "Phlv", "Psin", "Qaaa", "Qaab", "Qaac", "Qaad", "Qaae", "Qaaf", "Qaag", "Qaah", "Qaai", "Qaaj", "Qaak", "Qaal", "Qaam", "Qaan", "Qaao", "Qaap", "Qaaq", "Qaar", "Qaas", "Qaat", "Qaau", "Qaav", "Qaaw", "Qaax", "Qaay", "Qaaz", "Qaba", "Qabb", "Qabc", "Qabd", "Qabe", "Qabf", "Qabg", "Qabh", "Qabi", "Qabj", "Qabk", "Qabl", "Qabm", "Qabn", "Qabo", "Qabp", "Qabq", "Qabr", "Qabs", "Qabt", "Qabu", "Qabv", "Qabw", "Qabx", "Roro", "Shui", "Syre", "Syrn", "Visp", "Zinh", "Zmth", "Zsye", "Zsym" ],
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
                    "api": "1.3",
                    "date": "2023-02-20",
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
                },
                {
                    "full": "aa-Arab-ET",
                    "iana": [ "Afar" ],
                    "iso639_3": "aar",
                    "name": "Afar",
                    "nophonvars": true,
                    "region": "ET",
                    "regionname": "Ethiopia",
                    "regions": [ "DJ", "ER" ],
                    "script": "Arab",
                    "sldr": false,
                    "tag": "aa-Arab",
                    "windows": "aa-Arab-ET"
                },
                {
                    "full": "aa-Latn-DJ",
                    "iana": [ "Afar" ],
                    "iso639_3": "aar",
                    "localname": "Qafar",
                    "localnames": [ "Qafar af" ],
                    "name": "Afar",
                    "region": "DJ",
                    "regionname": "Djibouti",
                    "script": "Latn",
                    "sldr": true,
                    "tag": "aa-DJ",
                    "windows": "aa-Latn-DJ"
                },
                {
                    "full": "aa-Latn-ER",
                    "iana": [ "Afar" ],
                    "iso639_3": "aar",
                    "localname": "Qafar",
                    "localnames": [ "Qafar af" ],
                    "name": "Afar",
                    "region": "ER",
                    "regionname": "Eritrea",
                    "script": "Latn",
                    "sldr": true,
                    "tag": "aa-ER",
                    "windows": "aa-Latn-ER"
                },
                {
                    "full": "aa-Ethi-ET",
                    "iana": [ "Afar" ],
                    "iso639_3": "aar",
                    "name": "Afar",
                    "nophonvars": true,
                    "region": "ET",
                    "regionname": "Ethiopia",
                    "regions": [ "DJ", "ER" ],
                    "script": "Ethi",
                    "sldr": false,
                    "tag": "aa-Ethi",
                    "windows": "aa-Ethi-ET"
                }
            ]"#[..];
        let mut expected = Profiles::new();
        expected.insert(
            "production".into(),
            Arc::new(Config {
                sendfile_method: Some("X-Accel-Redirect".into()),
                langtags: LangTags::from_reader(langtags_json)
                    .expect("LangTags production test case."),
                langtags_dir: "test/short/".into(),
                sldr_dir: "/data/sldr/".into(),
            }),
        );
        expected.insert(
            "staging".into(),
            Config {
                sendfile_method: None,
                langtags: LangTags::from_reader(langtags_json)
                    .expect("LangTags staging test case."),
                langtags_dir: "test/short/".into(),
                sldr_dir: "/staging/data/sldr/".into(),
            }
            .into(),
        );

        assert_eq!(res, expected);
    }
}
