#![doc = include_str!("../README.md")]
#![doc = "\n\n---\n\n"]
#![doc = include_str!("../syntax.md")]

mod error;
mod filter;
mod parser;

pub use error::Error;
pub use filter::PathFilter;
