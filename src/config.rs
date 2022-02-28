use crate::langtags::LangTags;
use std::{
    collections::BTreeMap,
    path::PathBuf
};

#[derive(Debug, PartialEq)]
pub struct Config {
    sendfile_method: Option<String>,
    langtags: LangTags,
    langtags_dir: PathBuf,
    sldr_dir: PathBuf,
}

pub type Profiles = BTreeMap<String, Config>;

pub mod profiles {
    use crate::langtags::LangTags;
    use serde_json::{ Value };
    use std::{
        fs::File,
        io::{ self, Read },
        path::{ Path, PathBuf }
    };
    use super::{ Profiles, Config};

    pub fn default() -> io::Result<Profiles> {
        from("ldml-api.json")
    }

    pub fn from<P>(path: P) -> io::Result<Profiles> 
        where P: AsRef<Path>
    {
        from_reader(File::open(path)?)
    }

    fn into_parse_error(msg: &str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, format!("parse failed: {msg}"))
    }

    pub fn from_reader<R: Read>(mut reader: R) -> io::Result<Profiles> 
    {
        let cfg: Value = serde_json::from_reader(reader)?;

        let profiles = cfg.as_object()
            .ok_or_else(|| into_parse_error("profile map"))?;
        let mut configs = Profiles::new();
        // Read defined profiles
        for (name, v) in profiles.iter() {
            let mut sendfile_method = Default::default();
            let mut langtags_dir = Default::default();
            let mut sldr_dir = Default::default();

            v.as_object()
            .ok_or_else(|| into_parse_error("config object"))
            .and_then(|tbl| {
                sendfile_method = tbl.get("sendfile_method")
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

            let langtags = File::open(langtags_dir.join("langtags.txt"))
                .and_then(LangTags::from_reader)?;

            configs.insert(name.to_owned(), Config {
                sendfile_method,
                langtags,
                langtags_dir,
                sldr_dir
            });
        }
        
        Ok(configs)
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use crate::langtags::LangTags;
    use super::{Config, Profiles, profiles};

    #[test]
    fn missing_config() {
        let res = profiles::from("test/missing-config.json");
        assert_eq!(res.err().unwrap().kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn unreadable_config() {
        let res = profiles::from_reader(&br"hang this isn't JSON!"[..]).err().unwrap();
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
                 }"#[..]).err().unwrap();
        assert_eq!(res.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn valid_langtags() {
        let res = profiles::from_reader(
            &br#"{
                     "staging": {
                         "langtags": "test/",
                         "sldr": "/staging/data/sldr/"
                     },
                     "production": {
                         "sendfile_method": "X-Accel-Redirect",
                         "langtags": "test/",
                         "sldr": "/data/sldr/"
                     }
                 }"#[..]).ok().unwrap();
        let expected_langtags = LangTags::from_reader(
            &br#"*aa = *aa-ET = aa-Latn = aa-Latn-ET
                 aa-Arab = aa-Arab-ET"#[..]).unwrap();
        let mut expected = Profiles::new();
        expected.insert("production".to_owned(),
                        Config {
                            sendfile_method: Some("X-Accel-Redirect".into()),
                            langtags: expected_langtags.clone(),
                            langtags_dir: "test/".into(),
                            sldr_dir: "/data/sldr/".into()
                        });
        expected.insert("staging".to_owned(),
                        Config {
                            sendfile_method: None,
                            langtags: expected_langtags,
                            langtags_dir: "test/".into(),
                            sldr_dir: "/staging/data/sldr/".into()
                        });

        assert_eq!(res, expected);
    }
}