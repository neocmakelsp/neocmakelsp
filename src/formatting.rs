use std::path::Path;
use std::process::Stdio;

use lsp_types::{MessageType, Position, TextEdit};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tower_lsp::lsp_types;

use crate::CMakeNodeKinds;
use crate::config::CMAKE_FORMAT_CONFIG;
use crate::consts::TREESITTER_CMAKE_LANGUAGE;
use crate::utils::treehelper::contain_comment;

const CLOSURE: &[&str] = &[
    CMakeNodeKinds::FUNCTION_DEF,
    CMakeNodeKinds::MACRO_DEF,
    CMakeNodeKinds::IF_CONDITION,
    CMakeNodeKinds::FOREACH_LOOP,
];

/// NOTE: when element in the same place, format bugs
/// for example
/// ```cmake
/// if(${QT_VERSION} STREQUAL 5)  find_package(Qt5ServiceSupport)
///  find_package(Qt5ThemeSupport REQUIRED)
///  find_package(Qt5ThemeSupport REQUIRED)
/// endif()
/// ```
/// with out this function, it will copy the first line twice, this makes bug
fn restrict_format_part<'a>(origin_line: &'a str, row: usize, child: tree_sitter::Node) -> &'a str {
    if row != child.start_position().row {
        return origin_line;
    }
    let mut output = origin_line;
    let start = child.start_position().column;
    let end = child.end_position().column;

    if child.start_position().row == child.end_position().row {
        output = &output[start..end];
    } else {
        output = &output[start..];
    }
    output
}

fn pre_format(
    line: &str,
    row: usize,
    child: tree_sitter::Node,
    input: tree_sitter::Node,
) -> String {
    if child.kind() == CMakeNodeKinds::LINE_COMMENT {
        return restrict_format_part(line, row, child).to_string();
    }
    let comment_chars: Vec<usize> = line
        .chars()
        .enumerate()
        .filter(|(_, c)| *c == '#')
        .map(|(i, _)| i)
        .collect();
    let child_end_column = child.end_position().column;
    let child_end_row = child.end_position().row;
    let mut followed_by_comment = false;
    for column in comment_chars {
        if contain_comment(tree_sitter::Point { row, column }, input)
            // this means it is the extra line, so should think it should be comment line
            || (row == child_end_row && column >= child_end_column && line[child_end_column..column].trim_end().is_empty())
        {
            if column == 0
                || line.chars().nth(column - 1).unwrap() == ' '
                || line.chars().nth(column - 1).unwrap() == '\t'
            {
                followed_by_comment = true;
                break;
            }
            let linebefore = &line[..column];
            let linebefore = restrict_format_part(linebefore, row, child);
            let lineafter = &line[column..];
            return format!("{linebefore} {lineafter}");
        }
    }
    if followed_by_comment {
        line.to_string()
    } else {
        restrict_format_part(line, row, child).to_string()
    }
}

fn get_space(spacelen: u32, use_space: bool) -> String {
    let unit = if use_space { ' ' } else { '\t' };
    let mut space = String::new();
    for _ in 0..spacelen {
        space.push(unit);
    }
    space
}

pub async fn getformat(
    root_path: Option<&Path>,
    source: &str,
    client: &tower_lsp::Client,
    spacelen: u32,
    use_space: bool,
    insert_final_newline: bool,
) -> Option<Vec<TextEdit>> {
    if CMAKE_FORMAT_CONFIG.enable_external {
        let mut cmd = Command::new(&CMAKE_FORMAT_CONFIG.external_program);
        cmd.args(&CMAKE_FORMAT_CONFIG.external_args);
        if let Some(root_path) = root_path {
            cmd.current_dir(root_path);
        }

        let cmd = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn();

        let mut process = match cmd {
            Ok(process) => process,
            Err(err) => {
                client
                    .log_message(
                        MessageType::WARNING,
                        format!("Error running external formatter: {err:?}"),
                    )
                    .await;
                return None;
            }
        };

        let mut stdin = process
            .stdin
            .take()
            .expect("stdin for external formatter should be present");
        if let Err(err) = stdin.write(source.as_bytes()).await {
            client
                .log_message(
                    MessageType::WARNING,
                    format!("Error writing to stdin of external formatter: {err:?}"),
                )
                .await;
            return None;
        }

        let output = process.wait_with_output().await;
        let output = match output {
            Ok(output) => output,
            Err(err) => {
                client
                    .log_message(
                        MessageType::WARNING,
                        format!("Error reading output from external formatter: {err:?}"),
                    )
                    .await;
                return None;
            }
        };

        if !output.status.success() {
            client
                .log_message(
                    MessageType::WARNING,
                    format!(
                        "External formatter exited with error code: {}",
                        output.status.code().unwrap_or(-1)
                    ),
                )
                .await;
            return None;
        }

        let new_source = match String::from_utf8(output.stdout) {
            Err(err) => {
                client
                    .log_message(
                        MessageType::WARNING,
                        format!("Error converting output to UTF-8 string: {err:?}"),
                    )
                    .await;
                return None;
            }
            Ok(new_source) => new_source,
        };

        let lines = new_source.chars().filter(|c| *c == '\n').count();

        return Some(vec![TextEdit {
            range: lsp_types::Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: (lines + 1) as u32,
                    character: 0,
                },
            },
            new_text: new_source,
        }]);
    }

    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(source, None).unwrap();

    if tree.root_node().has_error() {
        client
            .log_message(MessageType::WARNING, "Error source")
            .await;
        return None;
    }
    let (mut new_text, endline) = format_content(
        tree.root_node(),
        &source.lines().collect(),
        spacelen,
        use_space,
        0,
        0,
        0,
    );
    for _ in endline..source.lines().count() {
        new_text.push('\n');
    }

    if insert_final_newline && new_text.chars().last().is_some_and(|c| c != '\n') {
        new_text.push('\n');
    }

    let len_count = new_text.lines().count();
    let len_origin = source.lines().count();
    let len = std::cmp::max(len_count, len_origin);
    Some(vec![TextEdit {
        range: lsp_types::Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: len as u32,
                character: 0,
            },
        },
        new_text,
    }])
}

fn format_content(
    input: tree_sitter::Node,
    newsource: &Vec<&str>,
    spacelen: u32,
    use_space: bool,
    appendtab: u32,
    endline: usize,
    lastendline: usize,
) -> (String, usize) {
    // lastendline is to check if if(A) is the sameline with comment
    let mut lastendline = lastendline;
    let mut endline = endline;
    let mut new_text = String::new();
    let mut course = input.walk();
    // when in body, the firstline is also the firstline of the child
    let mut isfirstunit = true;
    for child in input.children(&mut course) {
        let start_position = child.start_position();
        let end_position = child.end_position();
        let start_row = start_position.row;
        let end_row = end_position.row;
        // if is the commit at the end of line, continue
        if child.kind() == CMakeNodeKinds::LINE_COMMENT
            && endline == start_row
            && (!isfirstunit || start_row == lastendline)
            && !(start_row == 0 && isfirstunit)
        {
            continue;
        }

        if child.kind() == CMakeNodeKinds::BRACKET_COMMENT {
            for _ in endline..start_row {
                new_text.push('\n');
            }
            endline = end_position.row;
            lastendline = end_position.row;
            for comment in newsource.iter().take(endline + 1).skip(start_row) {
                new_text.push_str(comment);
                new_text.push('\n');
            }
            new_text.pop();
            continue;
        }

        for _ in endline..start_row {
            new_text.push('\n');
        }

        endline = start_position.row;
        if CLOSURE.contains(&child.kind()) {
            let (text, newend) = format_content(
                child,
                newsource,
                spacelen,
                use_space,
                appendtab,
                endline,
                lastendline,
            );
            endline = newend;
            lastendline = newend;
            new_text.push_str(&text);
            continue;
        }
        if child.kind() == CMakeNodeKinds::BODY {
            let (text, newend) = format_content(
                child,
                newsource,
                spacelen,
                use_space,
                appendtab + 1,
                endline,
                lastendline,
            );
            new_text.push_str(&text);
            endline = newend;
            continue;
        }

        endline = end_position.row;
        lastendline = end_position.row;

        for (index, currentline) in newsource
            .iter()
            .take(end_row + 1)
            .skip(start_row)
            .enumerate()
        {
            let currentline = pre_format(currentline, start_row + index, child, input);
            let currentline = currentline.trim_end();
            let trimapter = currentline.trim_start();
            let spacesize = currentline.len() - trimapter.len();
            let mut newline = if index != 0 {
                get_space(spacesize as u32, use_space)
            } else {
                let mut firstline = String::new();
                for _ in 0..appendtab {
                    firstline.push_str(&get_space(spacelen, use_space));
                }
                firstline
            };

            let startsource = currentline
                .trim_start()
                .trim_end()
                .split(' ')
                .collect::<Vec<&str>>();
            for unit in startsource {
                newline.push_str(unit);
                newline.push(' ');
            }
            let newline = newline.trim_end();
            new_text.push_str(newline);
            new_text.push('\n');
        }
        new_text = new_text.trim_end().to_string();
        isfirstunit = false;
    }
    (new_text, endline)
}

// Only source from cli need do normalize first
pub fn get_format_cli(
    source: &str,
    indent_size: u32,
    use_space: bool,
    insert_final_newline: bool,
) -> Option<String> {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(source, None).unwrap();
    let input = tree.root_node();
    if input.has_error() {
        return None;
    }
    let (mut new_text, endline) = format_content(
        tree.root_node(),
        &source.lines().collect(),
        indent_size,
        use_space,
        0,
        0,
        0,
    );
    for _ in endline..source.lines().count() {
        new_text.push('\n');
    }

    if insert_final_newline && new_text.chars().last().is_some_and(|c| c != '\n') {
        new_text.push('\n');
    }
    Some(new_text)
}

#[cfg(unix)]
#[test]
fn tst_format_function() {
    let source = include_str!("../assets_for_test/function/formatbefore.cmake");
    let sourceafter = include_str!("../assets_for_test/function/formatafter.cmake");
    let formatstr = get_format_cli(source, 1, false, false).unwrap();
    let formatstr_with_lastline = get_format_cli(source, 1, false, true).unwrap();
    assert_eq!(formatstr.as_str(), sourceafter);
    assert_eq!(formatstr_with_lastline.as_str(), sourceafter);
}

#[cfg(unix)]
#[test]
fn tst_format_base() {
    let source = include_str!("../assets_for_test/base/formatbefore.cmake");
    let sourceafter = include_str!("../assets_for_test/base/formatafter.cmake");
    let formatstr = get_format_cli(source, 1, false, false).unwrap();
    let formatstr_with_lastline = get_format_cli(source, 1, false, true).unwrap();
    assert_eq!(formatstr.as_str(), sourceafter);
    assert_eq!(formatstr_with_lastline.as_str(), sourceafter);
}

#[cfg(unix)]
#[test]
fn tst_format_lastline() {
    let source = include_str!("../assets_for_test/lastline/before.cmake");
    let sourceafter = include_str!("../assets_for_test/lastline/after.cmake");
    let formatstr = get_format_cli(source, 4, true, false).unwrap();
    let formatstr_with_lastline = get_format_cli(source, 4, true, true).unwrap();
    assert_eq!(formatstr.as_str(), sourceafter);
    assert_eq!(formatstr_with_lastline.as_str(), sourceafter);
}
