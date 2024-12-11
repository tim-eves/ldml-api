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

#[derive(Debug, Eq, PartialEq)]
pub struct LangTags {
    tagsets: Vec<TagSet>,
    tagmap: HashMap<Tag, TagSetRef>,
    scripts: HashSet<String>,
    regions: HashSet<String>,
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

        let mut scripts: HashSet<String> = Default::default();
        let mut regions: HashSet<String> = Default::default();
        let tagmap: HashMap<Tag, TagSetRef> =
            tagsets
                .iter()
                .enumerate()
                .fold(Default::default(), |mut m, (i, ts)| {
                    scripts.extend(ts.iter().filter_map(|t| t.script().map(str::to_string)));
                    regions.extend(ts.iter().filter_map(|t| t.region().map(str::to_string)));
                    m.extend(ts.iter().cloned().map(|t| (t, i as TagSetRef)));
                    m
                });
        Ok(LangTags {
            tagsets,
            tagmap,
            scripts,
            regions,
        })
    }

    pub fn conformant(&self, tag: &Tag) -> bool {
        let valid_script = tag
            .script()
            .map(|s| self.scripts.contains(s))
            .unwrap_or(true);
        let valid_region = tag
            .region()
            .map(|s| self.regions.contains(s))
            .unwrap_or(true);
        valid_script && valid_region
    }

    pub fn get(&self, k: &Tag) -> Option<&TagSet> {
        self.tagmap.get(k).map(|&i| &self.tagsets[i as usize])
    }

    pub fn orthographic_normal_form(&self, tag: &Tag) -> Option<&TagSet> {
        self.get(tag).or(if tag.region().is_none() {
            None
        } else {
            let mut t = tag.to_owned();
            t.set_region("");
            self.get(&t)
        })
    }

    pub fn locale_normal_form(&self, tag: &Tag) -> Option<TagSet> {
        self.orthographic_normal_form(tag).map(|ortho_tagset| {
            if let Some(region) = tag.region() {
                TagSet(
                    ortho_tagset
                        .0
                        .iter()
                        .filter_map(|tag| {
                            if tag.region().is_none() {
                                None
                            } else {
                                let mut tag = tag.to_owned();
                                tag.set_region(region);
                                Some(tag)
                            }
                        })
                        .collect::<HashSet<Tag>>(),
                )
            } else {
                ortho_tagset.clone()
            }
        })
    }

    pub fn iter(&self) -> Iter {
        Iter {
            inner: self.tagmap.iter(),
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
        self.tagsets.index(*self.tagmap.index(tag) as usize)
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
        tagset.sort_unstable();
        f.write_str(&crate::tagset::render_equivalence_set(tagset))
    }
}

#[cfg(test)]
mod test {
    use super::{LangTags, TagSet};
    use language_tag::Tag;
    use std::{collections::HashMap, io};

    #[test]
    fn invalid_tagset() {
        let test = LangTags::from_reader(&b"#*aa = *aa-ET = aa-Latn = aa-Latn-ET"[..])
            .err()
            .expect("io::Error from langtags test case parse.");
        assert_eq!(test.kind(), io::ErrorKind::InvalidData);
        assert_eq!(test.to_string(), "error Tag at: #*aa ");
    }

    #[test]
    fn load_minimal_langtags() {
        let test = LangTags::from_reader(
            &br#"
            *aa = *aa-ET = aa-Latn = aa-Latn-ET
            aa-Arab = aa-Arab-ET"#[..],
        )
        .expect("LangTags test case.");

        let tagsets = vec![
            TagSet(
                [
                    Tag::with_lang("aa"),
                    Tag::builder().lang("aa").region("ET").build(),
                    Tag::builder().lang("aa").script("Latn").build(),
                    Tag::builder()
                        .lang("aa")
                        .script("Latn")
                        .region("ET")
                        .build(),
                ]
                .into(),
            ),
            TagSet(
                [
                    Tag::builder().lang("aa").script("Arab").build(),
                    Tag::builder()
                        .lang("aa")
                        .script("Arab")
                        .region("ET")
                        .build(),
                ]
                .into(),
            ),
        ];
        let tagmap: HashMap<Tag, super::TagSetRef> =
            tagsets
                .iter()
                .enumerate()
                .fold(Default::default(), |mut m, (i, ts)| {
                    m.extend(ts.iter().cloned().map(|t| (t, i as super::TagSetRef)));
                    m
                });

        assert_eq!(
            test,
            LangTags {
                tagsets,
                tagmap,
                scripts: ["Arab".into(), "Latn".into()].into(),
                regions: ["ET".into()].into()
            }
        );
    }

    #[test]
    fn display_trait() {
        let mut test: Vec<_> = LangTags::from_reader(
            &br#"
            *aa = *aa-ET = aa-Latn = aa-Latn-ET
            aa-Arab = aa-Arab-ET"#[..],
        )
        .expect("LangTags test case.")
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
            ]
        );
    }
}
