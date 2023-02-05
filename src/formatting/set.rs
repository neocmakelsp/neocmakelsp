pub fn format_set(input: tree_sitter::Node, source: &str, spacelen: u32, usespace: bool) -> String {
    let unit = super::get_space(spacelen, usespace);
    let count = input.child_count();
    let mut keytype = KeyType::Start;
    let newsource: Vec<&str> = source.lines().collect();
    let mut cursor = input.walk();
    let mut units: Vec<String> = vec![];
    let mut mutiline = false;
    for child in input.children(&mut cursor) {
        let starty = child.start_position().row;
        let endy = child.end_position().row;
        if starty != endy {
            mutiline = true;
            let format_result = super::node_to_string(child, source);
            units.push(format_result);
        } else {
            let startx = child.start_position().column;
            let endx = child.end_position().column;
            let new_text = &newsource[starty][startx..endx];
            units.push(new_text.to_string());
        }
    }
    if count < 7 && !mutiline {
        let mut output = String::new();
        for new_text in units {
            let current_keytype = KeyType::match_it(&new_text);
            match (current_keytype, keytype) {
                (KeyType::Var, KeyType::Start) => {
                    output.push_str(&new_text);
                    keytype = KeyType::Var;
                }
                (_, KeyType::Start) => {
                    output.push_str(&new_text);
                }
                (KeyType::RightBracket, _) => {
                    output.push(')');
                }
                (_, KeyType::Var) => {
                    output.push_str(&format!(" {new_text}"));
                }
                (_, _) => {}
            }
        }
        output
    } else {
        let mut output = String::new();
        for new_text in units {
            let current_keytype = KeyType::match_it(&new_text);
            match (current_keytype, keytype) {
                (KeyType::Var, KeyType::Start) => {
                    output.push_str(&new_text);
                    keytype = KeyType::Var;
                }
                (KeyType::Var, KeyType::Keywords) => {
                    output.push_str(&format!(" {new_text}"));
                    keytype = KeyType::Var;
                }
                (_, KeyType::Start) => {
                    output.push_str(&new_text);
                }
                (KeyType::RightBracket, _) => {
                    output.push_str("\n)");
                }
                (KeyType::Keywords, _) => {
                    output.push_str(&format!("\n{unit}{new_text}"));
                    keytype = KeyType::Keywords;
                }
                (_, KeyType::Var) => {
                    if new_text.lines().count() == 1 {
                        output.push_str(&format!("\n{unit}{new_text}"));
                    } else {
                        output.push_str(&format!("\n{new_text}"));
                    }
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
