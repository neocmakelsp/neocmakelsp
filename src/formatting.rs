use lsp_types::{MessageType, Position, TextEdit};
use tower_lsp::lsp_types;

use crate::{consts::TREESITTER_CMAKE_LANGUAGE, utils::treehelper::is_comment, CMakeNodeKinds};

const CLOSURE: &[&str] = &["function_def", "macro_def", "if_condition", "foreach_loop"];

fn strip_trailing_newline(input: &str) -> &str {
    input
        .strip_suffix("\r\n")
        .or(input.strip_suffix('\n'))
        .unwrap_or(input)
}

fn pre_format(line: &str, row: usize, input: tree_sitter::Node) -> String {
    let comment_chars: Vec<usize> = line
        .chars()
        .enumerate()
        .filter(|(_, c)| *c == '#')
        .map(|(i, _)| i)
        .collect();
    for column in comment_chars {
        if column == 0 {
            continue;
        }
        if (is_comment(tree_sitter::Point { row, column }, input)
            // this means it is the extra line, so should think it should be comment line
            || (row == input.end_position().row && column >= input.end_position().column))
            && line.chars().nth(column - 1).unwrap() != ' '
        {
            let linebefore = &line[..column];
            let lineafter = &line[column..];
            return format!("{linebefore} {lineafter}");
        }
    }
    line.to_string()
}

// remove all \r to normal one
fn strip_trailing_newline_document(input: &str) -> String {
    let lines: Vec<&str> = input.lines().map(strip_trailing_newline).collect();
    let mut output = String::new();

    for line in lines {
        output.push_str(line);
        output.push('\n');
    }
    output
}

fn get_space(spacelen: u32, use_space: bool) -> String {
    let unit = if use_space { ' ' } else { '\t' };
    let mut space = String::new();
    for _ in 0..spacelen {
        space.push(unit);
    }
    space
}

// use crate::utils::treehelper::point_to_position;
pub async fn getformat(
    source: &str,
    client: &tower_lsp::Client,
    spacelen: u32,
    use_space: bool,
    insert_final_newline: bool,
) -> Option<Vec<TextEdit>> {
    let source = strip_trailing_newline_document(source);
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(source.as_str(), None).unwrap();

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
            let currentline = pre_format(currentline, start_row + index, input);
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

pub fn get_format_cli(
    source: &str,
    indent_size: u32,
    use_space: bool,
    insert_final_newline: bool,
) -> Option<String> {
    let source = strip_trailing_newline_document(source);
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let tree = parse.parse(&source, None).unwrap();
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

#[test]
fn strip_newline_works() {
    assert_eq!(
        strip_trailing_newline_document("Test0\r\n\r\n"),
        "Test0\n\n"
    );
    assert_eq!(strip_trailing_newline("Test1\r\n"), "Test1");
    assert_eq!(strip_trailing_newline("Test2\n"), "Test2");
    assert_eq!(strip_trailing_newline("Test3"), "Test3");
}

#[cfg(unix)]
#[test]
fn tst_format_function() {
    let source = include_str!("../assert/function/formatbefore.cmake");
    let sourceafter = include_str!("../assert/function/formatafter.cmake");
    let formatstr = get_format_cli(source, 1, false, false).unwrap();
    let formatstr_with_lastline = get_format_cli(source, 1, false, true).unwrap();
    assert_eq!(formatstr.as_str(), sourceafter);
    assert_eq!(formatstr_with_lastline.as_str(), sourceafter);
}

#[cfg(unix)]
#[test]
fn tst_format_base() {
    let source = include_str!("../assert/base/formatbefore.cmake");
    let sourceafter = include_str!("../assert/base/formatafter.cmake");
    let formatstr = get_format_cli(source, 1, false, false).unwrap();
    let formatstr_with_lastline = get_format_cli(source, 1, false, true).unwrap();
    assert_eq!(formatstr.as_str(), sourceafter);
    assert_eq!(formatstr_with_lastline.as_str(), sourceafter);
}

#[cfg(unix)]
#[test]
fn tst_format_lastline() {
    let source = include_str!("../assert/lastline/before.cmake");
    let sourceafter = include_str!("../assert/lastline/after.cmake");
    let formatstr = get_format_cli(source, 4, true, false).unwrap();
    let formatstr_with_lastline = get_format_cli(source, 4, true, true).unwrap();
    assert_eq!(formatstr.as_str(), sourceafter);
    assert_eq!(formatstr_with_lastline.as_str(), sourceafter);
}
