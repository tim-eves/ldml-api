use nom::{
    branch::alt,
    bytes::{tag, take_while_m_n},
    character::complete::{char, none_of, satisfy},
    combinator::{not, opt, peek, recognize, value, verify},
    error::{context, ContextError, ParseError},
    multi::{many1_count, many_m_n, separated_list0},
    sequence::{delimited, pair, preceded, terminated},
    AsChar, IResult, Parser,
};

use crate::Tag;

fn dash<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, char, E> {
    char('-').parse_complete(input)
}

fn extension_form<'a, E: ParseError<&'a str>>(
    prefix: impl Parser<&'a str, Output = char, Error = E>,
    min: usize,
) -> impl Parser<&'a str, Error = E> {
    preceded(prefix, many1_count((dash, alphanums(min, 8))))
}

fn subtag<'a, O, E: ParseError<&'a str>>(
    parser: impl Parser<&'a str, Output = O, Error = E>,
) -> impl Parser<&'a str, Output = O, Error = E> {
    let eot = not(peek(satisfy(AsChar::is_alphanum)));
    delimited(dash, parser, eot)
}

fn alphanums<'a, E: ParseError<&'a str>>(
    m: usize,
    n: usize,
) -> impl Parser<&'a str, Output = &'a str, Error = E> {
    take_while_m_n(m, n, |c: char| c.is_ascii_alphanumeric())
}

fn private<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, &'a str, E> {
    recognize(extension_form(char('x'), 1)).parse_complete(input)
}

macro_rules! fixed_parse {
    ($f:literal) => {
        fixed_parse!($f, $f)
    };
    ($f:literal, $l:literal) => {
        value(Tag::new($l, $l.len(), None, None, None, None), tag($f))
    };
    ($f:literal, $l:literal, $r:literal) => {
        value(
            Tag::new(
                concat($l, '-', $r),
                $l.len().try_into().ok(),
                None,
                $r.len().try_into().ok(),
                None,
                None,
            ),
            tag($f),
        )
    };
    ($f:literal, $l:literal, $r:literal, $v:literal) => {
        value(
            Tag::new(
                concat!($l, '-', $r, '-', $v),
                $l.len(),
                None,
                $r.len().try_into().ok(),
                $v.len().try_into().ok(),
                None,
            ),
            tag($f),
        )
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
    let variant = alt((ident, alphanums(5, 8)));
    let variants = subtag(recognize(separated_list0(dash, variant)));
    let extension = extension_form(singleton, 2);
    let extensions = subtag(recognize(separated_list0(dash, extension)));
    let private = subtag(private);
    let terminator = not(peek(satisfy(|c| c == '-' || c.is_ascii_alphanumeric())));
    let (rest, tags) = terminated(
        (
            context("language code", language),
            context("script code", opt(script)),
            context("region code", opt(region)),
            context("variant subtags", opt(variants)),
            context("extension subtags", opt(extensions)),
            context("private subtag", opt(private)),
        ),
        terminator,
    )
    .parse_complete(input)?;

    Ok((
        rest,
        Tag::new(
            &input[..input.len() - rest.len()],
            tags.0.len(),
            tags.1.and_then(|r| r.len().try_into().ok()),
            tags.2.and_then(|r| r.len().try_into().ok()),
            tags.3.and_then(|r| r.len().try_into().ok()),
            tags.4.and_then(|r| r.len().try_into().ok()),
        ),
    ))
}

fn privateuse<'a, E>(input: &'a str) -> IResult<&'a str, Tag, E>
where
    E: ParseError<&'a str> + ContextError<&'a str>,
{
    let (input, pu) = context("private use tag", private).parse_complete(input)?;
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
    )
    .parse_complete(input)
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
    )
    .parse_complete(input)
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
    ))
    .parse_complete(input)
}

#[cfg(test)]
mod tests {
    #[test]
    fn grandfathered() {
        use crate::Tag;

        let gf_cases = [
            ("art-lojban", Tag::with_lang("jbo")),
            ("cel-gaulish", Tag::with_lang("cel-gaulish")),
            (
                "en-GB-oed",
                Tag::builder()
                    .lang("en")
                    .region("GB")
                    .variant("oxendict")
                    .into(),
            ),
            ("i-ami", Tag::with_lang("ami")),
            ("i-bnn", Tag::with_lang("bnn")),
            ("i-default", Tag::with_lang("i-default")),
            ("i-enochian", Tag::with_lang("i-enochian")),
            ("i-hak", Tag::with_lang("hak")),
            ("i-klingon", Tag::with_lang("tlh")),
            ("i-lux", Tag::with_lang("lb")),
            ("i-mingo", Tag::with_lang("i-mingo")),
            ("i-navajo", Tag::with_lang("nv")),
            ("i-pwn", Tag::with_lang("pwn")),
            ("i-tao", Tag::with_lang("tao")),
            ("i-tay", Tag::with_lang("tay")),
            ("i-tsu", Tag::with_lang("tsu")),
            ("no-bok", Tag::with_lang("nb")),
            ("no-nyn", Tag::with_lang("nn")),
            ("sgn-BE-FR", Tag::with_lang("sfb")),
            ("sgn-BE-NL", Tag::with_lang("vgt")),
            ("sgn-CH-DE", Tag::with_lang("sgg")),
            ("zh-guoyu", Tag::with_lang("cmn")),
            ("zh-hakka", Tag::with_lang("hak")),
            ("zh-min", Tag::with_lang("zh-min")),
            ("zh-min-nan", Tag::with_lang("nan")),
            ("zh-xiang", Tag::with_lang("hsn")),
        ];
        for (test, result) in &gf_cases {
            let test: Tag = test.parse().expect("should parse grandfathered test case");
            assert_eq!(&test, result);
        }
    }
}
