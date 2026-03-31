//! Error types for the bse crate.
//!
//! This module defines the [`BseError`] enum for all error conditions
//! that can occur when working with basis sets.

use std::error::Error;
use std::fmt::Display;

/// Error type for basis set operations.
#[derive(Debug, Clone)]
pub enum BseError {
    /// Basis set or element data not found.
    DataNotFound(String),
    /// Invalid data format or content.
    DataError(String),
    /// Invalid key in lookup operation.
    KeyError(String),
    /// Invalid value provided by user.
    ValueError(String),
    /// Feature not yet implemented.
    NotImplementedError(String),
    /// I/O error reading files.
    IOError(String),
    /// JSON serialization/deserialization error.
    SerdeJsonError(String),
    /// Other errors.
    Miscellaneous(String),
    /// Builder field not initialized.
    UninitializedFieldError(derive_builder::UninitializedFieldError),
}

impl Display for BseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Error for BseError {}

impl From<std::io::Error> for BseError {
    fn from(err: std::io::Error) -> Self {
        BseError::IOError(err.to_string())
    }
}

impl From<serde_json::Error> for BseError {
    fn from(err: serde_json::Error) -> Self {
        BseError::SerdeJsonError(err.to_string())
    }
}

impl From<derive_builder::UninitializedFieldError> for BseError {
    fn from(err: derive_builder::UninitializedFieldError) -> Self {
        BseError::UninitializedFieldError(err)
    }
}

/// Create a trace string with file, line, and column information.
#[macro_export]
macro_rules! bse_trace {
    () => {
        concat!(file!(), ":", line!(), ":", column!(), ": ")
    };
}

/// Raise a [`BseError`] with trace information.
///
/// # Example
///
/// ```rust,compile_fail
/// use bse::prelude::*;
/// bse_raise!(ValueError, "Invalid element: {}", "Xx")?;
/// ```
#[macro_export]
macro_rules! bse_raise {
    ($errtype: ident, $($arg:tt)*) => {{
        use $crate::prelude::*;
        use std::fmt::Write;
        let mut s = String::new();
        write!(s, bse_trace!()).unwrap();
        write!(s, concat!("BseError::", stringify!($errtype), ": ")).unwrap();
        write!(s, $($arg)*).unwrap();
        Err(BseError::$errtype(s))
    }};
}
