use std::io;

use crate::parser;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Parsing(#[from] pest::error::Error<parser::Rule>),

    #[error("invalid pattern '{pattern}': {source}")]
    Pattern {
        pattern: String,
        source: glob::PatternError,
    },

    #[error("negated patterns must be absolute (start with '/')")]
    PatternNegatedMustBeAbsolute,

    #[error("negated patterns can not contain arbitrary subdirectories (must not contain '**')")]
    PatternNegatedNoArbitrarySubdirectories,

    #[error("unknown directive '{key}'")]
    DirectiveUnknown { key: String },

    #[error("invalid directive value for '{key}': {cause}")]
    DirectiveValueInvalid {
        key: String,
        cause: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("global directives are currently not supported")]
    DirectiveGlobalUnsupported,
}
