use crate::glob_walker::{GlobDirMatcher, GlobWalkDir};
use globset::GlobSetBuilder;
use walkdir::WalkDir;

mod glob_walker;
mod portable_glob;

fn main() {
    let includes = ["src/**", "third/*.py", "pyproject.toml", "foo/bar/**"];
    let mut include_globs = Vec::new();
    for include in includes {
        let include = format!("{include}");
        let glob = portable_glob::parse_portable_glob(&include).expect("TODO");
        include_globs.push(glob.clone());
    }

    let excludes = ["__pycache__", "*.pyc", "*.pyo"];
    let mut exclude_builder = GlobSetBuilder::new();
    for exclude in excludes {
        let exclude = if let Some(exclude) = exclude.strip_prefix("/") {
            exclude.to_string()
        } else {
            format!("**/{exclude}").to_string()
        };
        let glob = portable_glob::parse_portable_glob(&exclude).expect("TODO");
        exclude_builder.add(glob);
    }
    // https://github.com/BurntSushi/ripgrep/discussions/2927
    let exclude_matcher = exclude_builder.build().expect("TODO");

    let matcher = GlobDirMatcher::from_globs(&include_globs);

    let walkdir_root = "python";
    for entry in WalkDir::new(walkdir_root)
        .into_iter()
        .filter_entry(|entry| {
            // TODO(konsti): This is should be prettier.
            let relative = entry
                .path()
                .strip_prefix(walkdir_root)
                .expect("walkdir starts with root")
                .to_path_buf();

            matcher.match_directory(&relative) && !exclude_matcher.is_match(&relative)
        })
    {
        let entry = entry.unwrap();
        // TODO(konsti): This is should be prettier.
        let relative = entry
            .path()
            .strip_prefix(walkdir_root)
            .expect("walkdir starts with root")
            .to_path_buf();

        if matcher.match_path(&relative) && !exclude_matcher.is_match(&relative) {
            println!("{}", relative.display());
        };
    }
}
