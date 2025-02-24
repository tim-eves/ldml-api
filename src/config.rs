use langtags::json::LangTags;
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufReader, Read},
    ops::Index,
    path::PathBuf,
    sync::Arc,
};

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

#[derive(Clone)]
pub struct Profiles {
    inner: ProfilesInner,
    default: Option<Arc<Config>>,
}

type ProfilesInner = HashMap<String, Arc<Config>>;

impl Index<&str> for Profiles {
    type Output = Arc<Config>;

    fn index(&self, index: &str) -> &Self::Output {
        self.get(index).expect("no config found for profile")
    }
}

impl Profiles {
    pub fn set_default<S>(mut self, default: impl Into<Option<S>>) -> Self
    where
        S: AsRef<str>,
    {
        self.default = default
            .into()
            .and_then(|s| Some(self.inner.get(s.as_ref())?.clone()));
        self
    }

    fn make_error<E: Into<Box<dyn std::error::Error + Send + Sync>>>(err: E) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, err)
    }

    pub fn get(&self, profile: &str) -> Option<&Arc<Config>> {
        self.inner.get(profile).or(self.default.as_ref())
    }

    pub fn from_reader<R: Read>(reader: R) -> io::Result<Profiles> {
        let cfg: Value = serde_json::from_reader(reader)?;

        let profiles = cfg
            .as_object()
            .ok_or_else(|| Self::make_error("profiles object parse failed".to_string()))?;
        let mut configs = ProfilesInner::with_capacity(profiles.len());
        // Read defined profiles
        for (name, v) in profiles.iter() {
            let mut sendfile_method = Default::default();
            let mut langtags_dir = Default::default();
            let mut sldr_dir = Default::default();

            v.as_object()
                .ok_or_else(|| Self::make_error(format!("\"{name}\": profile parse failed")))
                .and_then(|tbl| {
                    sendfile_method = tbl
                        .get("sendfile_method")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    sldr_dir = tbl["sldr"].as_str().map(PathBuf::from).ok_or_else(|| {
                        Self::make_error(format!("\"{name}\".sldr: path parse failed"))
                    })?;
                    langtags_dir =
                        tbl["langtags"].as_str().map(PathBuf::from).ok_or_else(|| {
                            Self::make_error(format!("\"{name}\".langtags: path parse failed"))
                        })?;
                    Ok(())
                })?;

            let langtags_path = langtags_dir.join("langtags.json");
            let reader = BufReader::new(File::open(&langtags_path)?);
            let langtags = LangTags::from_reader(reader).map_err(Self::make_error)?;

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

        Ok(Profiles {
            inner: configs,
            default: None,
        })
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Arc<Config>)> + use<'_> {
        self.inner.iter()
    }

    #[inline]
    pub fn names(&self) -> impl Iterator<Item = &str> + use<'_> {
        self.inner.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::{Config, LangTags, Profiles, ProfilesInner};
    use serde_json::json;

    #[test]
    fn unreadable_config() {
        let res = Profiles::from_reader(&br"hang on this isn't JSON!"[..])
            .err()
            .expect("io::Error: Invlalid data.");
        assert_eq!(res.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(res.to_string(), "expected value at line 1 column 1");
    }

    #[test]
    fn missing_langtags() {
        let res = Profiles::from_reader(
            json!(
                {
                    "production": {
                        "langtags": "/data/",
                        "sldr": "/data/sldr/"
                    }
                }
            )
            .to_string()
            .as_bytes(),
        )
        .err()
        .expect("io:Error: Not found during profiles::from_reader.");
        assert_eq!(res.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn valid_langtags() {
        let res = Profiles::from_reader(
            json!(
                {
                    "staging": {
                        "langtags": "tests/short/",
                        "sldr": "/staging/data/sldr/"
                    },
                    "production": {
                        "sendfile_method": "X-Accel-Redirect",
                        "langtags": "tests/short/",
                        "sldr": "/data/sldr/"
                    }
                }
            )
            .to_string()
            .as_bytes(),
        )
        .expect("Profiles value.");
        let langtags_json = json!([
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
            },
            {
                "full": "eka-Latn-NG",
                "iana": [ "Ekajuk" ],
                "iso639_3": "eka",
                "name": "Ekajuk",
                "names": [ "Akajo", "Akajuk" ],
                "region": "NG",
                "regionname": "Nigeria",
                "script": "Latn",
                "sldr": true,
                "tag": "eka",
                "tags": [ "eka-Latn", "eka-NG" ],
                "windows": "eka-Latn"
            },
            {
                "full": "eka-Latn-NG-x-ekajuk",
                "iana": [ "Ekajuk" ],
                "iso639_3": "eka",
                "name": "Ekajuk",
                "names": [ "Akajo", "Akajuk" ],
                "region": "NG",
                "regionname": "Nigeria",
                "script": "Latn",
                "sldr": true,
                "tag": "eka-Latn-NG-x-ekajuk",
                "windows": "eka-Latn-NG-x-ekajuk"
            },
            {
                "full": "frm-Latn-FR",
                "iana": [ "Middle French (ca. 1400-1600)" ],
                "iso639_3": "frm",
                "name": "Middle French (ca. 1400-1600)",
                "region": "FR",
                "regionname": "France",
                "regions": [ "BE" ],
                "script": "Latn",
                "sldr": false,
                "tag": "frm",
                "tags": [ "frm-FR", "frm-Latn" ],
                "variants": [ "1606nict" ],
                "windows": "frm-Latn"
            },
            {
                "full": "thv-Latn-DZ",
                "iana": [ "Tahaggart Tamahaq" ],
                "iso639_3": "thv",
                "localname": "Tamahaq",
                "localnames": [ "Tamahaq" ],
                "macrolang": "tmh",
                "name": "Tamahaq, Tahaggart",
                "names": [ "Tahaggart Tamahaq", "Tamachek", "Tamachek’", "Tamahaq", "Tamashekin", "Tamasheq", "Tomachek", "Touareg", "Tourage", "Toureg", "Tuareg" ],
                "region": "DZ",
                "regionname": "Algeria",
                "regions": [ "LY", "NE" ],
                "script": "Latn",
                "sldr": true,
                "tag": "thv",
                "tags": [ "thv-DZ", "thv-Latn" ],
                "windows": "thv-Latn"
            },
            {
                "full": "thv-Latn-DZ-x-ahaggar",
                "iana": [ "Tahaggart Tamahaq" ],
                "iso639_3": "thv",
                "localname": "Tamahaq",
                "localnames": [ "Tamahaq" ],
                "macrolang": "tmh",
                "name": "Tamahaq, Tahaggart",
                "names": [ "Tahaggart Tamahaq", "Tamachek", "Tamachek’", "Tamahaq", "Tamashekin", "Tamasheq", "Tomachek", "Touareg", "Tourage", "Tuareg" ],
                "region": "DZ",
                "regionname": "Algeria",
                "script": "Latn",
                "sldr": true,
                "tag": "thv-Latn-DZ-x-ahaggar",
                "windows": "thv-Latn-DZ-x-ahaggar"
            }
        ]).to_string();
        let langtags_json = langtags_json.as_bytes();
        let mut expected = ProfilesInner::new();
        expected.insert(
            "production".into(),
            Config {
                sendfile_method: Some("X-Accel-Redirect".into()),
                langtags: LangTags::from_reader(Cursor::new(langtags_json))
                    .expect("LangTags production test case."),
                langtags_dir: "tests/short/".into(),
                sldr_dir: "/data/sldr/".into(),
            }
            .into(),
        );
        expected.insert(
            "staging".into(),
            Config {
                sendfile_method: None,
                langtags: LangTags::from_reader(Cursor::new(langtags_json))
                    .expect("LangTags staging test case."),
                langtags_dir: "tests/short/".into(),
                sldr_dir: "/staging/data/sldr/".into(),
            }
            .into(),
        );

        assert_eq!(res.inner, expected);
    }
}
