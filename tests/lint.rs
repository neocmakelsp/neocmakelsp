use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;

#[test]
fn lint_reports_diagnostics_and_exits_non_zero() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".neocmake.toml"), "line_max_words = 20\n").unwrap();
    fs::write(
        dir.path().join("CMakeLists.txt"),
        "cmake_minimum_required(VERSION 3.31)\nproject(demo)\n",
    )
    .unwrap();

    let output = cargo_bin_cmd!()
        .current_dir(dir.path())
        .args(["lint"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("CMakeLists.txt:1:1: [C0301]"),
        "unexpected output: {stdout}"
    );
}

#[test]
fn lint_accepts_a_clean_project() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".neocmake.toml"), "line_max_words = 80\n").unwrap();
    fs::write(dir.path().join("CMakeLists.txt"), "project(demo)\n").unwrap();

    let output = cargo_bin_cmd!()
        .current_dir(dir.path())
        .args(["lint"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn lint_walks_directories_and_skips_other_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("sub")).unwrap();
    fs::write(dir.path().join(".neocmake.toml"), "line_max_words = 20\n").unwrap();
    fs::write(dir.path().join("CMakeLists.txt"), "project(demo)\n").unwrap();
    fs::write(
        dir.path().join("sub").join("global.cmake"),
        "set(VAR aaaaaaaaaa bbbbbbbbbb)\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("notes.txt"),
        "this line is long but not a CMake file\n",
    )
    .unwrap();

    let output = cargo_bin_cmd!()
        .current_dir(dir.path())
        .args(["lint"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("sub/global.cmake:1:1: [C0301]")
            || stdout.contains("sub\\global.cmake:1:1: [C0301]"),
        "unexpected output: {stdout}"
    );
    assert!(!stdout.contains("notes.txt"), "unexpected output: {stdout}");
}
