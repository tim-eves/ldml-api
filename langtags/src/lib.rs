pub mod json;
mod langtags;
pub mod tagset;
pub mod text;

#[cfg(feature = "compact")]
use compact_str::CompactString as StringRepr;
#[cfg(not(feature = "compact"))]
use std::string::String as StringRepr;
