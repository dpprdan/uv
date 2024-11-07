use super::*;
use insta::assert_snapshot;

#[test]
fn test_error() {
    let parse_err = |glob| parse_pep639_glob(glob).unwrap_err().to_string();
    assert_snapshot!(
        parse_err(".."),
        @"The parent directory operator (`..`) at position 0 is not allowed in glob: `..`"
    );
    assert_snapshot!(
        parse_err("licenses/.."),
        @"The parent directory operator (`..`) at position 9 is not allowed in glob: `licenses/..`"
    );
    assert_snapshot!(
        parse_err("licenses/LICEN!E.txt"),
        @"Invalid character `!` at position 14 in glob: `licenses/LICEN!E.txt`"
    );
    assert_snapshot!(
        parse_err("licenses/LICEN[!C]E.txt"),
        @"Invalid character `!` at position 15 in glob: `licenses/LICEN[!C]E.txt`"
    );
    assert_snapshot!(
        parse_err("licenses/LICEN[C?]E.txt"),
        @"Invalid character `?` at position 16 in glob: `licenses/LICEN[C?]E.txt`"
    );
    assert_snapshot!(
        parse_err("******"),
        @"Too many at stars at position 0 in glob: `******`"
    );
    assert_snapshot!(
        parse_err("licenses/**license"),
        @"Too many at stars at position 9 in glob: `licenses/**license`"
    );
    assert_snapshot!(
        parse_err("licenses/***/licenses.csv"),
        @"Too many at stars at position 9 in glob: `licenses/***/licenses.csv`"
    );
    assert_snapshot!(
        parse_err(r"licenses\eula.txt"),
        @r"Invalid character `\` at position 8 in glob: `licenses\eula.txt`"
    );
}

#[test]
fn test_valid() {
    let cases = [
        "licenses/*.txt",
        "licenses/**/*.txt",
        "LICEN[CS]E.txt",
        "LICEN?E.txt",
        "[a-z].txt",
        "[a-z._-].txt",
        "*/**",
        "LICENSE..txt",
        "LICENSE_file-1.txt",
        // (google translate)
        "licenses/라이센스*.txt",
        "licenses/ライセンス*.txt",
        "licenses/执照*.txt",
    ];
    for case in cases {
        parse_pep639_glob(case).unwrap();
    }
}
