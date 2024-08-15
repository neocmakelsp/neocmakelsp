use tower_lsp::{
    lsp_types::{SemanticToken, SemanticTokenType, SemanticTokens, SemanticTokensResult},
    Client,
};

use std::sync::LazyLock;

use crate::consts::TREESITTER_CMAKE_LANGUAGE;

use crate::CMakeNodeKinds;
static NUMBERREGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^\d+(?:\.+\d*)?").unwrap());

const BOOL_VAL: &[&str] = &["ON", "OFF", "TRUE", "FALSE"];
const UNIQUE_KEYWORD: &[&str] = &["AND", "NOT"];

pub const LEGEND_TYPE: &[SemanticTokenType] = &[
    SemanticTokenType::FUNCTION,
    SemanticTokenType::METHOD,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::STRING,
    SemanticTokenType::COMMENT,
    SemanticTokenType::NUMBER,
    SemanticTokenType::KEYWORD,
    SemanticTokenType::OPERATOR,
    SemanticTokenType::PARAMETER,
];

fn get_token_position(tokentype: SemanticTokenType) -> u32 {
    LEGEND_TYPE
        .iter()
        .position(|data| *data == tokentype)
        .unwrap() as u32
}

pub async fn semantic_token(_client: &Client, context: &str) -> Option<SemanticTokensResult> {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let thetree = parse.parse(context, None);
    let tree = thetree?;
    Some(SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data: sub_tokens(
            tree.root_node(),
            &context.lines().collect(),
            &mut 0,
            &mut 0,
            false,
        ),
    }))
}

fn sub_tokens(
    input: tree_sitter::Node,
    source: &Vec<&str>,
    preline: &mut u32,
    prestart: &mut u32,
    is_if: bool,
) -> Vec<SemanticToken> {
    let mut res = vec![];

    let mut course = input.walk();

    for child in input.children(&mut course) {
        match child.kind() {
            "$" | "{" | "}" => {
                let h = child.start_position().row;
                let x = child.start_position().column;
                let y = child.end_position().column;
                if h as u32 != *preline {
                    *prestart = 0;
                }
                res.push(SemanticToken {
                    delta_line: h as u32 - *preline,
                    delta_start: x as u32 - *prestart,
                    length: (y - x) as u32,
                    token_type: get_token_position(SemanticTokenType::OPERATOR),
                    token_modifiers_bitset: 0,
                });
                *preline = h as u32;
                *prestart = x as u32;
            }
            CMakeNodeKinds::VARIABLE => {
                let h = child.start_position().row;
                let x = child.start_position().column;
                let y = child.end_position().column;
                if h as u32 != *preline {
                    *prestart = 0;
                }
                res.push(SemanticToken {
                    delta_line: h as u32 - *preline,
                    delta_start: x as u32 - *prestart,
                    length: (y - x) as u32,
                    token_type: get_token_position(SemanticTokenType::VARIABLE),
                    token_modifiers_bitset: 0,
                });
                *preline = h as u32;
                *prestart = x as u32;
            }
            CMakeNodeKinds::NORMAL_COMMAND => {
                // NOTE: identifier
                let Some(id) = child.child(0) else {
                    continue;
                };

                let h = id.start_position().row;
                let x = id.start_position().column;
                let y = id.end_position().column;

                if h as u32 != *preline {
                    *prestart = 0;
                }

                res.push(SemanticToken {
                    delta_line: h as u32 - *preline,
                    delta_start: x as u32 - *prestart,
                    length: (y - x) as u32,
                    token_type: get_token_position(SemanticTokenType::METHOD),
                    token_modifiers_bitset: 0,
                });
                *preline = h as u32;
                *prestart = x as u32;

                res.append(&mut sub_tokens(child, source, preline, prestart, false));
            }

            CMakeNodeKinds::LINE_COMMENT => {
                let h = child.start_position().row;
                let x = child.start_position().column;
                let y = child.end_position().column;
                if h as u32 != *preline {
                    *prestart = 0;
                }
                res.push(SemanticToken {
                    delta_line: h as u32 - *preline,
                    delta_start: x as u32 - *prestart,
                    length: (y - x) as u32,
                    token_type: get_token_position(SemanticTokenType::COMMENT),
                    token_modifiers_bitset: 0,
                });
                *preline = h as u32;
                *prestart = x as u32;
            }

            CMakeNodeKinds::ENDMACRO_COMMAND
            | CMakeNodeKinds::ENDIF_COMMAND
            | CMakeNodeKinds::ENDFUNCTION_COMMAND
            | CMakeNodeKinds::ELSE_COMMAND
            | CMakeNodeKinds::ENDFOREACH_COMMAND => {
                let Some(id) = child.child(0) else {
                    continue;
                };
                let h = id.start_position().row;
                let x = id.start_position().column;
                let y = id.end_position().column;
                if h as u32 != *preline {
                    *prestart = 0;
                }
                res.push(SemanticToken {
                    delta_line: h as u32 - *preline,
                    delta_start: x as u32 - *prestart,
                    length: (y - x) as u32,
                    token_type: get_token_position(SemanticTokenType::KEYWORD),
                    token_modifiers_bitset: 0,
                });
                *preline = h as u32;
                *prestart = x as u32;
            }
            CMakeNodeKinds::ARGUMENT_LIST => {
                let mut argument_course = child.walk();
                let mut is_first_val = !is_if; // NOTE: if is if, not check it
                for argument in child.children(&mut argument_course) {
                    let h = argument.start_position().row;
                    let x = argument.start_position().column;
                    let y = argument.end_position().column;
                    if h as u32 != *preline {
                        *prestart = 0;
                    }
                    if argument.kind() == "line_comment" {
                        res.push(SemanticToken {
                            delta_line: h as u32 - *preline,
                            delta_start: x as u32 - *prestart,
                            length: (y - x) as u32,
                            token_type: get_token_position(SemanticTokenType::COMMENT),
                            token_modifiers_bitset: 0,
                        });
                        *preline = h as u32;
                        *prestart = x as u32;
                        is_first_val = false;
                        continue;
                    }
                    if argument
                        .child(0)
                        .is_some_and(|child| child.kind() == "quoted_argument")
                    {
                        let quoted_argument = argument.child(0).unwrap();
                        if quoted_argument.child_count() == 1 {
                            res.push(SemanticToken {
                                delta_line: h as u32 - *preline,
                                delta_start: x as u32 - *prestart,
                                length: (y - x) as u32,
                                token_type: get_token_position(SemanticTokenType::STRING),
                                token_modifiers_bitset: 0,
                            });
                            *prestart = x as u32;
                            *preline = h as u32;
                        } else {
                            // TODO: very base implement, but it is enough for me,
                            // if you do not very satisfied with this
                            // implement, I am glad to accept your pr, thanks
                            // NOTE: highlight variable in string
                            let mut quoted_argument_course = quoted_argument.walk();
                            for element in quoted_argument.children(&mut quoted_argument_course) {
                                let h = element.start_position().row;
                                let x = element.start_position().column;
                                let y = element.end_position().column;
                                if element.kind() == "quoted_element" {
                                    let mut quoted_element_walk = element.walk();
                                    for variable in element.children(&mut quoted_element_walk) {
                                        if variable.kind() != "variable_ref" {
                                            continue;
                                        }
                                        let h = variable.start_position().row;
                                        let x = variable.start_position().column;
                                        let y = variable.end_position().column;
                                        res.push(SemanticToken {
                                            delta_line: h as u32 - *preline,
                                            delta_start: x as u32 - *prestart,
                                            length: (y - x) as u32,
                                            token_type: get_token_position(
                                                SemanticTokenType::VARIABLE,
                                            ),
                                            token_modifiers_bitset: 0,
                                        });
                                        *prestart = x as u32;
                                        *preline = h as u32;
                                    }
                                } else {
                                    res.push(SemanticToken {
                                        delta_line: h as u32 - *preline,
                                        delta_start: x as u32 - *prestart,
                                        length: (y - x) as u32,
                                        token_type: get_token_position(SemanticTokenType::STRING),
                                        token_modifiers_bitset: 0,
                                    });
                                    *prestart = x as u32;
                                    *preline = h as u32;
                                }
                            }
                        }
                        is_first_val = false;
                        continue;
                    }
                    if argument
                        .child(0)
                        .is_some_and(|child| child.kind() == "bracket_argument")
                    {
                        let bracket_argument = argument.child(0).unwrap();
                        let h = bracket_argument.start_position().row;
                        let h2 = bracket_argument.end_position().row;
                        let x = bracket_argument.start_position().column;
                        let y = bracket_argument.end_position().column;
                        for column in h..=h2 {
                            if column == h {
                                let content = &source[h][x..];
                                res.push(SemanticToken {
                                    delta_line: h as u32 - *preline,
                                    delta_start: x as u32 - *prestart,
                                    length: content.len() as u32,
                                    token_type: get_token_position(SemanticTokenType::STRING),
                                    token_modifiers_bitset: 0,
                                });
                                *prestart = x as u32;
                                *preline = h as u32;
                                continue;
                            }
                            if column == h2 {
                                let content = &source[h2][..y];
                                res.push(SemanticToken {
                                    delta_line: h2 as u32 - *preline,
                                    delta_start: 0,
                                    length: content.len() as u32,
                                    token_type: get_token_position(SemanticTokenType::STRING),
                                    token_modifiers_bitset: 0,
                                });
                                *prestart = 0;
                                *preline = h2 as u32;
                                continue;
                            }
                            let content = &source[column];
                            res.push(SemanticToken {
                                delta_line: column as u32 - *preline,
                                delta_start: 0,
                                length: content.len() as u32,
                                token_type: get_token_position(SemanticTokenType::STRING),
                                token_modifiers_bitset: 0,
                            });
                            *prestart = 0;
                            *preline = column as u32;
                        }
                        is_first_val = false;
                        continue;
                    }
                    if argument
                        .child(0)
                        .is_some_and(|child| child.child_count() != 0)
                    {
                        res.append(&mut sub_tokens(
                            argument.child(0).unwrap(),
                            source,
                            preline,
                            prestart,
                            false,
                        ));
                        is_first_val = false;
                        continue;
                    }
                    let name = &source[h][x..y];
                    if BOOL_VAL.contains(&name) {
                        res.push(SemanticToken {
                            delta_line: h as u32 - *preline,
                            delta_start: x as u32 - *prestart,
                            length: (y - x) as u32,
                            token_type: get_token_position(SemanticTokenType::VARIABLE),
                            token_modifiers_bitset: 0,
                        });
                        *prestart = x as u32;
                        *preline = h as u32;
                        is_first_val = false;
                        continue;
                    }
                    if NUMBERREGEX.is_match(name) {
                        res.push(SemanticToken {
                            delta_line: h as u32 - *preline,
                            delta_start: x as u32 - *prestart,
                            length: (y - x) as u32,
                            token_type: get_token_position(SemanticTokenType::NUMBER),
                            token_modifiers_bitset: 0,
                        });
                        *prestart = x as u32;
                        *preline = h as u32;
                        continue;
                    }
                    if UNIQUE_KEYWORD.contains(&name) {
                        res.push(SemanticToken {
                            delta_line: h as u32 - *preline,
                            delta_start: x as u32 - *prestart,
                            length: (y - x) as u32,
                            token_type: get_token_position(SemanticTokenType::KEYWORD),
                            token_modifiers_bitset: 0,
                        });
                        *prestart = x as u32;
                        *preline = h as u32;
                        is_first_val = false;
                        continue;
                    }
                    if name.chars().all(|a| !a.is_lowercase()) && !is_if {
                        res.push(SemanticToken {
                            delta_line: h as u32 - *preline,
                            delta_start: x as u32 - *prestart,
                            length: (y - x) as u32,
                            token_type: get_token_position(SemanticTokenType::KEYWORD),
                            token_modifiers_bitset: 0,
                        });
                        *prestart = x as u32;
                        *preline = h as u32;
                        is_first_val = false;
                        continue;
                    }
                    if is_first_val {
                        res.push(SemanticToken {
                            delta_line: h as u32 - *preline,
                            delta_start: x as u32 - *prestart,
                            length: (y - x) as u32,
                            token_type: get_token_position(SemanticTokenType::VARIABLE),
                            token_modifiers_bitset: 0,
                        });
                        *prestart = x as u32;
                        *preline = h as u32;
                    }
                    is_first_val = false;
                }
            }
            "function" | "macro" | "if" | "foreach" | "elseif" => {
                let h = child.start_position().row;
                let x = child.start_position().column;
                let y = child.end_position().column;
                if h as u32 != *preline {
                    *prestart = 0;
                }
                res.push(SemanticToken {
                    delta_line: h as u32 - *preline,
                    delta_start: x as u32 - *prestart,
                    length: (y - x) as u32,
                    token_type: get_token_position(SemanticTokenType::KEYWORD),
                    token_modifiers_bitset: 0,
                });
                *preline = h as u32;
                *prestart = x as u32;
                res.append(&mut sub_tokens(child, source, preline, prestart, false));
            }
            CMakeNodeKinds::BODY
            | CMakeNodeKinds::MACRO_DEF
            | CMakeNodeKinds::FUNCTION_DEF
            | CMakeNodeKinds::IF_CONDITION
            | CMakeNodeKinds::IF_COMMAND
            | CMakeNodeKinds::ELSEIF_COMMAND
            | CMakeNodeKinds::FUNCTION_COMMAND
            | CMakeNodeKinds::MACRO_COMMAND
            | CMakeNodeKinds::FOREACH_LOOP
            | CMakeNodeKinds::FOREACH_COMMAND
            | CMakeNodeKinds::VARIABLE_REF
            | CMakeNodeKinds::NORMAL_VAR
            | CMakeNodeKinds::QUOTED_ELEMENT => {
                res.append(&mut sub_tokens(
                    child,
                    source,
                    preline,
                    prestart,
                    child.kind() == CMakeNodeKinds::IF_COMMAND,
                ));
            }
            _ => {}
        }
    }

    res
}

#[test]
fn test_number() {
    assert!(NUMBERREGEX.is_match("1.1"));
    assert!(NUMBERREGEX.is_match("222"));
}

#[test]
fn test_hl() {
    fn semantic_token_test(context: &str) -> Option<SemanticTokensResult> {
        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let thetree = parse.parse(context, None);
        let tree = thetree?;
        Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: sub_tokens(
                tree.root_node(),
                &context.lines().collect(),
                &mut 0,
                &mut 0,
                false,
            ),
        }))
    }
    semantic_token_test(include_str!("../assert/highlight/bracket_argument.cmake"));
}
