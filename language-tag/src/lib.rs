pub use self::parser::languagetag;

mod from_str;
pub mod parser;
pub mod tag;

#[cfg(feature = "compact")]
use compact_str::CompactString as TagBuffer;
#[cfg(not(feature = "compact"))]
use std::string::String as TagBuffer;

pub use crate::{
    from_str::ParseTagError,
    tag::{Extension, Tag},
};

#[derive(Default, Debug)]
pub struct Builder<'a> {
    lang: &'a str,
    script: &'a str,
    region: &'a str,
    variants: Vec<&'a str>,
    extensions: Vec<TagBuffer>,
    private: Vec<&'a str>,
}

impl<'a> From<&'a Tag> for Builder<'a> {
    fn from(value: &'a Tag) -> Self {
        Builder {
            lang: value.lang(),
            script: value.script().unwrap_or_default(),
            region: value.region().unwrap_or_default(),
            variants: value.variants().collect(),
            extensions: value.extensions().map(Into::into).collect(),
            private: value.private().collect(),
        }
    }
}

impl From<Builder<'_>> for Tag {
    fn from(value: Builder) -> Self {
        value.build()
    }
}

impl<'a> Builder<'a> {
    #[inline]
    pub fn lang(mut self, lang: &'a str) -> Self {
        self.lang = lang;
        self
    }

    #[inline]
    pub fn script(mut self, script: &'a str) -> Self {
        self.script = script;
        self
    }

    #[inline]
    pub fn region(mut self, region: &'a str) -> Self {
        self.region = region;
        self
    }

    #[inline]
    pub fn private(mut self, private: &'a str) -> Self {
        self.private.push(private);
        self
    }

    #[inline]
    pub fn variant(self, variant: &'a str) -> Self {
        self.variants(Some(variant))
    }

    #[track_caller]
    #[inline]
    pub fn extension(self, extension: &'a str) -> Self {
        self.extensions(Some(extension))
    }

    #[inline]
    pub fn variants<C: IntoIterator<Item = &'a str>>(mut self, c: C) -> Self {
        self.variants.extend(c);
        self
    }

    #[track_caller]
    pub fn extensions<C: IntoIterator<Item = &'a str>>(mut self, c: C) -> Self {
        self.extensions.extend(c.into_iter().map(|s| {
            Extension::try_from(s)
                .unwrap_or_else(|err| panic!("invalid extension: {err}"))
                .into()
        }));
        self
    }

    #[track_caller]
    #[inline]
    pub fn private_names<C: IntoIterator<Item = &'a str>>(mut self, c: C) -> Self {
        self.private.extend(c);
        self
    }

    pub fn build(mut self) -> Tag {
        self.variants.sort_unstable();
        self.extensions.sort_unstable();
        self.private.sort_unstable();

        let mut tag = Tag::with_lang(self.lang);
        if !self.script.is_empty() {
            tag.set_script(self.script);
        }
        if !self.region.is_empty() {
            tag.set_region(self.region);
        }
        if !self.variants.is_empty() {
            tag.set_variants(self.variants);
        }
        if !self.extensions.is_empty() {
            tag.set_extensions(self.extensions);
        }
        if !self.private.is_empty() {
            tag.set_private(self.private);
        }

        tag.shrink_to_fit();
        tag
    }
}
