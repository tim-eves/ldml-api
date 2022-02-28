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
pub struct TagSet(HashSet<Tag>);

type LangTagsInner = HashMap<Tag, Arc<TagSet>>;

#[derive(Clone, Debug, PartialEq)]
pub struct LangTags(LangTagsInner);


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
        for (i, parses) in buf.lines().filter(|l| !l.trim().is_empty()).map(|l| l.split('=').map(parse)).enumerate() {
            let tagset = Arc::new(TagSet(parses.collect::<Result<HashSet<Tag>, _>>()
                .map_err(into_io_error)?));

            if tagset.len() <= 1 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("line {}: missing, one or more, '=' in TagSet specification", i+1)));
            }
            
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
        tagset.sort();
        let s = tagset.iter()
            .map(|t| t.to_string())
            .reduce(|accum, item| accum + " = " + &item)
            .unwrap_or_default();
        f.write_str(&s)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        io,
        iter::FromIterator,
        sync::Arc
     };
    use crate::tag::Tag;
    use super::{ LangTags, LangTagsInner, TagSet };

    #[test]
    fn invalid_tag() {
        let test = LangTags::from_reader(&b"#*aa = *aa-ET = aa-Latn = aa-Latn-ET"[..]).err().unwrap();
        assert_eq!(test.kind(), io::ErrorKind::InvalidData);
        assert_eq!(test.to_string(), "Parsing Error: (\"#*aa\", Tag)");
    }


    #[test]
    fn invalid_tagset() {
        let test = LangTags::from_reader(&b"aa aa-ET aa-Latn aa-Latn-ET"[..]).err().unwrap();
        assert_eq!(test.to_string(), "line 1: missing, one or more, '=' in TagSet specification");
    }

    #[test]
    fn load_minimal_langtags() {
        let test = LangTags::from_reader(&br#"
            *aa = *aa-ET = aa-Latn = aa-Latn-ET
            aa-Arab = aa-Arab-ET"#[..]).ok().unwrap();
        let aa: Arc<TagSet> = Arc::new(TagSet(HashSet::from_iter([
            Tag::lang("aa"), 
            Tag::lang("aa").region("ET"), 
            Tag::lang("aa").script("Latn"),
            Tag::lang("aa").script("Latn").region("ET")])));
        let aa_arab: Arc<TagSet> = Arc::new(TagSet(HashSet::from_iter([
            Tag::lang("aa").script("Arab"), 
            Tag::lang("aa").script("Arab").region("ET")])));
        let mut expected = LangTagsInner::new();
        expected.extend(aa.iter().cloned().map(|t| (t, aa.clone())));
        expected.extend(aa_arab.iter().cloned().map(|t| (t, aa_arab.clone())));

        assert_eq!(test.0, expected);
    }

    #[test]
    fn display_trait() {
        let mut test: Vec<_> = LangTags::from_reader(&br#"
            *aa = *aa-ET = aa-Latn = aa-Latn-ET
            aa-Arab = aa-Arab-ET"#[..])
            .ok()
            .unwrap()
            .iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect();
        test.sort();

        assert_eq!(
            test, 
            [
                "aa-Arab-ET: aa-Arab = aa-Arab-ET",
                "aa-Arab: aa-Arab = aa-Arab-ET",
                "aa-ET: aa = aa-ET = aa-Latn = aa-Latn-ET",
                "aa-Latn-ET: aa = aa-ET = aa-Latn = aa-Latn-ET",
                "aa-Latn: aa = aa-ET = aa-Latn = aa-Latn-ET",
                "aa: aa = aa-ET = aa-Latn = aa-Latn-ET",
            ]);
    }
}