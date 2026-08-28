use std::path::Path;
use std::process::Command;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position};
use tree_sitter::{Point, Query, QueryCursor, StreamingIterator};
use unicode_segmentation::UnicodeSegmentation;

use crate::config::{self, CONFIG, CommandCase};
use crate::consts::TREESITTER_CMAKE_LANGUAGE;
use crate::utils::query::{get_functions, get_macros, get_normal_commands};
use crate::utils::treehelper::ToPosition;
use crate::utils::{NeoStrExt, include_is_module};

const INCLUDE_CHECK_KEYWORDS: &[&str; 2] = &["include", "add_subdirectory"];

pub struct LintConfigInfo {
    pub use_lint: bool,
    pub use_extra_cmake_lint: bool,
}

trait CharacterCount {
    fn character_counts(&self) -> usize;
}

impl CharacterCount for str {
    fn character_counts(&self) -> usize {
        self.graphemes(true).count()
    }
}
impl CharacterCount for String {
    fn character_counts(&self) -> usize {
        self.graphemes(true).count()
    }
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum ErrorType {
    UpLowerCase {
        command_case: CommandCase,
        name: String,
    },
    Length {
        length: u32,
        max: u32,
    },
    Gammar,
    #[default]
    Other,
}

pub fn checkerror<P: AsRef<Path>>(
    local_path: &P,
    source: &str,
    LintConfigInfo {
        use_lint,
        use_extra_cmake_lint,
    }: LintConfigInfo,
) -> Option<Vec<Diagnostic>> {
    let newsource = source.lines().collect();
    let cmake_lint_info = if use_lint {
        run_cmake_lint(local_path, use_extra_cmake_lint, &newsource)
    } else {
        None
    };
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let thetree = parse.parse(source, None)?;
    let mut result = checkerror_inner(local_path, source, thetree.root_node(), use_lint);
    if let Some(v) = cmake_lint_info {
        let error_info = result.get_or_insert(vec![]);
        error_info.extend(v);
    }

    result
}

const RE_MATCH_LINT_RESULT: &str =
    r"(?P<line>\d+)(,(?P<column>\d+))?: (?P<message>\[(?P<severity>[A-Z])\d+\]\s+.*)";

static LINT_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(RE_MATCH_LINT_RESULT).unwrap());

fn run_cmake_lint<P: AsRef<Path>>(
    path: P,
    use_extra_cmake_lint: bool,
    contexts: &Vec<&str>,
) -> Option<Vec<Diagnostic>> {
    if use_extra_cmake_lint {
        return run_extra_lint(path);
    }
    let mut info = vec![];
    let max_len = CONFIG.line_max_words;
    for (index, line) in contexts.iter().enumerate() {
        let len = line.character_counts();
        if len > max_len {
            let start_point = Point {
                row: index,
                column: 0,
            };
            let end_point = Point {
                row: index,
                column: len,
            };
            let message = format!("[C0301] Line too long ({len}/{max_len})");
            let pointx = start_point.to_position();
            let pointy = end_point.to_position();
            use tower_lsp::lsp_types::Range;
            let range = Range {
                start: pointx,
                end: pointy,
            };
            info.push(Diagnostic {
                range,
                message: message.into(),
                severity: Some(DiagnosticSeverity::Warning),
                code: None,
                code_description: None,
                source: None,
                related_information: None,
                tags: None,
                data: Some(
                    serde_json::to_value(ErrorType::Length {
                        length: len as u32,
                        max: max_len as u32,
                    })
                    .unwrap(),
                ),
            });
        }
    }
    if info.is_empty() { None } else { Some(info) }
}
pub static LENGTH_LINT_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"((?<length>\d+)/(?<max>\d+))").unwrap());

fn run_extra_lint<P: AsRef<Path>>(path: P) -> Option<Vec<Diagnostic>> {
    let path = path.as_ref();
    if !path.exists() {
        return None;
    }

    let output = Command::new("cmake-lint").arg(path).output().ok()?;
    let output_str = String::from_utf8_lossy(&output.stdout);

    let mut info = vec![];

    for input in output_str.lines() {
        if let Some(m) = LINT_REGEX.captures(input) {
            let severity = match m.name("severity").unwrap().as_str() {
                "E" => DiagnosticSeverity::Error,
                "W" => DiagnosticSeverity::Warning,
                _ => DiagnosticSeverity::Information,
            };
            let row = m.name("line").unwrap().as_str().parse().unwrap_or(1) - 1;
            let column = m
                .name("column")
                .map(|m| m.as_str().parse().unwrap())
                .unwrap_or(0);
            let message = m.name("message").unwrap().as_str().to_owned();

            let error_type = if message.starts_with("[C0301]")
                && let Some(caps) = LENGTH_LINT_REGEX.captures(&message)
                && let Ok(length) = caps["length"].parse()
                && let Ok(max) = caps["max"].parse()
            {
                ErrorType::Length { length, max }
            } else {
                ErrorType::Other
            };
            let start = Position {
                line: row,
                character: column,
            };
            let range = tower_lsp::lsp_types::Range { start, end: start };
            info.push(Diagnostic {
                range,
                message: message.into(),
                severity: Some(severity),
                code: None,
                code_description: None,
                source: None,
                related_information: None,
                tags: None,
                data: Some(serde_json::to_value(error_type).unwrap()),
            });
        }
    }

    if info.is_empty() { None } else { Some(info) }
}

const ERROR_QUERY: &str = r"
(
    (ERROR) @error
)
";

fn checkerror_inner<P: AsRef<Path>>(
    local_path: P,
    source: &str,
    input: tree_sitter::Node,
    use_lint: bool,
) -> Option<Vec<Diagnostic>> {
    use tower_lsp::lsp_types::Range;
    if input.is_error() {
        let pointx = input.start_position().to_position();
        let pointy = input.end_position().to_position();
        let range = Range {
            start: pointx,
            end: pointy,
        };
        return Some(vec![Diagnostic {
            range,
            message: "Grammar error".into(),
            severity: Some(DiagnosticSeverity::Error),
            code: None,
            code_description: None,
            source: None,
            related_information: None,
            tags: None,
            data: Some(serde_json::to_value(ErrorType::Gammar).unwrap()),
        }]);
    }
    let source_bytes = source.as_bytes();
    let local_path = local_path.as_ref();
    let mut output = vec![];

    let query_error = Query::new(&TREESITTER_CMAKE_LANGUAGE, ERROR_QUERY).unwrap();
    let mut cursor_e = QueryCursor::new();
    let mut matches_e = cursor_e.matches(&query_error, input, source_bytes);
    while let Some(m) = matches_e.next() {
        for err in m.captures {
            let input = err.node;
            let pointx = input.start_position().to_position();
            let pointy = input.end_position().to_position();
            let range = Range {
                start: pointx,
                end: pointy,
            };
            output.push(Diagnostic {
                range,
                message: "Grammar error".into(),
                severity: Some(DiagnosticSeverity::Error),
                code: None,
                code_description: None,
                source: None,
                related_information: None,
                tags: None,
                data: Some(serde_json::to_value(ErrorType::Gammar).unwrap()),
            });
        }
    }
    if use_lint && let Some(command_case) = config::CONFIG.command_case {
        let macros = get_macros(source_bytes, input, None);
        let functions = get_functions(source_bytes, input, None);
        for macro_node in macros {
            let name = macro_node.name;
            let Some(hint) = command_case.check(name) else {
                continue;
            };
            let pointx = macro_node.name_node.start_position().to_position();
            let pointy = macro_node.name_node.end_position().to_position();
            let range = Range {
                start: pointx,
                end: pointy,
            };

            output.push(Diagnostic {
                range,
                message: hint.into(),
                severity: Some(DiagnosticSeverity::Hint),
                code: None,
                code_description: None,
                source: None,
                related_information: None,
                tags: None,
                data: Some(
                    serde_json::to_value(ErrorType::UpLowerCase {
                        command_case,
                        name: name.to_owned(),
                    })
                    .unwrap(),
                ),
            });
        }
        for fun_node in functions {
            let name = fun_node.name;
            let Some(hint) = command_case.check(name) else {
                continue;
            };
            let pointx = fun_node.name_node.start_position().to_position();
            let pointy = fun_node.name_node.end_position().to_position();
            let range = Range {
                start: pointx,
                end: pointy,
            };

            output.push(Diagnostic {
                range,
                message: hint.into(),
                severity: Some(DiagnosticSeverity::Hint),
                code: None,
                code_description: None,
                source: None,
                related_information: None,
                tags: None,
                data: Some(
                    serde_json::to_value(ErrorType::UpLowerCase {
                        command_case,
                        name: name.to_owned(),
                    })
                    .unwrap(),
                ),
            });
        }
    }
    let commands = get_normal_commands(source_bytes, input, None);
    for query_now in commands {
        let name = query_now.identifier;
        let name_node = query_now.identifier_node;
        if use_lint
            && let Some(command_case) = config::CONFIG.command_case
            && let Some(hint) = command_case.check(name)
        {
            let pointx = name_node.start_position().to_position();
            let pointy = name_node.end_position().to_position();
            let range = Range {
                start: pointx,
                end: pointy,
            };
            output.push(Diagnostic {
                range,
                message: hint.into(),
                severity: Some(DiagnosticSeverity::Hint),
                code: None,
                code_description: None,
                source: None,
                related_information: None,
                tags: None,
                data: Some(
                    serde_json::to_value(ErrorType::UpLowerCase {
                        command_case,
                        name: name.to_owned(),
                    })
                    .unwrap(),
                ),
            });
        }
        let lowercase_name = name.to_lowercase();
        if lowercase_name == "find_package" {
            let errorpackages = crate::filewatcher::get_error_packages();
            if errorpackages.is_empty() {
                continue;
            }

            for child in query_now.args {
                let name = &child.utf8_text(source_bytes).unwrap();
                let pointx = child.start_position().to_position();
                let pointy = child.end_position().to_position();
                let range = Range {
                    start: pointx,
                    end: pointy,
                };
                if errorpackages.contains(&name.to_string()) {
                    output.push(Diagnostic {
                        range,
                        message: "Cannot find such package".into(),
                        severity: Some(DiagnosticSeverity::Error),
                        code: None,
                        code_description: None,
                        source: None,
                        related_information: None,
                        tags: None,
                        data: Some(serde_json::to_value(ErrorType::Other).unwrap()),
                    });
                }
            }
            continue;
        }
        if INCLUDE_CHECK_KEYWORDS.contains(&lowercase_name.as_str()) && !query_now.args.is_empty() {
            let is_sub_directory = lowercase_name == "add_subdirectory";
            let Some(parent_path) = local_path.parent() else {
                continue;
            };
            let Some(first_arg) = query_now.first_arg else {
                continue;
            };
            let Some(first_arg) = first_arg.try_replace_placeholders() else {
                continue;
            };
            let first_arg_node = query_now.args[0];
            let first_arg = first_arg.replace("\\\\", "\\"); // TODO: proper string escape
            if first_arg.is_empty() {
                let pointx = first_arg_node.start_position().to_position();
                let pointy = first_arg_node.end_position().to_position();
                let range = Range {
                    start: pointx,
                    end: pointy,
                };
                output.push(Diagnostic {
                    range,
                    message: "Argument is empty".into(),
                    severity: Some(DiagnosticSeverity::Error),
                    code: None,
                    code_description: None,
                    source: None,
                    related_information: None,
                    tags: None,
                    data: Some(serde_json::to_value(ErrorType::Other).unwrap()),
                });
                continue;
            }
            if !is_sub_directory && include_is_module(&first_arg) {
                continue;
            }
            let sub_path = Path::new(&first_arg);
            let include_path = if sub_path.is_absolute() {
                sub_path.to_path_buf()
            } else {
                parent_path.join(sub_path)
            };
            match include_path.try_exists() {
                Ok(true) => {
                    if include_path.is_file() {
                        if scanner_include_error(include_path) {
                            let pointx = first_arg_node.start_position().to_position();
                            let pointy = first_arg_node.end_position().to_position();
                            let range = Range {
                                start: pointx,
                                end: pointy,
                            };
                            output.push(Diagnostic {
                                range,
                                message: "Error in include file".into(),
                                severity: Some(DiagnosticSeverity::Error),
                                code: None,
                                code_description: None,
                                source: None,
                                related_information: None,
                                tags: None,
                                data: Some(serde_json::to_value(ErrorType::Other).unwrap()),
                            });
                        }
                    } else {
                        if lowercase_name == "add_subdirectory" {
                            continue;
                        }
                        let pointx = first_arg_node.start_position().to_position();
                        let pointy = first_arg_node.end_position().to_position();
                        let range = Range {
                            start: pointx,
                            end: pointy,
                        };
                        output.push(Diagnostic {
                            range,
                            message: format!(
                                "\"{}\" is a directory",
                                include_path.to_str().unwrap()
                            )
                            .into(),
                            severity: Some(DiagnosticSeverity::Error),
                            code: None,
                            code_description: None,
                            source: None,
                            related_information: None,
                            tags: None,
                            data: Some(serde_json::to_value(ErrorType::Other).unwrap()),
                        });
                    }
                }
                _ => {
                    let message = if is_sub_directory {
                        format!(
                            "Directory \"{}\" does not exist or is inaccessible",
                            include_path.to_str().unwrap()
                        )
                    } else {
                        format!(
                            "File \"{}\" does not exist or is inaccessible",
                            include_path.to_str().unwrap()
                        )
                    };
                    let pointx = first_arg_node.start_position().to_position();
                    let pointy = first_arg_node.end_position().to_position();
                    let range = Range {
                        start: pointx,
                        end: pointy,
                    };
                    output.push(Diagnostic {
                        range,
                        message: message.into(),
                        severity: Some(DiagnosticSeverity::Warning),
                        code: None,
                        code_description: None,
                        source: None,
                        related_information: None,
                        tags: None,
                        data: Some(serde_json::to_value(ErrorType::Other).unwrap()),
                    });
                }
            }
        }
    }

    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

// Used to check if root_node has error
fn scanner_include_error<P: AsRef<Path>>(path: P) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return true;
    };
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let Some(tree) = parse.parse(content, None) else {
        return true;
    };
    tree.root_node().has_error()
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Write;

    use tempfile::tempdir;

    use super::*;
    #[cfg(not(windows))]
    use crate::fileapi::{cache, set_cache_data};

    #[cfg(not(windows))]
    #[test]
    fn test_gammar_check() {
        let dir = tempdir().unwrap();

        let json_value = format!(
            "{{
    \"kind\" : \"cache\",
    \"version\" :
    {{
        \"major\" : 2,
        \"minor\" : 0
    }},
    \"entries\" :
    [
        {{
            \"name\" : \"ROOT_DIR\",
            \"properties\" :
            [
            ],
            \"type\" : \"FILEPATH\",
            \"value\" : \"{}\"
        }}
    ]
    }}",
            dir.path().display()
        );
        let template_cache: cache::Cache = serde_json::from_str(&json_value).unwrap();
        set_cache_data(template_cache);
        let gammar_file_src = r#"
include("${ROOT_DIR}/hello.cmake")
include("${ROOT_DIR}/hello_unexist.cmake")
add_subdirectory("${ROOT_DIR}")
add_subdirectory("unexist_subdir")
"#;
        let top_cmake = dir.path().join("CMakeList.txt");
        let mut top_cmake_file = File::create(&top_cmake).unwrap();
        writeln!(top_cmake_file, "{}", gammar_file_src).unwrap();

        let hello_cmake = dir.path().join("hello.cmake");
        File::create(hello_cmake).unwrap();

        let hello_cmake_error = dir.path().join("hello_unexist.cmake");

        let unexist_subdir = dir.path().join("unexist_subdir");
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let thetree = parse.parse(gammar_file_src, None).unwrap();

        let check_result =
            checkerror_inner(top_cmake, gammar_file_src, thetree.root_node(), false).unwrap();

        use tower_lsp::lsp_types::Range;
        assert_eq!(
            *check_result,
            vec![
                Diagnostic {
                    range: Range {
                        start: Position {
                            line: 2,
                            character: 8
                        },
                        end: Position {
                            line: 2,
                            character: 41
                        }
                    },
                    message: format!(
                        "File \"{}\" does not exist or is inaccessible",
                        hello_cmake_error.display()
                    )
                    .into(),
                    severity: Some(DiagnosticSeverity::Warning),
                    code: None,
                    code_description: None,
                    source: None,
                    related_information: None,
                    tags: None,
                    data: Some(serde_json::to_value(ErrorType::Other).unwrap()),
                },
                Diagnostic {
                    range: Range {
                        start: Position {
                            line: 4,
                            character: 17
                        },
                        end: Position {
                            line: 4,
                            character: 33
                        },
                    },
                    message: format!(
                        "Directory \"{}\" does not exist or is inaccessible",
                        unexist_subdir.display()
                    )
                    .into(),
                    severity: Some(DiagnosticSeverity::Warning),
                    code: None,
                    code_description: None,
                    source: None,
                    related_information: None,
                    tags: None,
                    data: Some(serde_json::to_value(ErrorType::Other).unwrap()),
                },
            ]
        );
    }

    #[test]
    fn include_error_test() {
        let dir = tempdir().unwrap();

        let bad_cmake = dir.path().join("test.cmake");

        let bad_context = r"
include((()
";
        let mut bad_file = File::create(&bad_cmake).unwrap();

        writeln!(bad_file, "{}", bad_context).unwrap();

        assert!(scanner_include_error(bad_cmake));

        let good_cmake = dir.path().join("test2.cmake");

        let good_context = r"
include(abcd.text)
";
        let mut good_file = File::create(&good_cmake).unwrap();

        writeln!(good_file, "{}", good_context).unwrap();

        assert!(!scanner_include_error(good_cmake));
    }

    #[test]
    fn gammer_passed_check_1() {
        let source = include_str!("../assets_for_test/gammar/include_check.cmake");
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let thetree = parse.parse(source, None).unwrap();

        use tower_lsp::lsp_types::Range;
        let input = thetree.root_node();
        assert_eq!(
            checkerror_inner(std::path::Path::new("."), source, input, true,),
            Some(vec![Diagnostic {
                range: Range {
                    start: input.start_position().to_position(),
                    end: input.end_position().to_position()
                },
                message: "Grammar error".into(),
                severity: Some(DiagnosticSeverity::Error),
                code: None,
                code_description: None,
                source: None,
                related_information: None,
                tags: None,
                data: Some(serde_json::to_value(ErrorType::Gammar).unwrap()),
            }])
        );
    }

    #[test]
    fn gammer_passed_check_2() {
        let source = include_str!("../assets_for_test/gammar/pass_test.cmake");
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let thetree = parse.parse(source, None).unwrap();

        assert!(
            checkerror_inner(std::path::Path::new("."), source, thetree.root_node(), true,)
                .is_none()
        );
    }

    #[test]
    fn test_lint_regex() {
        let input = r"aa.cmake:38,00: [C0305] too many newlines between statements
aa.cmake:46: [C0301] Line too long (84/80)
aa.cmake:51,00: [C0111] Missing docstring on function or macro declaration
aa.cmake:55: [C0301] Line too long (133/80)
aa.cmake:56: [C0301] Line too long (143/80)
aa.cmake:57: [C0301] Line too long (145/80)";
        let re = regex::Regex::new(RE_MATCH_LINT_RESULT).unwrap();
        for s in input.split('\n') {
            let m = re.captures(s).unwrap();
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
    }
    #[test]
    fn lint_regex_text() {
        let information = "[C0301] Line too long (92/80)";
        let caps = LENGTH_LINT_REGEX.captures(information).unwrap();
        assert_eq!(&caps["length"], "92");
        assert_eq!(&caps["max"], "80");
    }

    #[test]
    fn error_type_serde() {
        let data = serde_json::json!({
            "Length": {
                "length": 96,
                "max": 80
            }
        });

        let result: ErrorType = serde_json::from_value(data).unwrap();

        assert_eq!(
            result,
            ErrorType::Length {
                length: 96,
                max: 80
            }
        );
        let data = serde_json::json!({
            "UpLowerCase": {
                "command_case": "upcase",
                "name": "Hello"
            }
        });

        let result: ErrorType = serde_json::from_value(data).unwrap();

        assert_eq!(
            result,
            ErrorType::UpLowerCase {
                command_case: CommandCase::Upper,
                name: "Hello".to_owned()
            }
        );
    }

    #[test]
    fn character_counts() {
        assert_eq!("é".character_counts(), 1);
        assert_eq!("ラウトは難しいです！".character_counts(), 10);
        assert_eq!("#яяяяяяяяяяяяяяяяяяяяяяяяяяяяяяяяяяяяяяяяяя".character_counts(), 43);
        assert_eq!("#йцукенйцукенйцукенйцукенйцукенйцукенйцукен".character_counts(), 43);
    }
}
