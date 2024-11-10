use globset::{Glob, GlobBuilder};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PortableGlobError {
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

pub fn parse_portable_glob(glob: &str) -> Result<Glob, PortableGlobError> {
    check_portable_glob(glob)?;
    Ok(GlobBuilder::new(glob).literal_separator(true).build()?)
}

/// See [`parse_portable_glob`].
pub fn check_portable_glob(glob: &str) -> Result<(), PortableGlobError> {
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
                return Err(PortableGlobError::TooManyStars {
                    glob: glob.to_string(),
                    // Three stars in the beginning makes pos 2 and star_run 3.
                    pos: pos + 1 - star_run,
                });
            } else if star_run == 2 {
                if chars.peek().is_none_or(|(_, c)| *c != '/') {
                    return Err(PortableGlobError::TooManyStars {
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
                return Err(PortableGlobError::ParentDirectory {
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
                    return Err(PortableGlobError::InvalidCharacterRange {
                        glob: glob.to_string(),
                        pos,
                        invalid: c,
                    });
                }
            }
            start_or_slash = false;
        } else {
            return Err(PortableGlobError::InvalidCharacter {
                glob: glob.to_string(),
                pos,
                invalid: c,
            });
        }
    }
    Ok(())
}
