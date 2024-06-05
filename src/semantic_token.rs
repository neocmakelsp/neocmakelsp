use tower_lsp::{
    lsp_types::{SemanticToken, SemanticTokenType, SemanticTokens, SemanticTokensResult},
    Client,
};
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
    parse.set_language(&tree_sitter_cmake::language()).unwrap();
    let thetree = parse.parse(context, None);
    let tree = thetree?;
    Some(SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data: sub_tokens(tree.root_node(), context, &mut 0),
    }))
}

fn sub_tokens(input: tree_sitter::Node, source: &str, preline: &mut u32) -> Vec<SemanticToken> {
    let mut res = vec![];

    let mut course = input.walk();

    for child in input.children(&mut course) {
        match child.kind() {
            "normal_command" => {
                let Some(id) = child.child(0) else {
                    continue;
                };

                let h = id.start_position().row;
                let x = id.start_position().column;
                let y = id.end_position().column;
                res.push(SemanticToken {
                    delta_line: h as u32 - *preline,
                    delta_start: x as u32,
                    length: (y - x) as u32,
                    token_type: get_token_position(SemanticTokenType::METHOD),
                    token_modifiers_bitset: 0,
                });
                *preline = h as u32;
            }

            "endmacro_command" | "endif_command" | "endfunction_command" | "else_command" => {
                let Some(id) = child.child(0) else {
                    continue;
                };
                let h = id.start_position().row;
                let x = id.start_position().column;
                let y = id.end_position().column;
                res.push(SemanticToken {
                    delta_line: h as u32 - *preline,
                    delta_start: x as u32,
                    length: (y - x) as u32,
                    token_type: get_token_position(SemanticTokenType::KEYWORD),
                    token_modifiers_bitset: 0,
                });
                *preline = h as u32;
            }
            "function" | "macro" | "if" => {
                let h = child.start_position().row;
                let x = child.start_position().column;
                let y = child.end_position().column;
                res.push(SemanticToken {
                    delta_line: h as u32 - *preline,
                    delta_start: x as u32,
                    length: (y - x) as u32,
                    token_type: get_token_position(SemanticTokenType::KEYWORD),
                    token_modifiers_bitset: 0,
                });
                *preline = h as u32;
                res.append(&mut sub_tokens(child, source, preline));
            }
            "body" | "macro_def" | "function_def" | "if_condition" | "if_command" | "function_command" | "macro_command"=> {
                res.append(&mut sub_tokens(child, source, preline));
            }
            _ => {}
        }
    }

    res
}
