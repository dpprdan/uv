//! Implementation of PEP 639 cross-language restricted globs.
//!
//! The goal is globs that are portable between languages and operating systems.

mod glob_walker;
mod portable_glob;

pub use glob_walker::{GlobDirMatcher, GlobWalkDir, GlobWalkerIntoIterator};
pub use portable_glob::{check_portable_glob, parse_portable_glob, PortableGlobError};
