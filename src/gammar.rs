use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tower_lsp::lsp_types::DiagnosticSeverity;
use tree_sitter::Point;

use crate::config::{self, CMAKE_LINT_CONFIG};
use crate::consts::TREESITTER_CMAKE_LANGUAGE;

pub(crate) struct LintConfigInfo {
    pub use_lint: bool,
    pub use_extra_cmake_lint: bool,
}

/// checkerror the gammer error
/// if there is error , it will return the position of the error
pub struct ErrorInfo {
    pub inner: Vec<(
        tree_sitter::Point,
        tree_sitter::Point,
        String,
        Option<DiagnosticSeverity>,
    )>,
}

pub fn checkerror(
    local_path: &Path,
    source: &str,
    input: tree_sitter::Node<'_>,
    LintConfigInfo {
        use_lint,
        use_extra_cmake_lint,
    }: LintConfigInfo,
) -> Option<ErrorInfo> {
    let cmake_lint_info = if use_lint && use_extra_cmake_lint {
        run_cmake_lint(local_path)
    } else {
        None
    };

    let mut result = checkerror_inner(local_path, &source.lines().collect(), input, use_lint);
    if let Some(v) = cmake_lint_info {
        let error_info = result.get_or_insert(ErrorInfo { inner: vec![] });
        for item in v.inner {
            error_info.inner.push(item);
        }
    };

    result
}

const RE_MATCH_LINT_RESULT: &str =
    r#"(?P<line>\d+)(,(?P<column>\d+))?: (?P<message>\[(?P<severity>[A-Z])\d+\]\s+.*)"#;

fn run_cmake_lint(path: &Path) -> Option<ErrorInfo> {
    if !path.exists() || !CMAKE_LINT_CONFIG.enable_external_cmake_lint {
        return None;
    }

    let output = Command::new("cmake-lint").arg(path).output().ok()?;
    let output_str = String::from_utf8_lossy(&output.stdout);

    let re = regex::Regex::new(RE_MATCH_LINT_RESULT).unwrap();
    let mut info = vec![];

    for input in output_str.lines() {
        if let Some(m) = re.captures(input) {
            let severity = match m.name("severity").unwrap().as_str() {
                "E" => DiagnosticSeverity::ERROR,
                "W" => DiagnosticSeverity::WARNING,
                _ => DiagnosticSeverity::INFORMATION,
            };
            let row = m.name("line").unwrap().as_str().parse().unwrap_or(1) - 1;
            let column = m
                .name("column")
                .map(|m| m.as_str().parse().unwrap())
                .unwrap_or(0);
            let message = m.name("message").unwrap().as_str().to_owned();

            let start_point = Point { row, column };
            let end_point = start_point;
            info.push((start_point, end_point, message, Some(severity)));
        }
    }

    if info.is_empty() {
        None
    } else {
        Some(ErrorInfo { inner: info })
    }
}

fn checkerror_inner(
    local_path: &Path,
    newsource: &Vec<&str>,
    input: tree_sitter::Node,
    use_lint: bool,
) -> Option<ErrorInfo> {
    if input.is_error() {
        return Some(ErrorInfo {
            inner: vec![(
                input.start_position(),
                input.end_position(),
                "Grammar error".to_string(),
                None,
            )],
        });
    }
    let mut course = input.walk();
    let mut output = vec![];
    for node in input.children(&mut course) {
        if let Some(mut tran) = checkerror_inner(local_path, newsource, node, use_lint) {
            output.append(&mut tran.inner);
        }
        if node.kind() != "normal_command" {
            // INFO: NO NEED TO CHECK ANYMORE
            continue;
        }

        let h = node.start_position().row;
        let ids = node.child(0).unwrap();
        //let ids = ids.child(2).unwrap();
        let x = ids.start_position().column;
        let y = ids.end_position().column;
        let name = &newsource[h][x..y];
        if use_lint && !config::CMAKE_LINT.lint_match(name.chars().all(|a| a.is_uppercase())) {
            output.push((
                ids.start_position(),
                ids.end_position(),
                config::CMAKE_LINT.hint.clone(),
                Some(DiagnosticSeverity::HINT),
            ));
        }
        if name.to_lowercase() == "find_package" && node.child_count() >= 4 {
            let mut walk = node.walk();
            let errorpackages = crate::filewatcher::get_error_packages();
            for child in node.children(&mut walk) {
                let h = child.start_position().row;
                let x = child.start_position().column;
                let y = child.end_position().column;
                if h < newsource.len() && y > x && y < newsource[h].len() {
                    let name = &newsource[h][x..y];
                    if errorpackages.contains(&name.to_string()) {
                        output.push((
                            child.start_position(),
                            child.end_position(),
                            "Cannot find such package".to_string(),
                            Some(DiagnosticSeverity::ERROR),
                        ));
                    }
                }
            }
        }
        if name == "include" && node.child_count() >= 4 {
            let Some(ids) = node.child(2) else {
                continue;
            };
            let Some(first_arg_node) = ids.child(0) else {
                continue;
            };
            if ids.start_position().row == ids.end_position().row {
                let h = ids.start_position().row;
                let x = first_arg_node.start_position().column;
                let y = first_arg_node.end_position().column;
                let first_arg = newsource[h][x..y].trim();
                let first_arg = if first_arg.contains('"') {
                    first_arg.split('"').collect::<Vec<&str>>()[1].trim()
                } else {
                    first_arg
                };
                let first_arg = first_arg.replace("\\\\", "\\"); // TODO: proper string escape
                if first_arg.is_empty() {
                    output.push((
                        first_arg_node.start_position(),
                        first_arg_node.end_position(),
                        "Argument is empty".to_string(),
                        Some(DiagnosticSeverity::ERROR),
                    ));
                    continue;
                }
                if first_arg.contains('$') {
                    continue;
                }
                {
                    let path = Path::new(&first_arg);
                    let is_last_char_sep =
                        std::path::is_separator(first_arg.chars().last().unwrap());
                    if !is_last_char_sep && path.extension().is_none() {
                        // first_arg could be a module
                        continue;
                    }
                }
                let include_path = if cfg!(windows) {
                    let path = local_path.parent().unwrap().join(&first_arg);
                    let path_str = path.to_str().unwrap();
                    let path_str = if !first_arg.starts_with('/') && path_str.starts_with('/') {
                        &path.to_str().unwrap()[1..] // remove first slash
                    } else {
                        path.to_str().unwrap()
                    };
                    PathBuf::from(path_str)
                } else {
                    local_path.parent().unwrap().join(&first_arg)
                };
                match include_path.try_exists() {
                    Ok(true) => {
                        if include_path.is_file() {
                            if scanner_include_error(&include_path) {
                                output.push((
                                    first_arg_node.start_position(),
                                    first_arg_node.end_position(),
                                    "Error in include file".to_string(),
                                    Some(DiagnosticSeverity::ERROR),
                                ));
                            }
                        } else {
                            output.push((
                                first_arg_node.start_position(),
                                first_arg_node.end_position(),
                                format!("\"{}\" is a directory", include_path.to_str().unwrap()),
                                Some(DiagnosticSeverity::ERROR),
                            ));
                        }
                    }
                    _ => {
                        output.push((
                            first_arg_node.start_position(),
                            first_arg_node.end_position(),
                            format!(
                                "File \"{}\" does not exist or is inaccessible",
                                include_path.to_str().unwrap()
                            ),
                            Some(DiagnosticSeverity::WARNING),
                        ));
                    }
                }
            }
        }
    }
    if output.is_empty() {
        None
    } else {
        Some(ErrorInfo { inner: output })
    }
}

fn scanner_include_error(path: &PathBuf) -> bool {
    match fs::read_to_string(path) {
        Ok(content) => {
            let mut parse = tree_sitter::Parser::new();
            parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
            let thetree = parse.parse(content, None);
            let tree = thetree.unwrap();
            tree.root_node().has_error()
        }
        Err(_) => true,
    }
}

#[test]
fn gammer_passed_check() {
    let source = include_str!("../assert/gammer/include_check.cmake");
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let thetree = parse.parse(&source, None).unwrap();

    checkerror_inner(
        std::path::Path::new("."),
        &source.lines().collect(),
        thetree.root_node(),
        true,
    );
}

#[test]
fn test_lint_regex() {
    let input = r#"aa.cmake:38,00: [C0305] too many newlines between statements
aa.cmake:46: [C0301] Line too long (84/80)
aa.cmake:51,00: [C0111] Missing docstring on function or macro declaration
aa.cmake:55: [C0301] Line too long (133/80)
aa.cmake:56: [C0301] Line too long (143/80)
aa.cmake:57: [C0301] Line too long (145/80)"#;
    let re = regex::Regex::new(RE_MATCH_LINT_RESULT).unwrap();
    for s in input.split('\n') {
        match re.captures(s) {
            Some(m) => {
                assert!(m.name("line").is_some() && m.name("message").is_some());
                let row = m.name("line").unwrap().as_str().parse().unwrap_or(1) - 1;
                let column = if let Some(m) = m.name("column") {
                    m.as_str().parse().unwrap()
                } else {
                    0
                };
                let message = m.name("message").unwrap().as_str().to_owned();
                println!("{row}:{column} -- {message}");
            }
            None => {
                assert!(false);
            }
        }
    }
}
