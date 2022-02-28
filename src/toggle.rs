use std::{
    ops::Deref,
    str::FromStr,
};

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct Toggle(bool);

impl Toggle {
    pub const ON: Toggle = Toggle(true);
    pub const OFF: Toggle = Toggle(false);
}

impl Deref for Toggle {
    type Target = bool;
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl FromStr for Toggle {
    type Err = core::convert::Infallible;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            ""|"0"|"no"|"false"|"off" => Toggle::OFF, 
            _ => Toggle::ON
        })  
    }    
}    


#[cfg(test)]
mod tests {
    use super::Toggle;

    #[test]
    fn default_value() {
        let t: Toggle = Default::default();
        assert_eq!(t, Toggle::OFF);
        assert_eq!(*Toggle::OFF, false);
        assert_eq!(*Toggle::ON, true);
    }

    #[test]
    fn off_parses() {
        assert_eq!("".parse(), Ok(Toggle::OFF));
        assert_eq!("0".parse::<Toggle>(), Ok(Toggle::OFF));
        assert_eq!("no".parse::<Toggle>(), Ok(Toggle::OFF));
        assert_eq!("false".parse::<Toggle>(), Ok(Toggle::OFF));
        assert_eq!("off".parse::<Toggle>(), Ok(Toggle::OFF));        
    }

    #[test]
    fn on_parses() {
        assert_eq!("1".parse::<Toggle>(), Ok(Toggle::ON));
        assert_eq!("yes".parse::<Toggle>(), Ok(Toggle::ON));
        assert_eq!("true".parse::<Toggle>(), Ok(Toggle::ON));
        assert_eq!("on".parse::<Toggle>(), Ok(Toggle::ON));
        assert_eq!("maybe".parse::<Toggle>(), Ok(Toggle::ON));
        assert_eq!("😼".parse::<Toggle>(), Ok(Toggle::ON));
    }
}
