use crate::{langtags::LangTags as CoreLangTags, tagset::TagSet};
use language_tag::Tag;
use std::{
    borrow::Borrow,
    collections::BinaryHeap as Heap,
    error::Error,
    io::{self, BufRead},
    ops::{Deref, DerefMut},
};

#[derive(Debug, Default, PartialEq)]
pub struct LangTags(CoreLangTags);

impl Deref for LangTags {
    type Target = CoreLangTags;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for LangTags {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Borrow<CoreLangTags> for LangTags {
    fn borrow(&self) -> &CoreLangTags {
        &self.0
    }
}

impl LangTags {
    pub fn from_reader<R: BufRead>(reader: R) -> io::Result<Self> {
        fn into_io_error<E>(error: E) -> io::Error
        where
            E: Into<Box<dyn Error + Send + Sync>>,
        {
            io::Error::new(io::ErrorKind::InvalidData, error)
        }

        let parse = |s: &str| s.trim_start_matches([' ', '*', '\t']).parse::<Tag>();

        let mut langtags = CoreLangTags {
            tagsets: reader
                .lines()
                .filter_map(|read_line| match read_line {
                    Ok(line) if line.trim().is_empty() => None,
                    Ok(line) => Some(
                        line.split('=')
                            .map(parse)
                            .collect::<Result<Heap<Tag>, _>>()
                            .map_err(into_io_error)
                            .map(|ts| {
                                let mut ts = ts.into_sorted_vec();
                                assert!(ts.len() >= 2);
                                TagSet {
                                    full: ts.remove(ts.len() - 1),
                                    sldr: line.contains('*'),
                                    tag: ts.remove(0),
                                    tags: ts,
                                    ..Default::default()
                                }
                            }),
                    ),
                    Err(err) => Some(Err(err)),
                })
                .collect::<Result<Vec<_>, _>>()?,
            ..Default::default()
        };

        langtags.build_caches();
        langtags.shrink_to_fit();
        Ok(LangTags(langtags))
    }
}

#[cfg(test)]
mod test {
    use super::{LangTags, TagSet};
    use language_tag::Tag;
    use std::{io, str::FromStr};

    #[test]
    fn invalid_tagset() {
        let test = LangTags::from_reader(b"#*aa = *aa-ET = aa-Latn = aa-Latn-ET".as_slice())
            .err()
            .expect("io::Error from langtags test case parse.");
        assert_eq!(test.kind(), io::ErrorKind::InvalidData);
        assert_eq!(test.to_string(), "failed to parse tag: #*aa ");
    }

    #[test]
    fn load_minimal_langtags() {
        let test = LangTags::from_reader(
            br#"
            *aa = *aa-ET = aa-Latn = aa-Latn-ET
            aa-Arab = aa-Arab-ET"#
                .as_slice(),
        )
        .expect("LangTags test case.");

        let mut langtags = LangTags::default();
        langtags.tagsets = vec![
            TagSet {
                full: Tag::from_str("aa-Latn-ET").unwrap(),
                sldr: true,
                tag: Tag::with_lang("aa"),
                tags: vec![
                    Tag::from_str("aa-ET").unwrap(),
                    Tag::from_str("aa-Latn").unwrap(),
                ],
                ..Default::default()
            },
            TagSet {
                full: Tag::from_str("aa-Arab-ET").unwrap(),
                sldr: false,
                tag: Tag::from_str("aa-Arab").unwrap(),
                tags: vec![],
                ..Default::default()
            },
        ];
        langtags.build_caches();

        assert_eq!(test, langtags);
    }

    #[test]
    fn display_trait() {
        let mut test: Vec<_> = LangTags::from_reader(
            br#"
            *aa = *aa-ET = aa-Latn = aa-Latn-ET
            aa-Arab = aa-Arab-ET"#
                .as_slice(),
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
