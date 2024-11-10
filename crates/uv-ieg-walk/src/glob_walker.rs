use bstr::{ByteSlice, ByteVec};
use globset::{Glob, GlobSet, GlobSetBuilder};
use regex_automata::dfa;
use regex_automata::dfa::Automaton;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use walkdir::{DirEntry, WalkDir};

pub struct GlobWalkDir {
    root: PathBuf,
    matcher: GlobDirMatcher,
    state: WalkDir,
}

impl GlobWalkDir {
    pub fn new(root: impl Into<PathBuf>, globs: &[Glob]) -> Self {
        let root = root.into();
        let walk_dir = WalkDir::new(&root);
        Self {
            root,
            matcher: GlobDirMatcher::new(globs),
            state: walk_dir,
        }
    }
}

impl IntoIterator for GlobWalkDir {
    type Item = walkdir::Result<(DirEntry, PathBuf)>;
    type IntoIter = GlobWalkerIntoIterator;

    fn into_iter(self) -> Self::IntoIter {
        GlobWalkerIntoIterator {
            root: self.root,
            matcher: self.matcher,
            state: self.state.into_iter(),
        }
    }
}

pub struct GlobWalkerIntoIterator {
    root: PathBuf,
    matcher: GlobDirMatcher,
    state: walkdir::IntoIter,
}

impl Iterator for GlobWalkerIntoIterator {
    /// Return the dir entry from walkdir and the relative path.
    type Item = walkdir::Result<(DirEntry, PathBuf)>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let entry = match self.state.next() {
                None => return None,
                Some(Ok(entry)) => entry,
                Some(Err(err)) => return Some(Err(err)),
            };
            // TODO(konsti): This is should be prettier.
            let relative = entry
                .path()
                .strip_prefix(&self.root)
                .expect("walkdir starts with root")
                .to_path_buf();

            if entry.path().is_dir() && !self.matcher.match_directory(&relative) {
                self.state.skip_current_dir();
                continue;
            }
            if !self.matcher.match_path(&relative) {
                continue;
            }
            return Some(Ok((entry, relative)));
        }
    }
}

pub struct GlobDirMatcher {
    glob_set: GlobSet,
    dfa: Option<dfa::dense::DFA<Vec<u32>>>,
}

impl GlobDirMatcher {
    pub fn new(globs: &[Glob]) -> Self {
        let mut glob_set_builder = GlobSetBuilder::new();
        for glob in globs {
            glob_set_builder.add(glob.clone());
        }
        let glob_set = glob_set_builder
            .build()
            // https://github.com/BurntSushi/ripgrep/discussions/2927
            .expect("globs can be combined to globset");

        let regexes: Vec<_> = globs
            .iter()
            .map(|glob| {
                let regex = glob
                    .regex()
                    .strip_prefix("(?-u)")
                    .expect("globs are non-unicode byte regex");
                regex
            })
            .collect();

        // Chosen at a whim -Konsti
        const SIZE_LIMIT: usize = 1_000_000;
        let dfa_builder = dfa::dense::Builder::new()
            .syntax(
                // The glob regex is a byte matcher
                regex_automata::util::syntax::Config::new()
                    .unicode(false)
                    .utf8(false),
            )
            .configure(
                dfa::dense::Config::new()
                    .start_kind(dfa::StartKind::Anchored)
                    // DFA can grow exponentially, in which case we bail out
                    .dfa_size_limit(Some(SIZE_LIMIT))
                    .determinize_size_limit(Some(SIZE_LIMIT)),
            )
            .build_many(&regexes);
        let dfa = match dfa_builder {
            Ok(dfa) => Some(dfa),
            Err(_) => {
                // TODO(konsti): `regex_automata::dfa::dense::BuildError` should allow asking whether
                // is a size error
                None
            }
        };

        Self { glob_set, dfa }
    }

    /// Whether the path matches any of the globs.
    pub fn match_path(&self, path: &Path) -> bool {
        self.glob_set.is_match(path)
    }

    /// Check whether a directory or any of its children has the option to be matched.
    pub fn match_directory(&self, path: &Path) -> bool {
        let Some(dfa) = &self.dfa else {
            return false;
        };

        // Allow the root path
        if path == Path::new("") {
            return true;
        }

        let config_anchored =
            regex_automata::util::start::Config::new().anchored(regex_automata::Anchored::Yes);
        let mut state = dfa.start_state(&config_anchored).unwrap();

        // Paths aren't necessarily UTF-8, which we can gloss over since the globs match bytes only
        // anyway.
        let byte_path = Vec::from_path_lossy(&path);
        for b in byte_path.as_bytes() {
            state = dfa.next_state(state, *b);
        }
        // Say we're looking at a directory `foo/bar`. We want to continue if either `foo/bar` is
        // a match, e.g., from `foo/*`, or a path below it can match, e.g., from `foo/bar/*`.
        let eoi_state = dfa.next_eoi_state(state);
        // We must not call `next_eoi_state` on the slash state, we want to only check if more
        // characters (path components) are allowed, not if we're matching the `$` anchor at the
        // end.
        let slash_state = dfa.next_state(state, u8::try_from(MAIN_SEPARATOR).unwrap());

        debug_assert!(
            !dfa.is_quit_state(eoi_state) && !dfa.is_quit_state(slash_state),
            "matcher is in quit state"
        );

        dfa.is_match_state(eoi_state) || !dfa.is_dead_state(slash_state)
    }
}

#[cfg(test)]
mod tests {
    use crate::glob_walker::{GlobDirMatcher, GlobWalkDir};
    use crate::portable_glob::parse_portable_glob;
    use std::path::Path;
    use tempfile::tempdir;
    use walkdir::WalkDir;

    const FILES: [&str; 5] = [
        "path1/dir1/subdir/a.txt",
        "path2/dir2/subdir/a.txt",
        "path3/dir3/subdir/a.txt",
        "path4/dir4/subdir/a.txt",
        "path5/dir5/subdir/a.txt",
    ];

    const PATTERNS: [&str; 5] = [
        // Only sufficient for descending one level
        "path1/*",
        // Only sufficient for descending one level
        "path2/dir2",
        // Sufficient for descending
        "path3/dir3/subdir/a.txt",
        // Sufficient for descending
        "path4/**/*",
        // Not sufficient for descending
        "path5",
    ];

    #[test]
    fn match_directory() {
        let patterns = PATTERNS.map(|pattern| parse_portable_glob(pattern).unwrap());
        let matcher = GlobDirMatcher::new(&patterns);
        assert!(matcher.match_directory(Path::new("path1/dir1")));
        assert!(matcher.match_directory(Path::new("path2/dir2")));
        assert!(matcher.match_directory(Path::new("path3/dir3")));
        assert!(matcher.match_directory(Path::new("path4/dir4")));
        assert!(!matcher.match_directory(Path::new("path5/dir5")));
    }

    /// Check that we skip directories that can never match.
    #[test]
    fn prefilter() {
        let dir = tempdir().unwrap();
        for file in FILES {
            let file = dir.path().join(file);
            fs_err::create_dir_all(file.parent().unwrap()).unwrap();
            fs_err::File::create(file).unwrap();
        }
        let patterns = PATTERNS.map(|pattern| parse_portable_glob(pattern).unwrap());
        let matcher = GlobDirMatcher::new(&patterns);

        // Test the prefix filtering
        let mut visited: Vec<_> = WalkDir::new(dir.path())
            .into_iter()
            .filter_entry(|entry| {
                let relative = entry
                    .path()
                    .strip_prefix(dir.path())
                    .expect("walkdir starts with root");
                matcher.match_directory(&relative)
            })
            .map(|entry| {
                let entry = entry.unwrap();
                let relative = entry
                    .path()
                    .strip_prefix(dir.path())
                    .expect("walkdir starts with root")
                    .to_str()
                    .unwrap()
                    .to_string();
                relative
            })
            .collect();
        visited.sort();
        assert_eq!(
            visited,
            [
                "",
                "path1",
                "path1/dir1",
                "path2",
                "path2/dir2",
                "path3",
                "path3/dir3",
                "path3/dir3/subdir",
                "path3/dir3/subdir/a.txt",
                "path4",
                "path4/dir4",
                "path4/dir4/subdir",
                "path4/dir4/subdir/a.txt",
                "path5"
            ]
        );
    }

    /// Check that the walkdir yield the correct set of files.
    #[test]
    fn walk_dir() {
        let dir = tempdir().unwrap();

        for file in FILES {
            let file = dir.path().join(file);
            fs_err::create_dir_all(file.parent().unwrap()).unwrap();
            fs_err::File::create(file).unwrap();
        }
        let patterns = PATTERNS.map(|pattern| parse_portable_glob(pattern).unwrap());

        let mut matches: Vec<_> = GlobWalkDir::new(dir.path(), &patterns)
            .into_iter()
            .map(|entry| entry.unwrap().1.to_str().unwrap().to_string())
            .collect();
        matches.sort();
        assert_eq!(
            matches,
            [
                "path1/dir1",
                "path2/dir2",
                "path3/dir3/subdir/a.txt",
                "path4/dir4",
                "path4/dir4/subdir",
                "path4/dir4/subdir/a.txt",
                "path5"
            ]
        );
    }
}
