pub fn format_othercommand(input: tree_sitter::Node, source: &str) -> String {
    let mut localline = input.start_position().row;
    let newsource: Vec<&str> = source.lines().collect();
    let mut output = String::new();
    let mut cursor = input.walk();
    for child in input.children(&mut cursor) {
        let childy = child.start_position().row;
        let startx = child.start_position().column;
        let endx = child.end_position().column;
        let new_text = &newsource[childy][startx..endx];
        if child.kind() == "identifier" {
            output.push_str(new_text);
        } else if new_text == "(" {
            output.push('(');
        } else if new_text == ")" {
            output.push_str(" )");
        } else if childy > localline {
            localline = childy;
            output.push_str(&format!("\n  {}", new_text));
        } else {
            output.push_str(&format!(" {}", new_text));
        }
    }
    output
}
