pub fn format_set(input: tree_sitter::Node, source: &str) -> String {
    let count = input.child_count();
    let mut keytype = KeyType::Start;
    let newsource: Vec<&str> = source.lines().collect();
    if count < 7 {
        let mut output = String::new();
        let mut cursor = input.walk();
        for child in input.children(&mut cursor) {
            let childy = child.start_position().row;
            let startx = child.start_position().column;
            let endx = child.end_position().column;
            let new_text = &newsource[childy][startx..endx];
            let current_keytype = KeyType::match_it(new_text);
            match (current_keytype, keytype) {
                (KeyType::Var, KeyType::Start) => {
                    output.push_str(new_text);
                    keytype = KeyType::Var;
                }
                (_, KeyType::Start) => {
                    output.push_str(new_text);
                }
                (KeyType::RightBracket, _) => {
                    output.push(')');
                }
                (_, KeyType::Var) => {
                    output.push_str(&format!(" {}", new_text));
                }
                (_, _) => {}
            }
        }
        output
    } else {
        let mut output = String::new();
        let mut cursor = input.walk();
        for child in input.children(&mut cursor) {
            let childy = child.start_position().row;
            let startx = child.start_position().column;
            let endx = child.end_position().column;
            let new_text = &newsource[childy][startx..endx];
            let current_keytype = KeyType::match_it(new_text);
            match (current_keytype, keytype) {
                (KeyType::Var, KeyType::Start) => {
                    output.push_str(new_text);
                    keytype = KeyType::Var;
                }
                (KeyType::Var, KeyType::Keywords) => {
                    output.push_str(&format!(" {}", new_text));
                    keytype = KeyType::Var;
                }
                (_, KeyType::Start) => {
                    output.push_str(new_text);
                }
                (KeyType::RightBracket, _) => {
                    output.push_str("\n)");
                }
                (KeyType::Keywords, _) => {
                    output.push_str(&format!("\n  {}", new_text));
                    keytype = KeyType::Keywords;
                }
                (_, KeyType::Var) => {
                    output.push_str(&format!("\n  {}", new_text));
                }
                (_, _) => {}
            }
        }
        output
    }
}
#[derive(Clone, Copy)]
enum KeyType {
    Start,
    Var,
    Keywords,
    LeftBracket,
    RightBracket,
}

impl KeyType {
    fn match_it(input: &str) -> Self {
        match input {
            "set" | "SET" => Self::Start,
            "CACHE" | "BOOL" | "FILEPATH" | "STRING" | "INTERNAL" | "FORCE " => Self::Keywords,
            "(" => KeyType::LeftBracket,
            ")" => KeyType::RightBracket,
            _ => Self::Var,
        }
    }
}
