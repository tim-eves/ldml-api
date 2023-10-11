use std::str::FromStr;

use super::Tag;

extern crate nom;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while_m_n},
    character::complete::{anychar, char, none_of},
    combinator::{not, opt, peek, recognize, value, verify},
    error::{context, ContextError, ParseError},
    multi::{many0, many_m_n, separated_list1},
    sequence::{delimited, pair, separated_pair, terminated, tuple},
    IResult,
};

pub use nom::{
    error::{Error, ErrorKind},
    Finish,
};

fn dash<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, char, E> {
    char('-')(input)
}

fn extension_form<'a, O, E, F>(
    prefix: F,
    min: usize,
) -> impl FnMut(&'a str) -> IResult<&'a str, &'a str, E>
where
    E: ParseError<&'a str>,
    F: FnMut(&'a str) -> IResult<&'a str, O, E>,
{
    recognize(separated_pair(
        prefix,
        dash,
        separated_list1(dash, alphanums(min, 8)),
    ))
}

fn subtag<'a, O, E, F>(parser: F) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
    E: ParseError<&'a str>,
    F: FnMut(&'a str) -> IResult<&'a str, O, E>,
{
    let eot = not(peek(verify(anychar, |c| c.is_ascii_alphanumeric())));
    delimited(dash, parser, eot)
}

fn alphanums<'a, E: ParseError<&'a str>>(
    m: usize,
    n: usize,
) -> impl Fn(&'a str) -> IResult<&'a str, &'a str, E> {
    take_while_m_n(m, n, |c: char| c.is_ascii_alphanumeric())
}

fn private<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, &'a str, E> {
    extension_form(char('x'), 1)(input)
}

fn fixed_parse<'a, E: ParseError<&'a str>>(
    name: &'static str,
    lang: impl Into<Option<&'static str>>,
    region: impl Into<Option<&'static str>>,
    variant: impl Into<Option<&'static str>>,
) -> impl FnMut(&'a str) -> IResult<&'a str, Tag, E> {
    value(
        Tag::from_parts(
            lang.into().unwrap_or(name),
            None,
            region.into(),
            variant.into(),
            None,
            None,
        ),
        tag(name),
    )
}

macro_rules! fixed_parse {
    ($f:literal) => {
        fixed_parse($f, None, None, None)
    };
    ($f:literal, $l:literal) => {
        fixed_parse($f, $l, None, None)
    };
    ($f:literal, $l:literal, $r:literal) => {
        fixed_parse($f, $l, $r, None)
    };
    ($f:literal, $l:literal, $r:tt, $v:literal) => {
        fixed_parse($f, $l, $r, $v)
    };
}

fn langtag<'a, E>(input: &'a str) -> IResult<&'a str, Tag, E>
where
    E: ParseError<&'a str> + ContextError<&'a str>,
{
    let letters = |l| take_while_m_n(l, l, |c: char| c.is_ascii_alphabetic());
    let digits = |l| take_while_m_n(l, l, |c: char| c.is_ascii_digit());
    let ident = verify(alphanums(4, 4), |s: &str| {
        s.starts_with(|c: char| c.is_ascii_digit())
    });
    let singleton = verify(none_of("xX"), |c| c.is_ascii_alphanumeric());
    let extlang = many_m_n(1, 3, subtag(letters(3)));
    let language = recognize(pair(alphanums(2, 3), opt(extlang)));
    let script = subtag(letters(4));
    let region = subtag(alt((letters(2), digits(3))));
    let variant = subtag(alt((ident, alphanums(5, 8))));
    let extension = subtag(extension_form(singleton, 2));
    let terminator = not(peek(verify(anychar, |c| {
        *c == '-' || c.is_ascii_alphanumeric()
    })));
    let (rest, mut tags) = terminated(
        tuple((
            context("language code", language),
            context("script code", opt(script)),
            context("region code", opt(region)),
            context("variant subtags", many0(variant)),
            context("extension subtags", many0(extension)),
            context("private subtag", opt(subtag(private))),
        )),
        terminator,
    )(input)?;
    tags.3.sort_unstable();
    tags.4.sort_unstable();
    Ok((
        rest,
        Tag::new(
            &input[..input.len() - rest.len()],
            tags.0.len(),
            tags.1.and_then(|r| r.len().try_into().ok()),
            tags.2.and_then(|r| r.len().try_into().ok()),
            tags.3.into_iter().map(|v| v.len().try_into().unwrap()),
            tags.4.into_iter().map(|e| e.len().try_into().unwrap()),
            tags.5.and_then(|r| r.len().try_into().ok()),
        ),
    ))
}

fn privateuse<'a, E>(input: &'a str) -> IResult<&'a str, Tag, E>
where
    E: ParseError<&'a str> + ContextError<&'a str>,
{
    let (input, pu) = context("private use tag", private)(input)?;
    Ok((input, Tag::privateuse(pu)))
}

fn grandfathered_regular<'a, E>(input: &'a str) -> IResult<&'a str, Tag, E>
where
    E: ParseError<&'a str> + ContextError<&'a str>,
{
    context(
        "regular grandfathered",
        alt((
            fixed_parse!("cel-gaulish"),
            fixed_parse!("art-lojban", "jbo"),
            fixed_parse!("zh-min-nan", "nan"),
            fixed_parse!("zh-hakka", "hak"),
            fixed_parse!("zh-guoyu", "cmn"),
            fixed_parse!("zh-xiang", "hsn"),
            fixed_parse!("zh-min"),
            fixed_parse!("no-bok", "nb"),
            fixed_parse!("no-nyn", "nn"),
        )),
    )(input)
}

fn grandfathered_irregular<'a, E>(input: &'a str) -> IResult<&'a str, Tag, E>
where
    E: ParseError<&'a str> + ContextError<&'a str>,
{
    context(
        "irregular grandfathered",
        alt((
            fixed_parse!("i-enochian"),
            fixed_parse!("en-GB-oed", "en", "GB", "oxendict"),
            fixed_parse!("i-default"),
            fixed_parse!("i-klingon", "tlh"),
            fixed_parse!("i-navajo", "nv"),
            fixed_parse!("sgn-BE-FR", "sfb"),
            fixed_parse!("sgn-BE-NL", "vgt"),
            fixed_parse!("sgn-CH-DE", "sgg"),
            fixed_parse!("i-mingo"),
            fixed_parse!("i-ami", "ami"),
            fixed_parse!("i-bnn", "bnn"),
            fixed_parse!("i-hak", "hak"),
            fixed_parse!("i-lux", "lb"),
            fixed_parse!("i-pwn", "pwn"),
            fixed_parse!("i-tao", "tao"),
            fixed_parse!("i-tay", "tay"),
            fixed_parse!("i-tsu", "tsu"),
        )),
    )(input)
}

pub fn languagetag<'a, E>(input: &'a str) -> IResult<&'a str, Tag, E>
where
    E: ParseError<&'a str> + ContextError<&'a str>,
{
    alt((
        grandfathered_regular,
        langtag,
        privateuse,
        grandfathered_irregular,
    ))(input)
}

impl FromStr for Tag {
    type Err = Error<String>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match languagetag(s).finish() {
            Ok((_, tag)) => Ok(tag),
            Err(Error { input, code }) => Err(Self::Err {
                input: input.to_owned(),
                code,
            }),
        }
    }
}

mod test {
    #[test]
    fn grandfathered() {
        use crate::Tag;

        let gf_cases = [
            ("art-lojban", Ok(Tag::with_lang("jbo"))),
            ("cel-gaulish", Ok(Tag::with_lang("cel-gaulish"))),
            (
                "en-GB-oed",
                Ok(Tag::builder()
                    .lang("en")
                    .region("GB")
                    .variant("oxendict")
                    .build()),
            ),
            ("i-ami", Ok(Tag::with_lang("ami"))),
            ("i-bnn", Ok(Tag::with_lang("bnn"))),
            ("i-default", Ok(Tag::with_lang("i-default"))),
            ("i-enochian", Ok(Tag::with_lang("i-enochian"))),
            ("i-hak", Ok(Tag::with_lang("hak"))),
            ("i-klingon", Ok(Tag::with_lang("tlh"))),
            ("i-lux", Ok(Tag::with_lang("lb"))),
            ("i-mingo", Ok(Tag::with_lang("i-mingo"))),
            ("i-navajo", Ok(Tag::with_lang("nv"))),
            ("i-pwn", Ok(Tag::with_lang("pwn"))),
            ("i-tao", Ok(Tag::with_lang("tao"))),
            ("i-tay", Ok(Tag::with_lang("tay"))),
            ("i-tsu", Ok(Tag::with_lang("tsu"))),
            ("no-bok", Ok(Tag::with_lang("nb"))),
            ("no-nyn", Ok(Tag::with_lang("nn"))),
            ("sgn-BE-FR", Ok(Tag::with_lang("sfb"))),
            ("sgn-BE-NL", Ok(Tag::with_lang("vgt"))),
            ("sgn-CH-DE", Ok(Tag::with_lang("sgg"))),
            ("zh-guoyu", Ok(Tag::with_lang("cmn"))),
            ("zh-hakka", Ok(Tag::with_lang("hak"))),
            ("zh-min", Ok(Tag::with_lang("zh-min"))),
            ("zh-min-nan", Ok(Tag::with_lang("nan"))),
            ("zh-xiang", Ok(Tag::with_lang("hsn"))),
        ];
        for (test, result) in &gf_cases {
            let test = test.parse::<Tag>();
            assert_eq!(test, *result);
        }
    }

    #[test]
    fn complex() {
        use crate::Tag;

        use nom::error::{Error, ErrorKind};
        let gf_cases = [
            (
                "-",
                Err(Error {
                    input: "-".to_string(),
                    code: ErrorKind::Tag,
                }),
            ),
            ("de", Ok(Tag::with_lang("de"))),
            (
                "en-x-priv2",
                Ok(Tag::builder().lang("en").private("x-priv2").build()),
            ),
            ("en-us", Ok(Tag::builder().lang("en").region("us").build())),
            (
                "en-Latn-US",
                Ok(Tag::builder()
                    .lang("en")
                    .script("Latn")
                    .region("US")
                    .build()),
            ),
            (
                "ca-valencia",
                Ok(Tag::builder().lang("ca").variant("valencia").build()),
            ),
            (
                "en-Latn-US-2abc-3cde-a2c3e-xwhat-x-priv2",
                Ok(Tag::builder()
                    .lang("en")
                    .script("Latn")
                    .region("US")
                    .variants(["2abc", "3cde", "a2c3e", "xwhat"])
                    .private("x-priv2")
                    .build()),
            ),
            (
                "en-aaa-ccc-Latn-US-2abc-what2-a-bable-test-q-babbel-x-priv1",
                Ok(Tag::builder()
                    .lang("en-aaa-ccc")
                    .script("Latn")
                    .region("US")
                    .variants(["2abc", "what2"])
                    .extension("a-bable")
                    .extension("a-test")
                    .extension("q-babbel")
                    .private("x-priv1")
                    .build()),
            ),
            (
                "x-priv1-priv2-xpriv3",
                Ok(Tag::privateuse("x-priv1-priv2-xpriv3")),
            ),
            (
                "en-gan-yue-Latn",
                Ok(Tag::builder().lang("en-gan-yue").script("Latn").build()),
            ),
        ];
        for (test, result) in &gf_cases {
            assert_eq!(test.parse::<Tag>(), *result);
        }
    }
}
