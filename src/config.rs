use langtags::json::LangTags;
use serde::Deserialize;
use std::{
    collections::HashMap,
    fmt::Display,
    fs::{self, File},
    io::{self, BufReader, Read},
    ops::Index,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, PartialEq, Deserialize)]
pub struct Config {
    pub sendfile_method: Option<String>,
    #[serde(skip_deserializing)]
    pub langtags: LangTags,
    #[serde(rename = "langtags")]
    pub langtags_dir: PathBuf,
    #[serde(rename = "sldr")]
    pub sldr_dir: PathBuf,
}

impl Config {
    pub fn sldr_path(&self, flat: bool) -> PathBuf {
        self.sldr_dir.join(if flat { "flat" } else { "unflat" })
    }
}

#[derive(Debug, Clone)]
pub struct Profiles {
    inner: ProfilesInner,
    default: Option<Arc<Config>>,
}

#[derive(Debug)]
enum ErrorKind {
    IO(PathBuf, io::Error),
    Json(serde_json::Error),
    LangTags(langtags::json::Error),
}

#[derive(Debug)]
pub struct Error(ErrorKind);

impl Error {
    #[inline]
    pub fn with_io_error(path: impl AsRef<Path>, err: io::Error) -> Self {
        Error(ErrorKind::IO(path.as_ref().to_owned(), err))
    }

    pub fn as_io_error(&self) -> Option<&io::Error> {
        if let ErrorKind::IO(_, ref err) = self.0 {
            Some(err)
        } else {
            None
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Error(ErrorKind::Json(value))
    }
}

impl From<langtags::json::Error> for Error {
    fn from(value: langtags::json::Error) -> Self {
        Error(ErrorKind::LangTags(value))
    }
}

impl From<ErrorKind> for Error {
    fn from(value: ErrorKind) -> Self {
        Error(value)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.0 {
            ErrorKind::IO(_, err) => Some(err),
            ErrorKind::Json(err) => Some(err),
            ErrorKind::LangTags(err) => Some(err),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            ErrorKind::IO(path, err) => {
                write!(f, "Error accessing: {path}: {err}", path = path.display())
            }
            ErrorKind::Json(err) => write!(f, "Could not parse config: {err}"),
            ErrorKind::LangTags(err) => write!(f, "{err}"),
        }
    }
}

type ProfilesInner = HashMap<String, Arc<Config>>;

impl Index<&str> for Profiles {
    type Output = Arc<Config>;

    fn index(&self, index: &str) -> &Self::Output {
        self.get(index).expect("should get config for profile")
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

    // fn make_error<E: Into<Box<dyn std::error::Error + Send + Sync>>>(err: E) -> io::Error {
    //     io::Error::new(io::ErrorKind::InvalidData, err)
    // }

    pub fn get(&self, profile: &str) -> Option<&Arc<Config>> {
        self.inner.get(profile).or(self.default.as_ref())
    }

    pub fn from_reader<R: Read>(reader: R) -> Result<Profiles, Error> {
        let configs = serde_json::from_reader::<_, HashMap<String, Config>>(reader)?
            .into_iter()
            .map(|(profile, mut config)| {
                let langtags_path = config.langtags_dir.join("langtags.json");
                // Call read_dir to check the sldr data set path exists and is accessible.
                let _ = fs::read_dir(&config.sldr_dir)
                    .map_err(|err| Error::with_io_error(&config.sldr_dir, err))?;
                let langtags_file = File::open(&langtags_path)
                    .map_err(|err| Error::with_io_error(langtags_path, err))?;
                config.langtags = LangTags::from_reader(BufReader::new(langtags_file))?;

                Ok((profile, config.into()))
            })
            .collect::<Result<ProfilesInner, Error>>()?;

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
            .expect("should not parse invalid JSON");
        assert_eq!(
            res.to_string(),
            "Could not parse config: expected value at line 1 column 1"
        );
    }

    #[test]
    fn missing_langtags() {
        let res = Profiles::from_reader(
            json!(
                {
                    "production": {
                        "langtags": "/not-here/",
                        "sldr": "tests"
                    }
                }
            )
            .to_string()
            .as_bytes(),
        )
        .expect_err("should not parse mock config.json with invalid langtags path");
        assert!(res.as_io_error().is_some(), "should be io::Error: {res}");
        assert_eq!(
            res.as_io_error().unwrap().kind(),
            std::io::ErrorKind::NotFound
        );
        assert_eq!(
            res.to_string(),
            "Error accessing: /not-here/langtags.json: No such file or directory (os error 2)"
        )
    }

    #[test]
    fn missing_sldr() {
        let res = Profiles::from_reader(
            json!(
                {
                    "production": {
                        "langtags": "tests/short",
                        "sldr": "/not-here"
                    }
                }
            )
            .to_string()
            .as_bytes(),
        )
        .expect_err("should not parse mock config.json with invalid langtags path");
        assert!(res.as_io_error().is_some(), "should be io::Error: {res}");
        assert_eq!(
            res.as_io_error().unwrap().kind(),
            std::io::ErrorKind::NotFound
        );
        assert_eq!(
            res.to_string(),
            "Error accessing: /not-here: No such file or directory (os error 2)"
        )
    }

    #[test]
    fn valid_langtags() {
        let res = Profiles::from_reader(
            json!(
                {
                    "staging": {
                        "langtags": "tests/short/",
                        "sldr": "tests"
                    },
                    "production": {
                        "sendfile_method": "X-Accel-Redirect",
                        "langtags": "tests/short/",
                        "sldr": "tests"
                    }
                }
            )
            .to_string()
            .as_bytes(),
        )
        .expect("should parse mock config.json");
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
                    .expect("should parse test langtags.json"),
                langtags_dir: "tests/short/".into(),
                sldr_dir: "tests".into(),
            }
            .into(),
        );
        expected.insert(
            "staging".into(),
            Config {
                sendfile_method: None,
                langtags: LangTags::from_reader(Cursor::new(langtags_json))
                    .expect("should parse test langtags.json"),
                langtags_dir: "tests/short/".into(),
                sldr_dir: "tests".into(),
            }
            .into(),
        );

        assert_eq!(res.inner, expected);
    }
}
