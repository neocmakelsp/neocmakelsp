pub fn format_project(input: tree_sitter::Node, source: &str) -> String {
    let mut output = String::new();
    let newsource: Vec<&str> = source.lines().collect();
    let mut keytype = KeyType::Start;
    let mut cursor = input.walk();
    let nodecount = input.child_count();
    for child in input.children(&mut cursor) {
        let childy = child.start_position().row;
        let startx = child.start_position().column;
        let endx = child.end_position().column;
        let new_text = &newsource[childy][startx..endx];
        let current_keytype = KeyType::match_it(new_text);
        match (current_keytype, keytype) {
            (KeyType::KeyWords, _) => {
                output.push_str(&format!("\n  {}", new_text));
                keytype = current_keytype;
            }
            (KeyType::RightBracket, _) => {
                if nodecount > 4 {
                    output.push_str("\n)");
                } else {
                    output.push(')');
                }
            }
            (_, KeyType::Start) => {
                output.push_str(new_text);
            }

            (_, KeyType::KeyWords) => {
                output.push_str(&format!(" {}", new_text));
            }
            (_, _) => {}
        }
    }
    output
}
#[derive(Clone, Copy)]
enum KeyType {
    Start,
    Nothing,
    KeyWords,
    LeftBracket,
    RightBracket,
}
impl KeyType {
    fn match_it(input: &str) -> Self {
        match input {
            "project" => Self::Start,
            "VERSION" | "DESCRIPTION" | "HOMEPAGE_URL" | "LANGUAGES" => Self::KeyWords,
            "(" => KeyType::LeftBracket,
            ")" => KeyType::RightBracket,
            _ => Self::Nothing,
        }
    }
}
