use std::{
    collections::{ HashMap, HashSet, hash_map },
    error::Error,
    fmt::Display,
    io::{ self, Read },
    ops::{ Index, Deref, DerefMut },
};
use crate::tag::Tag;

#[derive(Clone,Debug,Eq,PartialEq)]
pub struct TagSet(HashSet<Tag>);

type TagSetRef = u32;

#[derive(Debug, PartialEq)]
pub struct LangTags
{
    tagsets: Vec<TagSet>,
    map: HashMap<Tag, TagSetRef>,
}


impl LangTags {
    pub fn from_reader<R: Read>(mut reader: R) -> io::Result<Self> {
        fn into_io_error<E>(error: E)-> io::Error 
        where E: Into<Box<dyn Error + Send + Sync>> {
            io::Error::new(io::ErrorKind::InvalidData, error)
        }

        let parse = |s: &str| s.trim().trim_start_matches('*').parse::<Tag>();
        let mut buf = String::new();
        reader.read_to_string(&mut buf)?;

        let mut langtags = LangTags { 
            tagsets: Default::default(), 
            map: Default::default()
        };
        for parses in buf.lines().filter(|l| !l.trim().is_empty()).map(|l| l.split('=').map(parse)) {
            let tagset = TagSet(parses.collect::<Result<HashSet<Tag>, _>>()
                .map_err(into_io_error)?);
            langtags.add_tagset(tagset);
        }
        Ok(langtags)
    }
    
    pub fn get(&self, k: &Tag) -> Option<&TagSet> {
        self.map.get(k).and_then(|&i| self.tagsets.get(i as usize))
    }

    pub fn iter(&self) -> Iter {
        Iter { inner: self.map.iter(), tagsets: &self.tagsets }
    }

    fn add_tagset(&mut self, ts: TagSet) {
        let i = self.tagsets.len() as TagSetRef;
        self.map.extend(ts.iter().cloned().map(|t| (t, i)));
        self.tagsets.push(ts);
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
    tagsets: &'a Vec<TagSet>
}

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a Tag, &'a TagSet);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(k,&i)| (k, &self.tagsets[i as usize]))
    }
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
            .reduce(|accum, item| accum + "=" + &item)
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
     };
    use crate::tag::Tag;
    use super::{ LangTags, TagSet };

    #[test]
    fn invalid_tag() {
        let test = LangTags::from_reader(&b"#*aa = *aa-ET = aa-Latn = aa-Latn-ET"[..]).err().unwrap();
        assert_eq!(test.kind(), io::ErrorKind::InvalidData);
        assert_eq!(test.to_string(), "Parsing Error: (\"#*aa\", Tag)");
    }

    #[test]
    fn load_minimal_langtags() {
        let test = LangTags::from_reader(&br#"
            *aa = *aa-ET = aa-Latn = aa-Latn-ET
            aa-Arab = aa-Arab-ET"#[..]).ok().unwrap();
        let mut expected = LangTags { tagsets: Default::default(), map: Default::default()};
        expected.add_tagset(TagSet(HashSet::from_iter([
            Tag::lang("aa"), 
            Tag::lang("aa").region("ET"), 
            Tag::lang("aa").script("Latn"),
            Tag::lang("aa").script("Latn").region("ET")])));
        expected.add_tagset(TagSet(HashSet::from_iter([
            Tag::lang("aa").script("Arab"), 
            Tag::lang("aa").script("Arab").region("ET")])));

        assert_eq!(test, expected);
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
                "aa-Arab-ET: aa-Arab=aa-Arab-ET",
                "aa-Arab: aa-Arab=aa-Arab-ET",
                "aa-ET: aa=aa-ET=aa-Latn=aa-Latn-ET",
                "aa-Latn-ET: aa=aa-ET=aa-Latn=aa-Latn-ET",
                "aa-Latn: aa=aa-ET=aa-Latn=aa-Latn-ET",
                "aa: aa=aa-ET=aa-Latn=aa-Latn-ET",
            ]);
    }
}