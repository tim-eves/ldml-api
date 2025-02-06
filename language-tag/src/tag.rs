use crate::Builder;
use core::panic;
use std::{
    fmt::{Display, Write},
    hash::Hash,
    iter::{once, FusedIterator},
    num::NonZeroUsize,
    str::SplitTerminator,
};

#[cfg(feature = "compact")]
use compact_str::CompactString as TagBuffer;
#[cfg(not(feature = "compact"))]
use std::string::String as TagBuffer;
#[cfg(feature = "serde")]
use {serde_with::DeserializeFromStr, serde_with::SerializeDisplay};

#[derive(Clone, Debug, Default)]
struct Offsets {
    lang: u8,
    script: u8,
    region: u8,
    variants: u8,
    extensions: u8,
}

impl Offsets {
    #[inline]
    fn adjust_lang(&mut self, delta: isize) {
        self.lang = self.lang.wrapping_add_signed(delta as i8);
        self.adjust_script(delta);
    }
    #[inline]
    fn adjust_script(&mut self, delta: isize) {
        self.script = self.script.wrapping_add_signed(delta as i8);
        self.adjust_region(delta);
    }
    #[inline]
    fn adjust_region(&mut self, delta: isize) {
        self.region = self.region.wrapping_add_signed(delta as i8);
        self.adjust_variants(delta);
    }
    #[inline]
    fn adjust_variants(&mut self, delta: isize) {
        self.variants = self.variants.wrapping_add_signed(delta as i8);
        self.adjust_extensions(delta);
    }
    #[inline]
    fn adjust_extensions(&mut self, delta: isize) {
        self.extensions = self.extensions.wrapping_add_signed(delta as i8);
    }
}

#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(DeserializeFromStr, SerializeDisplay))]
pub struct Tag {
    buf: TagBuffer,
    end: Offsets,
}

macro_rules! _component_range {
    ($self:expr, script) => {
        $self.end.lang as usize..$self.end.script as usize
    };
    ($self:expr, region) => {
        $self.end.script as usize..$self.end.region as usize
    };
    ($self:expr, variants) => {
        $self.end.region as usize..$self.end.variants as usize
    };
    ($self:expr, extensions) => {
        $self.end.variants as usize..$self.end.extensions as usize
    };
    ($self:expr, private) => {
        $self.end.extensions as usize..$self.buf.len()
    };
}

macro_rules! component_range {
    ($self:expr, $comp:ident) => {{
        let mut range = _component_range!($self, $comp);
        if !$comp.is_empty() {
            if range.is_empty() {
                $self.buf.insert(range.start, '-');
                range.end += 1;
            }
            range.start += 1;
        }
        range
    }};
}

impl Tag {
    pub(crate) fn new(
        full: &str,
        lang: usize,
        script: impl Into<Option<NonZeroUsize>>,
        region: impl Into<Option<NonZeroUsize>>,
        variants: impl IntoIterator<Item = NonZeroUsize>,
        extensions: impl IntoIterator<Item = NonZeroUsize>,
        private: impl Into<Option<NonZeroUsize>>,
    ) -> Self {
        if lang == 0 && private.into().is_some() {
            Tag::privateuse(full)
        } else {
            let mut end = Offsets {
                lang: lang as u8,
                ..Offsets::default()
            };
            end.script = end.lang + script.into().map(|s| s.get() + 1).unwrap_or_default() as u8;
            end.region = end.script + region.into().map(|s| s.get() + 1).unwrap_or_default() as u8;
            end.variants = end.region
                + variants
                    .into_iter()
                    .reduce(|a, b| a.saturating_add(b.get()).saturating_add(1))
                    .map(|s| s.get() + 1)
                    .unwrap_or_default() as u8;
            end.extensions = end.variants
                + extensions
                    .into_iter()
                    .reduce(|a, b| a.saturating_add(b.get()).saturating_add(1))
                    .map(|s| s.get() + 1)
                    .unwrap_or_default() as u8;

            Tag {
                buf: full.into(),
                end,
            }
        }
    }

    pub(crate) fn from_parts<'a>(
        lang: &'a str,
        script: impl Into<Option<&'a str>>,
        region: impl Into<Option<&'a str>>,
        variants: impl IntoIterator<Item = &'a str, IntoIter = impl Iterator<Item = &'a str> + Clone>,
        extensions: impl IntoIterator<Item = &'a str, IntoIter = impl Iterator<Item = &'a str> + Clone>,
        private: impl Into<Option<&'a str>>,
    ) -> Self {
        let private = private.into();
        if lang.is_empty() {
            if let Some(private) = private {
                return Tag::privateuse(private);
            }
        }
        let script = script.into();
        let region = region.into();
        let variants = variants.into_iter();
        let extensions = extensions.into_iter().scan("", |ns, ext| {
            let parts = ext.split_at(2);
            if parts.0 == *ns {
                Some(parts.1)
            } else {
                *ns = parts.0;
                Some(ext)
            }
        });
        let mut full = lang.to_owned();
        script
            .iter()
            .copied()
            .chain(region.iter().copied())
            .chain(variants.clone())
            .chain(extensions.clone())
            .chain(private.iter().copied())
            .for_each(|v| {
                full.push('-');
                full.push_str(v)
            });

        Tag::new(
            &full,
            lang.len(),
            script.and_then(|r| r.len().try_into().ok()),
            region.and_then(|r| r.len().try_into().ok()),
            variants.map(|v| v.len().try_into().unwrap()),
            extensions.map(|e| e.len().try_into().unwrap()),
            private.and_then(|r| r.len().try_into().ok()),
        )
    }

    #[inline]
    pub fn with_lang(lang: impl AsRef<str>) -> Self {
        let len = lang.as_ref().len() as u8;
        Tag {
            buf: lang.as_ref().into(),
            end: Offsets {
                lang: len,
                script: len,
                region: len,
                variants: len,
                extensions: len,
            },
        }
    }

    #[inline]
    pub fn privateuse(private: impl AsRef<str>) -> Self {
        Tag {
            buf: private.as_ref().into(),
            end: Default::default(),
        }
    }

    #[inline(always)]
    pub fn builder<'a>() -> Builder<'a> {
        Builder::default()
    }

    #[inline(always)]
    pub fn shrink_to_fit(&mut self) {
        self.buf.shrink_to_fit();
    }

    pub fn set_lang(&mut self, lang: &str) {
        let old = self.buf.len() as isize;
        self.buf.replace_range(..self.end.lang as usize, lang);
        self.end.adjust_lang(self.buf.len() as isize - old);
    }

    pub fn set_script(&mut self, script: &str) {
        let old = self.buf.len() as isize;
        let range = component_range!(self, script);
        self.buf.replace_range(range, script);
        self.end.adjust_script(self.buf.len() as isize - old);
    }

    pub fn set_region(&mut self, region: &str) {
        let old = self.buf.len() as isize;
        let range = component_range!(self, region);
        self.buf.replace_range(range, region);
        self.end.adjust_region(self.buf.len() as isize - old);
    }

    pub fn set_variants<'a>(&mut self, variants: impl AsRef<[&'a str]>) {
        let variants = variants.as_ref();
        let variants = variants.join("-");
        let old = self.buf.len() as isize;
        let range = component_range!(self, variants);
        self.buf.replace_range(range, &variants);
        self.end.adjust_variants(self.buf.len() as isize - old);
    }

    #[inline(always)]
    #[track_caller]
    fn assert_extension(subtag: &str) {
        if subtag.len() <= 4 || subtag.as_bytes()[1] != b'-' {
            panic!("subtag \"{subtag}\" is not a valid extension");
        }
    }

    #[track_caller]
    pub fn set_extensions<'a>(&mut self, extensions: impl AsRef<[&'a str]>) {
        let mut extensions = extensions.as_ref().to_vec();
        let extensions = if extensions.is_empty() {
            Default::default()
        } else {
            extensions.sort_unstable();
            let mut ns = "\0";
            for e in extensions.iter_mut() {
                Tag::assert_extension(e);
                let parts = e.split_at(2);
                if parts.0 == ns {
                    *e = parts.1;
                } else {
                    ns = parts.0;
                }
            }
            extensions.join("-")
        };

        let old = self.buf.len() as isize;
        let range = component_range!(self, extensions);
        self.buf.replace_range(range, &extensions);
        self.end.adjust_extensions(self.buf.len() as isize - old);
    }

    pub fn set_private(&mut self, private: &str) {
        let range = component_range!(self, private);
        self.buf.replace_range(range, private);
    }

    #[track_caller]
    pub fn push_variant(&mut self, variant: &str) {
        let old = self.buf.len() as isize;
        self.buf.insert(self.end.variants as usize, '-');
        self.buf.insert_str(self.end.variants as usize + 1, variant);
        self.end.adjust_variants(self.buf.len() as isize - old);
    }

    pub fn pop_variant(&mut self) -> Option<String> {
        let old = self.buf.len() as isize;
        let mut range = _component_range!(self, variants);
        let variant = self.buf[range.clone()].rsplit_once('-')?.1.to_string();
        range.start = range.end - variant.len() - 1;
        self.buf.replace_range(range, "");
        self.end.adjust_variants(self.buf.len() as isize - old);
        Some(variant)
    }

    fn find_extension<'c, 'e: 'c>(
        &'c self,
        extension: &'e str,
    ) -> Result<(usize, &'e str), (usize, &'e str)> {
        Tag::assert_extension(extension);
        let parts = extension.split_at(2);
        let range = _component_range!(self, extensions);
        let elided_extensions = &self.buf[range.clone()];
        let mut offsets = elided_extensions.match_indices('-').map(|(off, _)| off + 1);
        offsets
            .by_ref()
            .find(|&off| &elided_extensions[off..=off + 1] == parts.0)
            .and_then(|_| {
                offsets.next().map(|start| {
                    let names: Vec<(usize, &str)> = offsets
                        .chain(once(elided_extensions.len() + 1))
                        .scan(start, |start, end| {
                            let name = &elided_extensions[*start..end - 1];
                            if name.len() == 1 {
                                return None;
                            }
                            let res = (*start, name);
                            *start = end;
                            Some(res)
                        })
                        .collect();
                    names
                        .binary_search_by_key(&parts.1, |(_, name)| name)
                        .map_err(|i| {
                            (
                                range.start
                                    + if i == names.len() {
                                        names[i - 1].0 + names[i - 1].1.len()
                                    } else {
                                        names[i].0 - 1
                                    },
                                parts.1,
                            )
                        })
                        .map(|i| {
                            if names.len() == 1 {
                                (range.start + names[i].0 - 2, extension)
                            } else {
                                (range.start + names[i].0, parts.1)
                            }
                        })
                })
            })
            .unwrap_or(Err((self.end.extensions as usize, extension)))
    }

    #[inline]
    pub fn has_extension(&self, extension: &str) -> bool {
        self.find_extension(extension).is_ok()
    }

    pub fn add_extension(&mut self, extension: &str) {
        if let Err((pos, extension)) = self.find_extension(extension) {
            let old = self.buf.len() as isize;
            self.buf.insert(pos, '-');
            self.buf.insert_str(pos + 1, extension);
            self.end.adjust_extensions(self.buf.len() as isize - old);
        }
    }

    pub fn remove_extension(&mut self, extension: &str) -> bool {
        if let Ok((start, extension)) = self.find_extension(extension) {
            let old = self.buf.len() as isize;
            self.buf
                .replace_range(start - 1..start + extension.len(), "");
            self.end.adjust_extensions(self.buf.len() as isize - old);
            true
        } else {
            false
        }
    }

    #[inline(always)]
    pub fn lang(&self) -> &str {
        &self.buf[..self.end.lang as usize]
    }

    #[inline]
    pub fn script(&self) -> Option<&str> {
        let s = &self.buf[self.end.lang as usize..self.end.script as usize];
        if s.is_empty() {
            None
        } else {
            Some(&s[1..])
        }
    }

    #[inline]
    pub fn region(&self) -> Option<&str> {
        let s = &self.buf[self.end.script as usize..self.end.region as usize];
        if s.is_empty() {
            None
        } else {
            Some(&s[1..])
        }
    }

    #[inline]
    pub fn variants(&self) -> Variants {
        let mut range = self.end.region as usize..self.end.variants as usize;
        if !range.is_empty() {
            range.start += 1;
        }
        Variants::new(&self.buf[range])
    }

    #[inline]
    pub fn extensions(&self) -> Extentions {
        let mut range = self.end.variants as usize..self.end.extensions as usize;
        if !range.is_empty() {
            range.start += 1;
        }
        Extentions::new(&self.buf[range.clone()])
    }

    #[inline]
    pub fn private(&self) -> Option<&str> {
        let s = &self.buf[self.end.extensions as usize..];
        if s.is_empty() {
            None
        } else {
            Some(&s[1..])
        }
    }

    #[inline(always)]
    pub fn has_variants(&self) -> bool {
        self.end.variants != self.end.region
    }

    #[inline(always)]
    pub fn has_extensions(&self) -> bool {
        self.end.extensions != self.end.variants
    }

    #[inline]
    pub fn is_privateuse(&self) -> bool {
        self.end.extensions == 0 && !self.buf.is_empty()
    }

    #[inline]
    #[cfg(feature = "compact")]
    pub fn is_heap_allocated(&self) -> bool {
        self.buf.is_heap_allocated()
    }
}

impl AsRef<str> for Tag {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        &self.buf
    }
}

impl Display for Tag {
    #[inline(always)]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.buf)
    }
}

impl PartialEq for Tag {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.buf.eq_ignore_ascii_case(&other.buf)
    }
}

impl Eq for Tag {}

impl Hash for Tag {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.buf.to_ascii_lowercase().hash(state);
    }
}

impl Ord for Tag {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let mut this = self.clone();
        let mut other = other.clone();
        this.buf.make_ascii_lowercase();
        other.buf.make_ascii_lowercase();
        this.lang()
            .cmp(other.lang())
            .then_with(|| this.script().cmp(&other.script()))
            .then_with(|| this.region().cmp(&other.region()))
    }
}

impl PartialOrd for Tag {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// Variants iterator
#[derive(Clone, Debug)]
pub struct Variants<'c>(SplitTerminator<'c, char>);

impl<'c> Variants<'c> {
    #[inline]
    fn new<'a: 'c>(subtags: &'a str) -> Self {
        Variants(subtags.split_terminator('-'))
    }
}

impl<'c> Iterator for Variants<'c> {
    type Item = &'c str;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl FusedIterator for Variants<'_> {}

impl DoubleEndedIterator for Variants<'_> {
    #[inline(always)]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExtensionRef<'c> {
    name: &'c str,
    namespace: char,
}

impl PartialEq<&str> for ExtensionRef<'_> {
    fn eq(&self, other: &&str) -> bool {
        [self.namespace as u8, b'-'].eq(&other.as_bytes()[..2]) && self.name.eq(&other[2..])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParseExtensionError {
    InvalidNamespace,
    MissingNamespace,
    NameToLong,
}

impl std::error::Error for ParseExtensionError {}

impl Display for ParseExtensionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseExtensionError::InvalidNamespace => "non-ascii character for namespace",
            ParseExtensionError::MissingNamespace => "no namespace prefix found",
            ParseExtensionError::NameToLong => "name value must be 2-8 ascii characters long",
        }
        .fmt(f)
    }
}

impl<'c> TryFrom<&'c str> for ExtensionRef<'c> {
    type Error = ParseExtensionError;
    fn try_from(s: &'c str) -> Result<Self, Self::Error> {
        let ns = &s.as_bytes()[..2];
        match ns {
            [n, b'-'] if n.is_ascii() => {
                if s.len() > 10 || s.len() < 4 {
                    Err(ParseExtensionError::NameToLong)
                } else {
                    Ok(ExtensionRef {
                        namespace: *n as char,
                        name: &s[2..],
                    })
                }
            }
            [_, b'-'] => Err(ParseExtensionError::InvalidNamespace),
            _ => Err(ParseExtensionError::MissingNamespace),
        }
    }
}

impl Display for ExtensionRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_char(self.namespace)?;
        f.write_char('-')?;
        f.write_str(self.name)
    }
}

#[derive(Clone, Debug)]
pub struct Extentions<'c> {
    subtags: SplitTerminator<'c, char>,
    curr_ns: char,
}

impl<'c> Extentions<'c> {
    fn new<'a: 'c>(subtags: &'a str) -> Self {
        Extentions {
            subtags: subtags.split_terminator('-'),
            curr_ns: Default::default(),
        }
    }
}

impl<'c> Iterator for Extentions<'c> {
    type Item = ExtensionRef<'c>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let mut n = self.subtags.next()?;
        if n.len() == 1 {
            self.curr_ns = n.chars().next()?;
            n = self.subtags.next()?;
        }
        Some(ExtensionRef {
            name: n,
            namespace: self.curr_ns,
        })
    }
}

impl FusedIterator for Extentions<'_> {}

// impl<'c> DoubleEndedIterator for Extentions<'c> {
//     #[inline]
//     fn next_back(&mut self) -> Option<Self::Item> {

//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_parts() {
        let tag = Tag::from_parts(
            "en",
            "Latn",
            "US",
            ["1abc", "2def", "3ghi"],
            ["a-abcdef", "b-ghijklmn", "c-tester"],
            "x-priv",
        );
        assert_eq!(
            tag,
            Tag {
                buf: "en-Latn-US-1abc-2def-3ghi-a-abcdef-b-ghijklmn-c-tester-x-priv".into(),
                end: Offsets {
                    lang: 2,
                    script: 7,
                    region: 10,
                    variants: 25,
                    extensions: 54
                },
            }
        );

        let tag = Tag::from_parts(
            "en",
            "Latn",
            "US",
            ["1abc", "2def", "3ghi"],
            ["a-abcdef", "b-ghijklmn", "c-tester"],
            None,
        );
        assert_eq!(
            tag,
            Tag {
                buf: "en-Latn-US-1abc-2def-3ghi-a-abcdef-b-ghijklmn-c-tester".into(),
                end: Offsets {
                    lang: 2,
                    script: 7,
                    region: 10,
                    variants: 25,
                    extensions: 25
                },
            }
        );

        let tag = Tag::from_parts("en", "Latn", "US", ["1abc", "2def", "3ghi"], None, None);
        assert_eq!(
            tag,
            Tag {
                buf: "en-Latn-US-1abc-2def-3ghi".into(),
                end: Offsets {
                    lang: 2,
                    script: 7,
                    region: 10,
                    variants: 10,
                    extensions: 10
                },
            }
        );

        let tag = Tag::from_parts("en", "Latn", "US", None, None, None);
        assert_eq!(
            tag,
            Tag {
                buf: "en-Latn-US".into(),
                end: Offsets {
                    lang: 2,
                    script: 7,
                    region: 7,
                    variants: 7,
                    extensions: 7
                },
            }
        );

        let tag = Tag::from_parts("en", None, "US", None, None, None);
        assert_eq!(
            tag,
            Tag {
                buf: "en-US".into(),
                end: Offsets {
                    lang: 2,
                    script: 2,
                    region: 5,
                    variants: 5,
                    extensions: 5
                },
            }
        );
    }

    #[test]
    fn constructors() {
        assert_eq!(
            Tag::with_lang("en"),
            Tag {
                buf: "en".into(),
                end: Offsets {
                    lang: 2,
                    script: 2,
                    region: 2,
                    variants: 2,
                    extensions: 2
                }
            }
        );
        assert_eq!(
            Tag::privateuse("x-priv"),
            Tag {
                buf: "x-priv".into(),
                end: Offsets {
                    lang: 0,
                    script: 0,
                    region: 0,
                    variants: 0,
                    extensions: 0
                }
            }
        );
        assert_eq!(
            Tag::default(),
            Tag {
                buf: "".into(),
                end: Offsets {
                    lang: 0,
                    script: 0,
                    region: 0,
                    variants: 0,
                    extensions: 0
                }
            }
        );
    }

    #[test]
    #[cfg(feature = "compact")]
    fn compact_string() {
        use std::str::FromStr;

        let tag = Tag::from_str("en-Latn-US").unwrap();
        assert!(tag.buf.len() < 24);
        assert!(!tag.is_heap_allocated());

        let tag = Tag::from_str("eng-Latn-US-x-test1-test2").unwrap();
        assert!(tag.buf.len() >= 24);
        assert!(tag.is_heap_allocated())
    }
}
