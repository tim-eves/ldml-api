pub use self::parser::languagetag;

mod from_str;
pub mod parser;
pub mod tag;

pub use crate::{
    from_str::ParseTagError,
    tag::{ExtensionRef, Tag},
};

#[derive(Default, Debug)]
pub struct Builder<'a> {
    lang: &'a str,
    script: &'a str,
    region: &'a str,
    variants: Vec<&'a str>,
    extensions: Vec<String>,
    private: &'a str,
}

impl<'a> From<&'a Tag> for Builder<'a> {
    fn from(value: &'a Tag) -> Self {
        Builder {
            lang: value.lang(),
            script: value.script().unwrap_or_default(),
            region: value.region().unwrap_or_default(),
            variants: value.variants().collect(),
            extensions: value.extensions().map(|e| e.to_string()).collect(),
            private: value.private().unwrap_or_default(),
        }
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
        self.private = private;
        self
    }

    #[inline]
    pub fn variant(mut self, variant: &'a str) -> Self {
        self.variants.push(variant);
        self
    }

    #[inline]
    pub fn extension(mut self, extension: &'a str) -> Self {
        self.extensions.push(extension.to_owned());
        self
    }

    pub fn variants<C: AsRef<[&'a str]>>(mut self, c: C) -> Self {
        self.variants = c.as_ref().to_owned();
        self
    }

    pub fn extensions<C: IntoIterator<Item = impl AsRef<str>>>(mut self, c: C) -> Self {
        self.extensions = c.into_iter().map(|e| e.as_ref().into()).collect();
        self
    }

    pub fn build(mut self) -> Tag {
        self.variants.sort_unstable();
        self.extensions.sort_unstable();
        let mut tag = Tag::from_parts(
            self.lang,
            Builder::to_option(self.script),
            Builder::to_option(self.region),
            self.variants,
            self.extensions.iter().map(AsRef::<str>::as_ref),
            Builder::to_option(self.private),
        );
        tag.shrink_to_fit();
        tag
    }

    #[inline(always)]
    fn to_option(s: &str) -> Option<&str> {
        (!s.is_empty()).then_some(s)
    }
}
