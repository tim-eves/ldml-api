use language_tag::Tag;
use std::{
    collections::{hash_map, HashMap, HashSet},
    error::Error,
    fmt::Display,
    io::{self, BufRead},
    ops::{Deref, DerefMut, Index},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TagSet(HashSet<Tag>);

type TagSetRef = u32;

#[derive(Debug, PartialEq)]
pub struct LangTags {
    tagsets: Vec<TagSet>,
    map: HashMap<Tag, TagSetRef>,
}

impl LangTags {
    pub fn from_reader<R: BufRead>(reader: R) -> io::Result<Self> {
        fn into_io_error<E>(error: E) -> io::Error
        where
            E: Into<Box<dyn Error + Send + Sync>>,
        {
            io::Error::new(io::ErrorKind::InvalidData, error)
        }

        let parse = |s: &str| s.trim_start_matches(&[' ', '*', '\t'][..]).parse::<Tag>();
        let tagsets = reader
            .lines()
            .filter_map(|read_line| match read_line {
                Ok(line) if line.trim().is_empty() => None,
                Ok(line) => Some(
                    line.split('=')
                        .map(parse)
                        .collect::<Result<HashSet<Tag>, _>>()
                        .map(TagSet)
                        .map_err(into_io_error),
                ),
                Err(err) => Some(Err(err)),
            })
            .collect::<Result<Vec<_>, _>>()?;

        let map: HashMap<Tag, TagSetRef> =
            tagsets
                .iter()
                .enumerate()
                .fold(Default::default(), |mut m, (i, ts)| {
                    m.extend(ts.iter().cloned().map(|t| (t, i as TagSetRef)));
                    m
                });
        Ok(LangTags { tagsets, map })
    }

    pub fn get(&self, k: &Tag) -> Option<&TagSet> {
        self.map.get(k).and_then(|&i| self.tagsets.get(i as usize))
    }

    pub fn _iter(&self) -> Iter {
        Iter {
            inner: self.map.iter(),
            tagsets: &self.tagsets,
        }
    }

    pub fn tagsets(&self) -> impl Iterator<Item = &TagSet> + '_ {
        self.tagsets.iter()
    }
}

impl Index<&Tag> for LangTags {
    type Output = TagSet;

    fn index(&self, tag: &Tag) -> &Self::Output {
        &self.tagsets[self.map[tag] as usize]
    }
}

pub struct Iter<'a> {
    inner: hash_map::Iter<'a, Tag, TagSetRef>,
    tagsets: &'a Vec<TagSet>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a Tag, &'a TagSet);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|(k, &i)| (k, &self.tagsets[i as usize]))
    }
}

impl Deref for TagSet {
    type Target = HashSet<Tag>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TagSet {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Display for TagSet {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut tagset: Vec<_> = self.iter().collect();
        tagset.sort();
        let s = tagset
            .iter()
            .map(|t| t.to_string())
            .reduce(|accum, item| accum + "=" + &item)
            .unwrap_or_default();
        f.write_str(&s)
    }
}

#[cfg(test)]
mod tests {
    use super::{LangTags, TagSet};
    use language_tag::Tag;
    use std::{
        collections::{HashMap, HashSet},
        io,
        iter::FromIterator,
    };

    #[test]
    fn invalid_tag() {
        let test = LangTags::from_reader(&b"#*aa = *aa-ET = aa-Latn = aa-Latn-ET"[..])
            .err()
            .expect("io::Error from langtags test case parse.");
        assert_eq!(test.kind(), io::ErrorKind::InvalidData);
        assert_eq!(test.to_string(), "Parsing Error: (\"#*aa \", Tag)");
    }

    #[test]
    fn load_minimal_langtags() {
        let test = LangTags::from_reader(
            &br#"
            *aa = *aa-ET = aa-Latn = aa-Latn-ET
            aa-Arab = aa-Arab-ET"#[..],
        )
        .ok()
        .expect("LangTags test case.");

        let tagsets = vec![
            TagSet(HashSet::from_iter([
                Tag::lang("aa"),
                Tag::lang("aa").region("ET"),
                Tag::lang("aa").script("Latn"),
                Tag::lang("aa").script("Latn").region("ET"),
            ])),
            TagSet(HashSet::from_iter([
                Tag::lang("aa").script("Arab"),
                Tag::lang("aa").script("Arab").region("ET"),
            ])),
        ];
        let map: HashMap<Tag, super::TagSetRef> =
            tagsets
                .iter()
                .enumerate()
                .fold(Default::default(), |mut m, (i, ts)| {
                    m.extend(ts.iter().cloned().map(|t| (t, i as super::TagSetRef)));
                    m
                });

        assert_eq!(test, LangTags { tagsets, map });
    }

    #[test]
    fn display_trait() {
        let mut test: Vec<_> = LangTags::from_reader(
            &br#"
            *aa = *aa-ET = aa-Latn = aa-Latn-ET
            aa-Arab = aa-Arab-ET"#[..],
        )
        .ok()
        .expect("LangTags test case.")
        ._iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect();
        test.sort();

        assert_eq!(
            test,
            [
                "aa-Arab-ET: aa-Arab=aa-Arab-ET",
                "aa-Arab: aa-Arab=aa-Arab-ET",
                "aa-ET: aa=aa-ET=aa-Latn=aa-Latn-ET",
                "aa-Latn-ET: aa=aa-ET=aa-Latn=aa-Latn-ET",
                "aa-Latn: aa=aa-ET=aa-Latn=aa-Latn-ET",
                "aa: aa=aa-ET=aa-Latn=aa-Latn-ET",
            ]
        );
    }
}

mod json {
    use language_tag::Tag;
    use serde::Deserialize;
    use std::borrow::Cow;

    type CowStr<'a> = Cow<'a, str>;

    #[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
    #[serde(default)]
    struct TagSet<'a> {
        full: Tag,
        iana: Vec<CowStr<'a>>,
        iso639_3: CowStr<'a>,
        latnnames: Vec<CowStr<'a>>,
        localname: CowStr<'a>,
        localnames: Vec<CowStr<'a>>,
        name: CowStr<'a>,
        names: Vec<CowStr<'a>>,
        nophonvars: bool,
        obsolete: bool,
        regionname: CowStr<'a>,
        regions: Vec<CowStr<'a>>,
        rod: CowStr<'a>,
        sldr: bool,
        suppress: bool,
        tag: Tag,
        tags: Vec<Tag>,
        unwritten: bool,
        variants: Vec<CowStr<'a>>,
        windows: Tag,
    }

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    #[serde(tag = "tag")]
    enum Header<'a> {
        #[serde(rename = "_globalvar")]
        GlobalVar { variants: Vec<CowStr<'a>> },
        #[serde(rename = "_phonvar")]
        PhonVar { variants: Vec<CowStr<'a>> },
        #[serde(rename = "_version")]
        Version { api: CowStr<'a>, date: CowStr<'a> },
    }

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    #[serde(untagged)]
    enum Element<'a> {
        Header(Header<'a>),
        Record(Box<TagSet<'a>>),
    }

    #[cfg(test)]
    mod tests {
        use super::{Element, Header, TagSet};
        use language_tag::Tag;
        use std::{fs::File, io::BufReader};

        #[test]
        fn tagset() {
            let src = &r#"{
                            "full": "pt-Latn-BR",
                            "iana": [ "Portuguese" ],
                            "iso639_3": "por",
                            "localname": "português",
                            "localnames": [ "Português" ],
                            "name": "Portuguese",
                            "names": [ "Portugais", "Portugués", "Portugués del Uruguay", "Português", "Portunhol", "Portuñol", "Purtagaalee", "Uruguayan Portuguese" ],
                            "region": "BR",
                            "regionname": "Brazil",
                            "regions": [ "AD", "AG", "AU", "BE", "BM", "CA", "CG", "CW", "DE", "ES", "FI", "FR", "GG", "GY", "IN", "JE", "JM", "MW", "PY", "RU", "SN", "SR", "US", "UY", "VC", "VE", "ZA", "ZM" ],
                            "script": "Latn",
                            "sldr": true,
                            "suppress": true,
                            "tag": "pt",
                            "tags": [ "pt-BR", "pt-Latn" ],
                            "variants": [ "abl1943", "ai1990", "colb1945" ],
                            "windows": "pt-BR"
                        }"#;
            let ts: TagSet = serde_json::from_str(src).expect("TagSet value");
            assert_eq!(
                ts,
                TagSet {
                    full: Tag::lang("pt").script("Latn").region("BR"),
                    iana: vec!["Portuguese".into()],
                    iso639_3: "por".into(),
                    localname: "português".into(),
                    localnames: vec!["Português".into()],
                    name: "Portuguese".into(),
                    names: vec![
                        "Portugais".into(),
                        "Portugués".into(),
                        "Portugués del Uruguay".into(),
                        "Português".into(),
                        "Portunhol".into(),
                        "Portuñol".into(),
                        "Purtagaalee".into(),
                        "Uruguayan Portuguese".into(),
                    ],
                    regionname: "Brazil".into(),
                    regions: [
                        "AD", "AG", "AU", "BE", "BM", "CA", "CG", "CW", "DE", "ES", "FI", "FR",
                        "GG", "GY", "IN", "JE", "JM", "MW", "PY", "RU", "SN", "SR", "US", "UY",
                        "VC", "VE", "ZA", "ZM"
                    ]
                    .iter()
                    .map(|&x| x.into())
                    .collect(),
                    sldr: true,
                    suppress: true,
                    tag: Tag::lang("pt"),
                    tags: vec![Tag::lang("pt").region("BR"), Tag::lang("pt").script("Latn")],
                    variants: vec!["abl1943".into(), "ai1990".into(), "colb1945".into()],
                    windows: Tag::lang("pt").region("BR"),
                    ..Default::default()
                }
            )
        }

        #[test]
        fn langtags_header() {
            let src = r#"[            
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
                    ]"#;
            let db: Vec<Element> = serde_json::from_str(src).expect("TagSet value");
            assert_eq!(
                db[0],
                Element::Header(Header::GlobalVar {
                    variants: vec!["simple".into()]
                })
            );
            assert_eq!(
                db[1],
                Element::Header(Header::PhonVar {
                    variants: vec![
                        "alalc97".into(),
                        "fonipa".into(),
                        "fonkirsh".into(),
                        "fonnapa".into(),
                        "fonupa".into(),
                        "fonxsamp".into()
                    ]
                })
            );
            assert_eq!(
                db[2],
                Element::Header(Header::Version {
                    api: "1.2.1".into(),
                    date: "2021-06-29".into()
                })
            );
            assert_eq!(
                db[3],
                Element::Record(Box::new(TagSet {
                    full: Tag::lang("aa").script("Latn").region("ET"),
                    iana: vec!["Afar".into()],
                    iso639_3: "aar".into(),
                    localname: "Qafar".into(),
                    localnames: vec!["Qafar af".into()],
                    name: "Afar".into(),
                    regionname: "Ethiopia".into(),
                    sldr: true,
                    tag: Tag::lang("aa"),
                    tags: vec![Tag::lang("aa").region("ET"), Tag::lang("aa").script("Latn")],
                    windows: Tag::lang("aa").script("Latn").region("ET"),
                    ..Default::default()
                }))
            )
        }

        #[test]
        fn langtags_db() {
            let file = File::open("../langtags/pub/langtags.json").expect("open langtags.json");
            let mut records: Vec<Element> =
                serde_json::from_reader(BufReader::new(file)).expect("read langtags.json");
            let db: Vec<TagSet> = records
                .drain(3..)
                .map_while(|e| match e {
                    Element::Record(ts) => Some(*ts),
                    Element::Header(_) => None,
                })
                .collect();
            // for ts in db.iter() {
            //     assert!(&ts.full.lang == &ts.tag.lang);
            // }
            println!("{len} records found in DB.", len = db.len());
        }
    }
}
