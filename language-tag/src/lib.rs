mod parser;

use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::hash::Hash;
use std::iter::FusedIterator;
use std::fmt::{Display, Write};
use std::str::FromStr;
use parser::Finish;


#[derive(Clone,Debug,Default, Eq, Ord, PartialEq, PartialOrd)]
struct Offsets {
    variants: Vec<u8>,
    extensions: Vec<u8>,
    lang: u8,
    script: u8,
    region: u8,
    variant: u8,
    extension: u8,
}

#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    Ord,
    PartialEq,
    PartialOrd,
    DeserializeFromStr,
    SerializeDisplay,
)]
pub struct Tag {    
    buf: String,
    end: Offsets
}

impl Hash for Tag {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.buf.hash(state)
    }
}

impl FromStr for Tag {
    type Err = parser::Error<String>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match parser::languagetag(s).finish() {
            Ok((_, tag)) => Ok(tag),
            Err(parser::Error { input, code }) => Err(Self::Err {
                input: input.to_owned(),
                code,
            }),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Iter<'c> {
    inner: std::slice::Iter<'c, u8>,
    data: &'c str
} 

impl<'c> Iterator for Iter<'c> {
    type Item = &'c str;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().and_then(|&len| {
            let p = self.data.split_at(len as usize);
            self.data = p.1;
            Some(p.0)
        })
    }
}

impl FusedIterator for Iter<'_> {}

impl Tag {
    fn new<'a>(
        lang: &'a str, 
        script: impl Into<Option<&'a str>>, 
        region: impl Into<Option<&'a str>>, 
        variants: impl IntoIterator<Item = &'a str>, 
        extensions: impl IntoIterator<Item = &'a str>, 
        private: impl Into<Option<&'a str>>) -> Self
    {
        let script = script.into().unwrap_or_default();
        let region = region.into().unwrap_or_default();
        let private = private.into().unwrap_or_default();
        
        let mut full = lang.to_string() + script + region;
        let variants = variants.into_iter().scan(&mut full, |stor, x| {
            **stor += x;
            Some(x.len() as u8)
        }).collect();
        let extensions: Vec<_> = extensions.into_iter().scan(&mut full,|stor, x| {
            **stor += x;
            Some(x.len() as u8)
        }).collect();
        full += private;
        let extension = (full.len() - private.len()) as u8;

        Tag {
            buf: full,
            end: Offsets {
                lang: lang.len() as u8,
                script: (lang.len() + script.len()) as u8,
                region: (lang.len() + script.len() + region.len()) as u8,
                variant: extension - extensions.iter().sum::<u8>(),
                extension,
                variants,
                extensions,
            }
        }
    }

    #[inline]
    pub fn with_lang(lang: impl AsRef<str>) -> Self {
        let mut tag = Tag::default();
        tag.set_lang(lang.as_ref());
        tag
     }

     #[inline]
     pub fn privateuse(private: &str) -> Self {
        Tag {
            buf: private.to_owned(),
            end: Default::default()
        }
    }
    
    #[inline]
    pub fn builder<'a>() -> Builder<'a> {
        Builder::default()
    }

    pub fn set_lang(&mut self, lang: &str) {
        let old = self.buf.len() as isize;
        self.buf.replace_range(..self.end.lang as usize, lang);
        let delta = (self.buf.len() as isize - old) as i8;
        self.end.lang = self.end.lang.wrapping_add_signed(delta);
        self.end.script = self.end.script.wrapping_add_signed(delta);
        self.end.region = self.end.region.wrapping_add_signed(delta);
        self.end.variant = self.end.variant.wrapping_add_signed(delta);
        self.end.extension = self.end.extension.wrapping_add_signed(delta);
    }

    pub fn set_script(&mut self, script: &str) {
        let old = self.buf.len() as isize;
        self.buf.replace_range(self.end.lang as usize..self.end.script as usize, script);
        let delta = (self.buf.len() as isize - old) as i8;
        self.end.script = self.end.script.wrapping_add_signed(delta);
        self.end.region = self.end.region.wrapping_add_signed(delta);
        self.end.variant = self.end.variant.wrapping_add_signed(delta);
        self.end.extension = self.end.extension.wrapping_add_signed(delta);
    }
    
    pub fn set_region(&mut self, region: &str) {
        let old = self.buf.len() as isize;
        self.buf.replace_range(self.end.script as usize..self.end.region as usize, region);
        let delta = (self.buf.len() as isize - old) as i8;
        self.end.region = self.end.region.wrapping_add_signed(delta);
        self.end.variant = self.end.variant.wrapping_add_signed(delta);
        self.end.extension = self.end.extension.wrapping_add_signed(delta);
    }
    
    pub fn set_variants<'a, C: AsRef<[&'a str]>>(&mut self, variants: C) {
        let variants = variants.as_ref();
        self.end.variants = variants.iter().map(|s| s.len() as u8).collect();
        let variants = variants.concat();
        let old = self.buf.len() as isize;
        self.buf.replace_range(self.end.region as usize..self.end.variant as usize, &variants);
        let delta = (self.buf.len() as isize - old) as i8;
        self.end.variant = self.end.variant.wrapping_add_signed(delta);
        self.end.extension = self.end.extension.wrapping_add_signed(delta);
    }

    pub fn set_extensions<'a, C: AsRef<[&'a str]>>(&mut self, extensions: C) {
        let extensions = extensions.as_ref();
        self.end.extensions = extensions.iter().map(|s| s.len() as u8).collect();
        let extensions = extensions.concat();
        let old = self.buf.len() as isize;
        self.buf.replace_range(self.end.variant as usize..self.end.extension as usize, &extensions);
        let delta = (self.buf.len() as isize - old) as i8;
        self.end.extension = self.end.extension.wrapping_add_signed(delta);
    }
    
    pub fn set_private(&mut self, private: &str) {
        self.buf.replace_range(self.end.extension as usize.., private);
    }

    pub fn push_variant(&mut self, variant: &str) {
        self.buf.insert_str(self.end.variant as usize, variant);
        let delta = variant.len() as u8;
        self.end.variants.push(delta);
        self.end.variant += delta;
        self.end.extension += delta;
    }

    pub fn add_extension(&mut self, extension: &str) {
        self.buf.insert_str(self.end.extension as usize, extension);
        let delta = extension.len() as u8;
        self.end.extensions.push(delta);
        self.end.extension += delta;
    }

    #[inline]
    pub fn lang(&self) -> &str {
        &self.buf[..self.end.lang as usize]
    }

    #[inline]
    pub fn script(&self) -> Option<&str> {
        let s = &self.buf[self.end.lang as usize..self.end.script as usize];
        if s.is_empty() { None } else { Some(s) }
    }

    #[inline]
    pub fn region(&self) -> Option<&str> {
        let s = &self.buf[self.end.script as usize..self.end.region as usize];
        if s.is_empty() { None } else { Some(s) }
    }

    #[inline]
    pub fn variants(&self) -> Iter {
        Iter {
            inner: self.end.variants.iter(),
            data: &self.buf[self.end.region as usize..]
        }
    }

    #[inline]
    pub fn extensions(&self) -> Iter {
        Iter {
            inner: self.end.extensions.iter(),
            data: &self.buf[self.end.variant as usize..]
        }
    }

    #[inline]
    pub fn private(&self) -> Option<&str> {
        let s = &self.buf[self.end.extension as usize..];
        if s.is_empty() { None } else { Some(s) }
    }

    #[inline]
    pub fn is_privateuse(&self) -> bool {
        self.end.extension == 0 && !self.buf.is_empty()
    }
}    

#[derive(Default, Debug)]
pub struct Builder<'a> {
    lang: &'a str,
    script: &'a str,
    region: &'a str,
    variants: Vec<&'a str>,
    extensions: Vec<&'a str>,
    private: &'a str,
}

impl<'a> From<&'a Tag> for Builder<'a> {
    fn from(value: &'a Tag) -> Self {
        Builder {
            lang: value.lang(),
            script: value.script().unwrap_or_default(),
            region: value.region().unwrap_or_default(),
            variants: value.variants().collect(),
            extensions: value.extensions().collect(),
            private: value.private().unwrap_or_default(),
        }
    }
}

impl<'a> Builder<'a> {
    #[inline]
    pub fn lang(mut self, lang: &'a str) -> Self {
        self.lang = lang.as_ref();
        self
    }
    
    #[inline]
    pub fn script(mut self, script: &'a str) -> Self {
        self.script = script.as_ref();
        self
    }
    
    #[inline]
    pub fn region(mut self, region: &'a str) -> Self {
        self.region = region.as_ref();
        self
    }
    
    #[inline]
    pub fn private(mut self, private: &'a str) -> Self {
        self.private = private.as_ref();
        self
    }

    #[inline]
    pub fn variant(mut self, variant: &'a str) -> Self {
        self.variants.push(variant.as_ref());
        self
    }

    #[inline]
    pub fn extension(mut self, extension: &'a str) -> Self {
        self.extensions.push(extension.as_ref());
        self
    }

    pub fn variants<C: AsRef<[&'a str]>>(mut self, c: C) -> Self {
        self.variants = c.as_ref().to_owned();
        self
    }

    pub fn extensions<C: AsRef<[&'a str]>>(mut self, c: C) -> Self {
        self.extensions = c.as_ref().to_owned();
        self
    }

    
    pub fn build(mut self) -> Tag {
        self.extensions.sort_unstable();
        Tag::new(
            self.lang,
            to_option(self.script),
            to_option(self.region),
            self.variants,
            self.extensions,
            to_option(self.private),
        )
    }
}

#[inline]
fn to_option(s: &str) -> Option<&str> {
    if s.is_empty() { None } else { Some(s) }
}

impl Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.lang())?;
        if let Some(script) = self.script() {
            f.write_char('-').and(f.write_str(script))?;
        }
        if let Some(region) = self.region() {
            f.write_char('-').and(f.write_str(region))?;
        }
        for v in self.variants() {
            f.write_char('-').and(f.write_str(v))?;
        }
        for v in self.extensions() {
            f.write_char('-').and(f.write_str(v))?;
        }
        if let Some(private) = self.private() {
            if !self.lang().is_empty() {
                f.write_char('-')?;
            }
            f.write_str(private)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Tag;

    #[test]
    fn constructors() {
        assert_eq!(Tag::default(), Tag::new("", None, None, [], [], None));
        assert_eq!(Tag::with_lang("en"), Tag::new("en", None, None, [], [], None));
        assert_eq!(Tag::privateuse("x-priv"), Tag::new("", None, None, [], [], "x-priv"));
    }

    #[test]
    fn setters() {
        // Test each in isolation
        let mut tag = Tag::default(); tag.set_lang("en");
        assert_eq!(tag, Tag::new("en", None, None, [], [], None));
        let mut tag = Tag::default(); tag.set_script("Latn");
        assert_eq!(tag, Tag::new("", "Latn", None, [], [], None));
        let mut tag = Tag::default(); tag.set_region("US");
        assert_eq!(tag, Tag::new("", None, "US", [], [], None));
        let mut tag = Tag::default(); tag.set_variants(["2abc", "1cde"]);
        assert_eq!(tag, Tag::new("", None, None, ["2abc", "1cde"], [], None));
        let mut tag = Tag::default(); tag.set_extensions(["a-vari", "q-abcdef"]);
        assert_eq!(tag, Tag::new("", None, None, [], ["a-vari", "q-abcdef"], None));
        let mut tag = Tag::default(); tag.push_variant("2abc");
        assert_eq!(tag, Tag::new("", None, None, ["2abc"], [], None));
        let mut tag = Tag::default(); tag.add_extension("a-var1");
        assert_eq!(tag, Tag::new("", None, None, [], ["a-var1"], None));
        let mut tag = Tag::default(); tag.set_private("x-priv");
        assert_eq!(tag, Tag::new("", None, None, [], [], "x-priv"));

        // Test cumlatively
        let mut tag = Tag::with_lang("en");
        tag.set_script("Latn");
        assert_eq!(tag, Tag::new("en", "Latn", None, [], [], None));
        tag.set_region("US");
        assert_eq!(tag, Tag::new("en", "Latn", "US", [], [], None));
        tag.set_variants(["1abc", "2def"]);
        assert_eq!(tag, Tag::new("en", "Latn", "US", ["1abc", "2def"], [], None));
        tag.push_variant("3ghi");
        assert_eq!(tag, Tag::new("en", "Latn", "US", ["1abc", "2def", "3ghi"], [], None));
        tag.set_extensions(["a-abcdef", "b-ghijklmn"]);
        assert_eq!(tag, Tag::new("en", "Latn", "US", ["1abc", "2def", "3ghi"], ["a-abcdef", "b-ghijklmn"], None));
        tag.add_extension("c-tester");
        assert_eq!(tag, Tag::new("en", "Latn", "US", ["1abc", "2def", "3ghi"], ["a-abcdef", "b-ghijklmn", "c-tester"], None));
        tag.set_private("x-priv");
        assert_eq!(tag, Tag::new("en", "Latn", "US", ["1abc", "2def", "3ghi"], ["a-abcdef", "b-ghijklmn", "c-tester"], "x-priv"));

        tag.set_script("");
        assert_eq!(tag, Tag::new("en", None, "US", ["1abc", "2def", "3ghi"], ["a-abcdef", "b-ghijklmn", "c-tester"], "x-priv"));
    }

    #[test]
    fn builder() {
        let tag = Tag::builder()
            .lang("en")
            .script("Latn")
            .region("US")
            .variant("2abc")
            .extensions(["a-bable", "q-babbel"])
            .build();
        assert_eq!(
            tag,
            Tag::new("en", "Latn", "US", ["2abc"], ["a-bable", "q-babbel"], None),
        );
    }

    #[test]
    fn from_str() {
        use std::str::FromStr;

        assert_eq!(
            Tag::from_str("en-Latn-US").ok().expect("Ok value not found"),
            Tag::builder().lang("en").region("US").script("Latn").build()
        );

        assert_eq!(
            Tag::from_str("en-Latn-USA").err().expect("Err value not found").to_string(),
            "error Tag at: en-Latn-USA"
        );
    }

    #[test]
    fn display() {
        let mut tag = Tag::with_lang("en-aaa-ccc");
        tag.set_script("Latn");
        tag.set_region("US");
        tag.push_variant("2abc");
        tag.push_variant("what2");
        tag.set_extensions(["a-bable", "q-babbel"]);
        tag.set_private("x-priv1");
        println!("{tag:?} failed as {tag}");
        assert_eq!(
            tag.to_string(),
            "en-aaa-ccc-Latn-US-2abc-what2-a-bable-q-babbel-x-priv1"
        );
    }

    #[test]
    fn sorting() {
        let aa = Tag::with_lang("aa");
        let aa_et = Tag::builder().lang("aa").region("ET").build();
        let aa_latn = Tag::builder().lang("aa").script("Latn").build();
        let aa_latn_et = Tag::builder().lang("aa").script("Latn").region("ET").build();
        let standard = [&aa, &aa_et, &aa_latn, &aa_latn_et];
        let mut test = [&aa_latn_et, &aa, &aa_et, &aa_latn];
        test.sort();

        assert_eq!(test, standard);
    }
}
