use crate::glob_walker::{GlobDirMatcher, GlobWalkDir};
use globset::GlobSetBuilder;

mod glob_walker;
mod portable_glob;

fn main() {
    let includes = ["src/**/*", "third/*.py", "pyproject.toml", "foo/bar/*"];
    let mut include_builder = GlobSetBuilder::new();
    let mut include_globs = Vec::new();
    for include in includes {
        let glob = portable_glob::parse_portable_glob(include).expect("TODO");
        include_globs.push(glob.clone());
        include_builder.add(glob);
    }
    // https://github.com/BurntSushi/ripgrep/discussions/2927
    let include_matcher = include_builder.build().expect("TODO");

    let excludes = ["__pycache__", "*.pyc", "*.pyo"];
    let mut exclude_builder = GlobSetBuilder::new();
    for exclude in excludes {
        let glob = portable_glob::parse_portable_glob(exclude).expect("TODO");
        exclude_builder.add(glob);
    }
    // https://github.com/BurntSushi/ripgrep/discussions/2927
    let exclude_matcher = exclude_builder.build().expect("TODO");

    let walkdir_root = "python";
    for entry in GlobWalkDir::new(walkdir_root, &include_globs) {
        let (_entry, relative) = entry.unwrap();

        if relative.starts_with("target") {
            continue;
        }

        println!(
            "{} {} {}",
            relative.display(),
            include_matcher.is_match(&relative),
            exclude_matcher.is_match(&relative),
        );
    }
}
