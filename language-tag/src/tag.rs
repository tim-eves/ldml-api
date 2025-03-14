use crate::{Builder, TagBuffer};
use std::{
    fmt::{Display, Write},
    hash::Hash,
    iter::{once, FusedIterator},
    num::NonZeroUsize,
    str::SplitTerminator,
};

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
    fn join_subtags(subtags: impl IntoIterator<Item = impl AsRef<str>>) -> String {
        let mut subtags = subtags.into_iter();
        let mut buffer: String = subtags
            .next()
            .map(|s| s.as_ref().into())
            .unwrap_or_default();
        for subtag in subtags {
            buffer.push('-');
            buffer.push_str(subtag.as_ref());
        }
        buffer
    }

    pub(crate) fn new(
        full: &str,
        lang: usize,
        script: impl Into<Option<NonZeroUsize>>,
        region: impl Into<Option<NonZeroUsize>>,
        variants: impl IntoIterator<Item = NonZeroUsize>,
        extensions: impl IntoIterator<Item = NonZeroUsize>,
        private: impl IntoIterator<Item = NonZeroUsize>,
    ) -> Self {
        if lang == 0 && private.into_iter().next().is_some() {
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

    pub fn set_lang(&mut self, lang: impl AsRef<str>) {
        let old = self.buf.len() as isize;
        self.buf
            .replace_range(..self.end.lang as usize, lang.as_ref());
        self.end.adjust_lang(self.buf.len() as isize - old);
    }

    #[inline]
    pub fn clear_lang(&mut self) {
        self.set_lang("");
    }

    pub fn set_script(&mut self, script: impl AsRef<str>) {
        let script = script.as_ref();
        let old = self.buf.len() as isize;
        let range = component_range!(self, script);
        self.buf.replace_range(range, script);
        self.end.adjust_script(self.buf.len() as isize - old);
    }

    #[inline]
    pub fn clear_script(&mut self) {
        self.set_script("");
    }

    pub fn set_region(&mut self, region: impl AsRef<str>) {
        let region = region.as_ref();
        let old = self.buf.len() as isize;
        let range = component_range!(self, region);
        self.buf.replace_range(range, region);
        self.end.adjust_region(self.buf.len() as isize - old);
    }

    #[inline]
    pub fn clear_region(&mut self) {
        self.set_region("");
    }

    pub fn set_variants(&mut self, variants: impl IntoIterator<Item = impl AsRef<str>>) {
        let variants = Tag::join_subtags(variants);
        let old = self.buf.len() as isize;
        let range = component_range!(self, variants);
        self.buf.replace_range(range, &variants);
        self.end.adjust_variants(self.buf.len() as isize - old);
    }

    #[inline]
    pub fn clear_variants(&mut self) {
        self.set_variants([] as [&str; 0]);
    }

    fn build_extensions(mut extensions: impl Iterator<Item = impl AsRef<str>>) -> TagBuffer {
        let mut buf: TagBuffer = extensions
            .next()
            .map(|s| s.as_ref().into())
            .unwrap_or_default();
        let mut ns = buf.chars().next().unwrap_or_default();
        for ext in extensions {
            let ext = ext.as_ref();
            let er = ExtensionRef::try_from(ext).expect("should be an extension");
            buf.push('-');
            buf.push_str(if ns == er.namespace {
                er.name
            } else {
                ns = er.namespace;
                ext
            });
        }
        buf
    }

    #[track_caller]
    pub fn set_extensions(&mut self, extensions: impl IntoIterator<Item = impl AsRef<str>>) {
        let extensions = Self::build_extensions(extensions.into_iter());
        let old = self.buf.len() as isize;
        let range = component_range!(self, extensions);
        self.buf.replace_range(range, &extensions);
        self.end.adjust_extensions(self.buf.len() as isize - old);
    }

    #[inline]
    pub fn clear_extensions(&mut self) {
        self.set_extensions([] as [&str; 0]);
    }

    pub fn set_private(&mut self, private: impl IntoIterator<Item = impl AsRef<str>>) {
        let mut private = Tag::join_subtags(private);
        let range = component_range!(self, private);
        if !private.is_empty() {
            private.insert_str(0, "x-");
        }
        self.buf.replace_range(range, &private);
    }

    #[inline]
    pub fn clear_private(&mut self) {
        self.set_private([] as [&str; 0]);
    }

    #[track_caller]
    pub fn push_variant(&mut self, variant: impl AsRef<str>) {
        let old = self.buf.len() as isize;
        self.buf.insert(self.end.variants as usize, '-');
        self.buf
            .insert_str(self.end.variants as usize + 1, variant.as_ref());
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
    pub fn has_extension(&self, extension: impl AsRef<str>) -> bool {
        self.find_extension(extension.as_ref()).is_ok()
    }

    pub fn add_extension(&mut self, extension: impl AsRef<str>) {
        if let Err((pos, extension)) = self.find_extension(extension.as_ref()) {
            let old = self.buf.len() as isize;
            self.buf.insert(pos, '-');
            self.buf.insert_str(pos + 1, extension);
            self.end.adjust_extensions(self.buf.len() as isize - old);
        }
    }

    pub fn remove_extension(&mut self, extension: impl AsRef<str>) -> bool {
        if let Ok((start, extension)) = self.find_extension(extension.as_ref()) {
            let old = self.buf.len() as isize;
            self.buf
                .replace_range(start - 1..start + extension.len(), "");
            self.end.adjust_extensions(self.buf.len() as isize - old);
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn add_private(&mut self, private: impl AsRef<str>) {
        self.add_extension("x-".to_owned() + private.as_ref());
    }

    #[inline]
    pub fn remove_private(&mut self, private: impl AsRef<str>) -> bool {
        self.remove_extension("x-".to_owned() + private.as_ref())
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
    pub fn variants(&self) -> Subtags {
        let mut range = self.end.region as usize..self.end.variants as usize;
        if !range.is_empty() {
            range.start += 1;
        }
        Subtags::new(&self.buf[range])
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
    pub fn private(&self) -> Subtags {
        let mut range = self.end.extensions as usize..self.buf.len();
        if !range.is_empty() {
            range.start += 3;
        }
        Subtags::new(&self.buf[range])
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

// Subtags iterator
#[derive(Clone, Debug)]
pub struct Subtags<'c>(SplitTerminator<'c, char>);

impl<'c> Subtags<'c> {
    #[inline]
    fn new<'a: 'c>(subtags: &'a str) -> Self {
        Subtags(subtags.split_terminator('-'))
    }
}

impl<'c> Iterator for Subtags<'c> {
    type Item = &'c str;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl FusedIterator for Subtags<'_> {}

impl DoubleEndedIterator for Subtags<'_> {
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

impl From<ExtensionRef<'_>> for TagBuffer {
    fn from(value: ExtensionRef<'_>) -> Self {
        let mut buf = TagBuffer::default();
        buf.push(value.namespace);
        buf.push('-');
        buf.push_str(value.name);
        buf
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

    // TODO: Make sure these test tags are exervised somewhere:
    // "en-Latn-US-1abc-2def-3ghi-a-abcdef-b-ghijklmn-c-tester-x-priv"
    // "en-Latn-US-1abc-2def-3ghi-a-abcdef-b-ghijklmn-c-tester"
    // "en-Latn-US-1abc-2def-3ghi"

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
