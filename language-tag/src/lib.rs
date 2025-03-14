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
    private: Vec<&'a str>,
}

impl<'a> From<&'a Tag> for Builder<'a> {
    fn from(value: &'a Tag) -> Self {
        Builder {
            lang: value.lang(),
            script: value.script().unwrap_or_default(),
            region: value.region().unwrap_or_default(),
            variants: value.variants().collect(),
            extensions: value.extensions().map(|e| e.to_string()).collect(),
            private: value.private().collect(),
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
        self.private.push(private);
        self
    }

    #[inline]
    pub fn variant(mut self, variant: &'a str) -> Self {
        self.variants.push(variant);
        self
    }

    #[inline]
    pub fn extension(mut self, extension: &'a str) -> Self {
        self.extensions.push(extension.to_owned().into());
        self
    }

    pub fn variants<C: IntoIterator<Item = &'a str>>(mut self, c: C) -> Self {
        self.variants.extend(c);
        self
    }

    pub fn extensions<C: IntoIterator<Item = &'a str>>(mut self, c: C) -> Self {
        self.extensions.extend(c.into_iter().map(Into::into));
        self
    }

    pub fn private_names<C: IntoIterator<Item = &'a str>>(mut self, c: C) -> Self {
        self.private.extend(c);
        self
    }

    pub fn build(mut self) -> Tag {
        self.variants.sort_unstable();
        self.extensions.sort_unstable();
        self.private.sort_unstable();
        if !self.private.is_empty() {
            self.private.insert(0, "x");
        }
        let mut tag = Tag::from_parts(
            self.lang,
            Builder::to_option(self.script),
            Builder::to_option(self.region),
            self.variants,
            self.extensions.iter().map(AsRef::<str>::as_ref),
            self.private,
        );
        tag.shrink_to_fit();
        tag
    }

    #[inline(always)]
    fn to_option(s: &str) -> Option<&str> {
        (!s.is_empty()).then_some(s)
    }
}
