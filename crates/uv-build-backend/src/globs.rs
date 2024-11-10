//! Implementation of PEP 639 cross-language restricted globs.
//!
//! The goal is globs that are portable between languages and operating systems.

use globset::{Glob, GlobSet, GlobSetBuilder};
use itertools::Itertools;
use regex::bytes::Regex as BytesRegex;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Pep639GlobError {
    /// Shows the failing glob in the error message.
    #[error(transparent)]
    GlobError(#[from] globset::Error),
    #[error(
        "The parent directory operator (`..`) at position {pos} is not allowed in glob: `{glob}`"
    )]
    ParentDirectory { glob: String, pos: usize },
    #[error("Invalid character `{invalid}` at position {pos} in glob: `{glob}`")]
    InvalidCharacter {
        glob: String,
        pos: usize,
        invalid: char,
    },
    #[error("Invalid character `{invalid}` at position {pos} in glob: `{glob}`")]
    InvalidCharacterRange {
        glob: String,
        pos: usize,
        invalid: char,
    },
    #[error("Too many at stars at position {pos} in glob: `{glob}`")]
    TooManyStars { glob: String, pos: usize },
}

/// Parse a PEP 639 `license-files` glob.
///
/// The syntax is more restricted than regular globbing in Python or Rust for platform independent
/// results. Since [`globset::Glob`] is a superset over this format, we can use it after validating
/// that no unsupported features are in the string.
///
/// From [PEP 639](https://peps.python.org/pep-0639/#add-license-files-key):
///
/// > Its value is an array of strings which MUST contain valid glob patterns,
/// > as specified below:
/// >
/// > - Alphanumeric characters, underscores (`_`), hyphens (`-`) and dots (`.`)
/// >   MUST be matched verbatim.
/// >
/// > - Special glob characters: `*`, `?`, `**` and character ranges: `[]`
/// >   containing only the verbatim matched characters MUST be supported.
/// >   Within `[...]`, the hyphen indicates a range (e.g. `a-z`).
/// >   Hyphens at the start or end are matched literally.
/// >
/// > - Path delimiters MUST be the forward slash character (`/`).
/// >   Patterns are relative to the directory containing `pyproject.toml`,
/// >   therefore the leading slash character MUST NOT be used.
/// >
/// > - Parent directory indicators (`..`) MUST NOT be used.
/// >
/// > Any characters or character sequences not covered by this specification are
/// > invalid. Projects MUST NOT use such values.
/// > Tools consuming this field MAY reject invalid values with an error.
pub(crate) fn parse_pep639_glob(glob: &str) -> Result<Glob, Pep639GlobError> {
    check_pep639_globs(glob)?;
    Ok(Glob::new(glob)?)
}

/// See [`parse_pep639_glob`].
fn check_pep639_globs(glob: &str) -> Result<(), Pep639GlobError> {
    let mut chars = glob.chars().enumerate().peekable();
    // A `..` is on a parent directory indicator at the start of the string or after a directory
    // separator.
    let mut start_or_slash = true;
    // The number of consecutive stars before the current character.
    while let Some((pos, c)) = chars.next() {
        // `***` or `**literals` can be correctly represented with less stars. They are banned by
        // `glob`, they are allowed by `globset` and PEP 639 is ambiguous, so we're filtering them
        // out.
        if c == '*' {
            let mut star_run = 1;
            while let Some((_, c)) = chars.peek() {
                if *c == '*' {
                    star_run += 1;
                    chars.next();
                } else {
                    break;
                }
            }
            if star_run == 3 {
                return Err(Pep639GlobError::TooManyStars {
                    glob: glob.to_string(),
                    // Three stars in the beginning makes pos 2 and star_run 3.
                    pos: pos + 1 - star_run,
                });
            } else if star_run == 2 {
                if c != '/' {
                    return Err(Pep639GlobError::TooManyStars {
                        glob: glob.to_string(),
                        pos: pos - star_run,
                    });
                }
            }
            start_or_slash = false;
        } else if c.is_alphanumeric() || matches!(c, '_' | '-' | '?') {
            start_or_slash = false;
        } else if c == '.' {
            if start_or_slash && matches!(chars.peek(), Some((_, '.'))) {
                return Err(Pep639GlobError::ParentDirectory {
                    pos,
                    glob: glob.to_string(),
                });
            }
            start_or_slash = false;
        } else if c == '/' {
            start_or_slash = true;
        } else if c == '[' {
            for (pos, c) in chars.by_ref() {
                if c.is_alphanumeric() || matches!(c, '_' | '-' | '.') {
                    // Allowed.
                } else if c == ']' {
                    break;
                } else {
                    return Err(Pep639GlobError::InvalidCharacterRange {
                        glob: glob.to_string(),
                        pos,
                        invalid: c,
                    });
                }
            }
            start_or_slash = false;
        } else {
            return Err(Pep639GlobError::InvalidCharacter {
                glob: glob.to_string(),
                pos,
                invalid: c,
            });
        }
    }
    Ok(())
}

pub(crate) struct Pep639GlobSet {
    globset: GlobSet,
    filter: Option<BytesRegex>,
}

impl Pep639GlobSet {
    pub(crate) fn new(globs: &[String]) -> Result<Self, Pep639GlobError> {
        let mut include_builder = GlobSetBuilder::new();
        let mut regexes = Vec::new();
        for glob in globs {
            let glob = parse_pep639_glob(glob)?;
            regexes.push(glob.regex());
            include_builder.add(glob);
        }
        let globset_regex = "^".to_string() + &regexes.iter().map(|re| format!("({re})")).join("|");
        let filter = BytesRegex::new(&globset_regex).expect("regex generated by globset is valid");

        let globset = include_builder.build()?;
        globset.Ok(Self { globset })
    }
}

#[cfg(test)]
mod tests;
