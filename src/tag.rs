use serde_with::{
    DeserializeFromStr,
    SerializeDisplay,
};
use std::{
    // cmp::Ordering, 
    fmt::Display,
};

#[derive(Clone,Debug,Default,Eq,Hash,Ord,PartialEq,PartialOrd)]
#[derive(DeserializeFromStr,SerializeDisplay)]
pub struct Tag {
    pub lang: String,
    pub script: Option<String>,
    pub region: Option<String>,
    pub variant: Vec<String>,
    pub extension: Vec<String>,
    pub private: Option<String>
}

impl Tag {
    pub fn lang<T: AsRef<str>>(l: T) -> Self {
        Tag { lang: l.as_ref().to_owned(), ..Default::default() }
    }
    
    pub fn privateuse<T: AsRef<str>>(p: T) -> Self {
        Tag {private: Some(p.as_ref().to_owned()), ..Default::default() }
    }

    pub fn script<T: AsRef<str>>(mut self, s: T) -> Self {
        self.script = Some(s.as_ref().to_owned()); self
    }

    pub fn region<T: AsRef<str>>(mut self, r: T) -> Self {
        self.region = Some(r.as_ref().to_owned()); self
    }

    pub fn private<T: AsRef<str>>(mut self, p: T) -> Self {
        self.private = Some(p.as_ref().to_owned()); self
    }

    pub fn variants<T, C: AsRef<[T]>>(mut self, c: C) -> Self where T: AsRef<str> + ToOwned {
        self.variant = c.as_ref().iter().map(|s| s.as_ref().to_owned()).collect();
        self
    }

    pub fn add_variant<T: AsRef<str>>(mut self, v: T) -> Self {
        self.variant.push(v.as_ref().to_owned());
        self
    }

    pub fn extensions<T, C: AsRef<[T]>>(mut self, c: C) -> Self where T: AsRef<str> + ToOwned {
        self.extension = c.as_ref().iter().map(|s| s.as_ref().to_owned()).collect();
        self.extension.sort();
        self
    }

    pub fn add_extension<T: AsRef<str>>(mut self, x: T) -> Self {
        self.extension.push(x.as_ref().to_owned());
        self.extension.sort();
        self
    }
}

impl Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.lang)?;
        if let Some(script) = &self.script { write!(f, "-{}", &script)?; }
        if let Some(region) = &self.region { write!(f, "-{}", &region)?; }
        if !self.variant.is_empty() {
            write!(f, "-{}", &self.variant.join("-"))?; 
        }
        if !self.extension.is_empty() { 
            write!(f, "-{}", &self.extension.join("-"))?; 
        }
        if let Some(private) = &self.private { 
            if !self.lang.is_empty() { f.write_str("-")?; }
            f.write_str(private)?; 
        }
        Ok(())
    }
}

// impl Ord for Tag {
//     fn cmp(&self, other: &Self) -> Ordering {
//         self.lang.cmp(&other.lang)
//             .then(self.variant.cmp(&other.variant))
//             .then(self.extension.cmp(&other.extension))
//             .then(self.private.cmp(&other.private))
//             .then(self.script.cmp(&other.script))
//             .then(self.region.cmp(&other.region))
//     }
// }

// impl PartialOrd for Tag {
//     fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
//         if self.lang.eq(&other.lang) 
//         && self.variant.eq(&other.variant)
//         && self.extension.eq(&other.extension)
//         && self.private.eq(&other.private) {
//             let script = match (self.script.as_ref(), other.script.as_ref()) {
//                 (   a,    b) if a == b => Some(Ordering::Equal),
//                 (None,   _ ) => Some(Ordering::Less),
//                 (   _, None) => Some(Ordering::Greater),
//                 _ => None  
//             }?;

//             let region = match (self.region.as_ref(), other.region.as_ref()) {
//                 (   a,    b) if a == b => Some(Ordering::Equal),
//                 (None,   _ ) => Some(Ordering::Less),
//                 (   _, None) => Some(Ordering::Greater),
//                 _ => None  
//             }?;

//            Some(script.then(region))
//         } else { None } 
//     }
// }

mod parser {
    use super::Tag;
    use std::str::FromStr;

    extern crate nom;
    use nom::{
        IResult,
        branch::alt,
        bytes::complete::{ take_while_m_n, tag },
        character::complete::{ anychar, char, none_of },
        combinator::{ map, not, opt, peek, recognize, value, verify },
        error::{ context, ErrorKind },
        multi::{ many0, many_m_n, separated_nonempty_list },
        sequence::{ delimited, pair, separated_pair, terminated, tuple },
    };
    
    impl FromStr for Tag {
        type Err = self::nom::Err<(String, ErrorKind)>;
    
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            self::languagetag(s)
                .map(|r| r.1)
                .map_err(|err| err.map_input(|inp| inp.to_string()))
        }
    }    
    
    fn dash(input: &str) -> IResult<&str, char> { char('-')(input) }

    fn extension_form<'a, O, F>(prefix: F, min: usize) -> impl Fn(&'a str) -> IResult<&'a str, &'a str>
    where F: Fn(&'a str) -> IResult<&'a str, O> {
        recognize(separated_pair(prefix, dash, separated_nonempty_list(dash, alphanums(min, 8))))
    }

    fn subtag<'a, O, F>(parser: F) -> impl Fn(&'a str) -> IResult<&'a str, O>
    where F: Fn(&'a str) -> IResult<&'a str, O> {
        let eot = not(peek(verify(anychar, |c| c.is_ascii_alphanumeric())));
        delimited(dash, parser, eot)
    }

    fn from_str<'a, F>(parser: F) -> impl Fn(&'a str) -> IResult<&'a str, String>
    where F: Fn(&'a str) -> IResult<&'a str, &'a str> {
        map(parser, str::to_string)
    }

    fn alphanums<'a>(m: usize, n: usize) -> impl Fn(&'a str) -> IResult<&'a str, &'a str> { 
        take_while_m_n(m, n, |c: char| c.is_ascii_alphanumeric()) 
    }

    fn private(input: &str) -> IResult<&str, &str> { 
        extension_form(char('x'), 1)(input)
    }

    fn fixed_parse<'a, L, R, V>(name: &'static str, lang: L, region: R, variant: V) -> impl Fn(&'a str) -> IResult<&'a str, Tag>
    where
        L: Into<Option<&'static str>>, 
        R: Into<Option<&'static str>>,
        V: Into<Option<&'static str>> 
    {
        value(Tag {
                lang: lang.into().unwrap_or(name).into(),
                region: region.into().map(str::to_string),
                variant: variant.into().map(str::to_string).into_iter().collect(),
                ..Tag::default()
            },
            tag(name))
    }

    macro_rules! fixed_parse {
        ($f:literal) => (fixed_parse($f, None, None, None));
        ($f:literal, $l:literal) => (fixed_parse($f, $l, None, None));
        ($f:literal, $l:literal, $r:literal) => (fixed_parse($f, $l, $r, None));
        ($f:literal, $l:literal, $r:tt, $v:literal) => (fixed_parse($f, $l, $r, $v));
    }

    fn langtag(input: &str) -> IResult<&str, Tag> {
        let letters = |l| take_while_m_n(l, l, |c: char| c.is_ascii_alphabetic());
        let digits = |l| take_while_m_n(l, l, |c: char| c.is_ascii_digit());
        let ident = verify(alphanums(4,4), |s: &str| s.starts_with(|c: char| c.is_ascii_digit()));
        let singleton = verify(none_of("xX"), |c| c.is_ascii_alphanumeric());
        let extlang = many_m_n(1, 3, subtag(letters(3)));
        let language = recognize(pair(alphanums(2,3), opt(extlang)));
        let script = subtag(letters(4));
        let region = subtag(alt((letters(2), digits(3))));
        let variants = subtag(alt((ident, alphanums(5,8))));
        let extensions = subtag(extension_form(singleton, 2));
        let terminator = not(peek(verify(anychar, |c| *c == '-' || c.is_ascii_alphanumeric())));
        let (input, tags) = terminated(
            tuple((
                context("language code", from_str(language)), 
                context("script code", opt(from_str(script))), 
                context("region code", opt(from_str(region))),
                context("variant subtags", many0(from_str(variants))),
                context("extension subtags", many0(from_str(extensions))),
                context("private subtag", opt(from_str(subtag(private)))))),
            terminator)(input)?;
        Ok((input, Tag { 
            lang:      tags.0,
            script:    tags.1, 
            region:    tags.2, 
            variant:   tags.3, 
            extension: tags.4, 
            private:   tags.5 
        }))    
    }

    fn privateuse(input: &str) -> IResult<&str, Tag> {
        let (input, pu) = context("private use tag", private)(input)?;
        Ok((input, Tag::default().private(pu)))
    }

    fn grandfathered_regular(input: &str) -> IResult<&str, Tag> {
        context("regular grandfathered", 
            alt(( 
                fixed_parse!("cel-gaulish"),
                fixed_parse!("art-lojban","jbo"),
                fixed_parse!("zh-min-nan","nan"),
                fixed_parse!("zh-hakka","hak"),
                fixed_parse!("zh-guoyu","cmn"),
                fixed_parse!("zh-xiang","hsn"),
                fixed_parse!("zh-min"),
                fixed_parse!("no-bok","nb"),
                fixed_parse!("no-nyn","nn"),
            )))(input)
    }

    fn grandfathered_irregular(input: &str) -> IResult<&str, Tag> {
        context("irregular grandfathered", 
            alt(( 
                fixed_parse!("i-enochian"),
                fixed_parse!("en-GB-oed","en","GB","oxendict"),
                fixed_parse!("i-default"),
                fixed_parse!("i-klingon","tlh"),
                fixed_parse!("i-navajo","nv"),
                fixed_parse!("sgn-BE-FR","sfb"),
                fixed_parse!("sgn-BE-NL","vgt"),
                fixed_parse!("sgn-CH-DE","sgg"),
                fixed_parse!("i-mingo"),
                fixed_parse!("i-ami","ami"),
                fixed_parse!("i-bnn","bnn"),
                fixed_parse!("i-hak","hak"),
                fixed_parse!("i-lux","lb"),
                fixed_parse!("i-pwn","pwn"),
                fixed_parse!("i-tao","tao"),
                fixed_parse!("i-tay","tay"),
                fixed_parse!("i-tsu","tsu")
            )))(input)
    }

    pub fn languagetag(input: &str) -> IResult<&str, Tag> {
        alt((
            grandfathered_regular, 
            langtag, 
            privateuse, 
            grandfathered_irregular))(input)
    }
}

#[cfg(test)]
mod tests {
    use super::Tag;

    #[test]
    fn display() {
        let tag = Tag::lang("en-aaa-ccc")
            .script("Latn")
            .region("US")
            .add_variant("2abc")
            .add_variant("what2")
            .extensions(["a-bable", "q-babbel"])
            .private("x-priv1");
        assert_eq!(tag.to_string(), "en-aaa-ccc-Latn-US-2abc-what2-a-bable-q-babbel-x-priv1");
    }

    #[test]
    fn sorting() {
        let aa          = Tag::lang("aa");
        let aa_et       = Tag::lang("aa").region("ET");
        let aa_latn     = Tag::lang("aa").script("Latn");
        let aa_latn_et  = Tag::lang("aa").script("Latn").region("ET");
        let standard = [&aa, &aa_et, &aa_latn, &aa_latn_et];
        let mut test = [&aa_latn_et, &aa, &aa_et, &aa_latn];
        test.sort();

        assert_eq!(test, standard);
    }

    #[test]
    fn grandfathered() {
        let gf_cases = [
            ("art-lojban",      Ok(Tag::lang("jbo"))),
            ("cel-gaulish",     Ok(Tag::lang("cel-gaulish"))),
            ("en-GB-oed",       Ok(Tag::lang("en")
                                        .region("GB")
                                        .add_variant("oxendict"))),
            ("i-ami",           Ok(Tag::lang("ami"))),
            ("i-bnn",           Ok(Tag::lang("bnn"))),
            ("i-default",       Ok(Tag::lang("i-default"))),
            ("i-enochian",      Ok(Tag::lang("i-enochian"))),
            ("i-hak",           Ok(Tag::lang("hak"))),
            ("i-klingon",       Ok(Tag::lang("tlh"))),
            ("i-lux",           Ok(Tag::lang("lb"))),
            ("i-mingo",         Ok(Tag::lang("i-mingo"))),
            ("i-navajo",        Ok(Tag::lang("nv"))),
            ("i-pwn",           Ok(Tag::lang("pwn"))),
            ("i-tao",           Ok(Tag::lang("tao"))),
            ("i-tay",           Ok(Tag::lang("tay"))),
            ("i-tsu",           Ok(Tag::lang("tsu"))),
            ("no-bok",          Ok(Tag::lang("nb"))),
            ("no-nyn",          Ok(Tag::lang("nn"))),
            ("sgn-BE-FR",       Ok(Tag::lang("sfb"))),
            ("sgn-BE-NL",       Ok(Tag::lang("vgt"))),
            ("sgn-CH-DE",       Ok(Tag::lang("sgg"))),
            ("zh-guoyu",        Ok(Tag::lang("cmn"))),
            ("zh-hakka",        Ok(Tag::lang("hak"))),
            ("zh-min",          Ok(Tag::lang("zh-min"))),
            ("zh-min-nan",      Ok(Tag::lang("nan"))),
            ("zh-xiang",        Ok(Tag::lang("hsn"))),
        ];
        for (test, result) in &gf_cases {
            assert_eq!(test.parse::<Tag>(), *result);
        }
    }

    #[test]
    fn complex() {
        use nom::{ Err, error::ErrorKind };
        let gf_cases = [
            ("-",                   Err(Err::Error(("-".to_string(), ErrorKind::Tag)))),
            ("de",                  Ok(Tag::lang("de"))),
            ("en-x-priv2",          Ok(Tag::lang("en").private("x-priv2"))),
            ("en-us",               Ok(Tag::lang("en").region("us"))),
            ("en-Latn-US",          Ok(Tag::lang("en").script("Latn").region("US"))),
            ("ca-valencia",         Ok(Tag::lang("ca").add_variant("valencia"))),
            ("en-Latn-US-2abc-3cde-a2c3e-xwhat-x-priv2",    
                                    Ok(Tag::lang("en").script("Latn").region("US")
                                        .variants(["2abc","3cde","a2c3e","xwhat"])
                                        .private("x-priv2"))),
            ("en-aaa-ccc-Latn-US-2abc-what2-a-bable-q-babbel-x-priv1",
                                    Ok(Tag::lang("en-aaa-ccc").script("Latn").region("US")
                                        .variants(["2abc","what2"])
                                        .add_extension("a-bable") 
                                        .add_extension("q-babbel")
                                        .private("x-priv1"))),
            ("x-priv1-priv2-xpriv3",Ok(Tag::privateuse("x-priv1-priv2-xpriv3"))),
            ("en-gan-yue-Latn",     Ok(Tag::lang("en-gan-yue").script("Latn").clone())),
        ];
        for (test, result) in &gf_cases {
            assert_eq!(test.parse::<Tag>(), *result);
        }
    }
}
