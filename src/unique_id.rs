use rand::prelude::*;
use serde_with::DeserializeFromStr;
use std::{num::NonZeroU32, ops::Deref, str::FromStr};

#[derive(Debug, Clone, Copy, DeserializeFromStr, Eq, PartialEq)]
pub struct UniqueID(u32);

impl UniqueID {
    fn new() -> Self {
        UniqueID(random())
    }
}

impl Deref for UniqueID {
    type Target = u32;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for UniqueID {
    type Err = <NonZeroU32 as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "unknown" => Ok(UniqueID::new()),
            _ => NonZeroU32::from_str(s).map(|n| UniqueID(n.get())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UniqueID;

    #[test]
    fn deref() {
        let t = UniqueID(012345678);
        assert_eq!(*t, 12345678u32);
    }

    #[test]
    fn parses() {
        use std::num::IntErrorKind;

        assert_eq!(
            "".parse::<UniqueID>().unwrap_err().kind(),
            &IntErrorKind::Empty
        );
        assert_eq!(
            "0".parse::<UniqueID>().unwrap_err().kind(),
            &IntErrorKind::Zero
        );
        assert_eq!(
            "none".parse::<UniqueID>().unwrap_err().kind(),
            &IntErrorKind::InvalidDigit
        );
        assert_eq!("012345678".parse::<UniqueID>().unwrap(), UniqueID(12345678));
        assert!("unknown".parse::<UniqueID>().is_ok());
    }
}
