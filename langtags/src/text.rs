use crate::{langtags::LangTags as CoreLangTags, tagset::TagSet};
use language_tag::Tag;
use std::{
    borrow::Borrow,
    fmt::Display,
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

#[derive(Debug)]
enum ErrorKind {
    IO(io::Error),
    Parse(language_tag::ParseTagError),
    TagSetToSmall,
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IO(err) => write!(f, "{err}"),
            Self::Parse(err) => write!(f, "{err}"),
            Self::TagSetToSmall => f.write_str("a tagset needs at least 2 tags"),
        }
    }
}

#[derive(Debug)]
pub struct Error {
    line: usize,
    kind: ErrorKind,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Could not parse langtags.txt data, at line {}: {}",
            self.line + 1,
            self.kind
        )
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.kind {
            ErrorKind::IO(ref err) => Some(err),
            ErrorKind::Parse(ref err) => Some(err),
            _ => None,
        }
    }
}

impl LangTags {
    pub fn from_reader<R: BufRead>(reader: R) -> Result<Self, Error> {
        let parse = |s: &str| s.trim_start_matches([' ', '*', '\t']).parse::<Tag>();

        let mut langtags = CoreLangTags {
            tagsets: reader
                .lines()
                .enumerate()
                .filter_map(|(line_no, read_line)| match read_line {
                    Ok(line) if line.trim().is_empty() => None,
                    Ok(line) => Some(
                        line.split('=')
                            .map(parse)
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(|err| Error {
                                line: line_no,
                                kind: ErrorKind::Parse(err),
                            })
                            .and_then(|mut ts| {
                                if ts.len() < 2 {
                                    return Err(Error {
                                        line: line_no,
                                        kind: ErrorKind::TagSetToSmall,
                                    });
                                }

                                ts.sort();
                                Ok(TagSet {
                                    full: ts.remove(ts.len() - 1),
                                    sldr: line.contains('*'),
                                    tag: ts.remove(0),
                                    tags: ts,
                                    ..Default::default()
                                })
                            }),
                    ),
                    Err(err) => Some(Err(Error {
                        line: line_no,
                        kind: ErrorKind::IO(err),
                    })),
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
    use std::str::FromStr;

    #[test]
    fn invalid_tagset() {
        let test = LangTags::from_reader(b"#*aa = *aa-ET = aa-Latn = aa-Latn-ET".as_slice())
            .err()
            .expect("should fail to parse mock langtags.txt");
        assert_eq!(
            test.to_string(),
            "Could not parse langtags.txt data, at line 1: failed to parse tag: #*aa"
        );
    }

    #[test]
    fn load_minimal_langtags() {
        let test = LangTags::from_reader(
            br#"
            *aa = *aa-ET = aa-Latn = aa-Latn-ET
            aa-Arab = aa-Arab-ET"#
                .as_slice(),
        )
        .expect("should fail to parse mock langtags.txt");

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
}
