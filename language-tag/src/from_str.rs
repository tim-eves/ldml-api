use std::{error::Error, fmt::Display, str::FromStr};

use nom::Finish;

use crate::{parser::languagetag, Tag};

#[derive(Debug)]
pub struct ParseTagError(nom::error::Error<String>);

impl Display for ParseTagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "failed to parse tag: {input}",
            input = self.0.input.trim_end()
        )
    }
}

impl Error for ParseTagError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl FromStr for Tag {
    type Err = ParseTagError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use nom::error::Error;
        match languagetag(s).finish() {
            Ok((_, tag)) => Ok(tag),
            Err(Error { input, code }) => Err(ParseTagError(Error {
                input: input.to_owned(),
                code,
            })),
        }
    }
}
