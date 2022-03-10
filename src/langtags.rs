use language_tag::Tag;
use std::{
    collections::{hash_map, HashMap, HashSet},
    error::Error,
    fmt::Display,
    io::{self, Read},
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
    pub fn from_reader<R: Read>(mut reader: R) -> io::Result<Self> {
        fn into_io_error<E>(error: E) -> io::Error
        where
            E: Into<Box<dyn Error + Send + Sync>>,
        {
            io::Error::new(io::ErrorKind::InvalidData, error)
        }

        let parse = |s: &str| s.trim().trim_start_matches('*').parse::<Tag>();
        let mut buf = String::new();
        reader.read_to_string(&mut buf)?;

        let tagsets = buf
            .lines()
            .filter_map(|l| {
                if l.trim().is_empty() {
                    None
                } else {
                    Some(
                        l.split('=')
                            .map(parse)
                            .collect::<Result<HashSet<Tag>, _>>()
                            .map(TagSet),
                    )
                }
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(into_io_error)?;

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
        assert_eq!(test.to_string(), "Parsing Error: (\"#*aa\", Tag)");
    }

    #[test]
    fn load_minimal_langtags() {
        let test = LangTags::from_reader(
            &br#"
            *aa = *aa-ET = aa-Latn = aa-Latn-ET
            aa-Arab = aa-Arab-ET"#[..],
        )
        .ok()
        .unwrap();

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
        .unwrap()
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
