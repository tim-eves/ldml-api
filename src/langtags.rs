use std::{
    collections::{ HashMap, HashSet },
    error::Error,
    fmt::Display,
    io::{ self, Read },
    ops::{ Deref, DerefMut },
    sync::Arc
};
use crate::tag::Tag;

#[derive(Debug,Eq,PartialEq)]
pub struct TagSet(pub HashSet<Tag>);

type LangTagsInner = HashMap<Tag, Arc<TagSet>>;

#[derive(Debug)]
pub struct LangTags(pub LangTagsInner);


impl LangTags {
    pub fn from_reader<R: Read>(mut reader: R) -> io::Result<Self> {
        fn into_io_error<E>(error: E)-> io::Error 
        where E: Into<Box<dyn Error + Send + Sync>> {
            io::Error::new(io::ErrorKind::InvalidData, error)
        }

        let parse = |s: &str| s.trim().trim_start_matches('*').parse::<Tag>();
        let mut buf = String::new();
        reader.read_to_string(&mut buf)?;

        let mut map = LangTagsInner::new();
        for parses in buf.lines().map(|l| l.split('=').map(parse)) {
            let tagset = Arc::new(TagSet(parses.collect::<Result<HashSet<Tag>, _>>()
                .map_err(into_io_error)?));
            map.extend(tagset.iter().cloned().map(|t| (t, tagset.clone())));
        }
        Ok(LangTags(map))
    }
}

impl Deref for LangTags {
    type Target = HashMap<Tag, Arc<TagSet>>;

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl Deref for TagSet {
    type Target = HashSet<Tag>;

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl DerefMut for TagSet {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl Display for TagSet {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut tagset: Vec<_> = self.iter().collect();
        tagset.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut s = tagset.iter().fold(
            String::new(), 
            |s, ts| s + &ts.to_string() + "=");
        s.pop();
        f.write_str(&s)
    }
}