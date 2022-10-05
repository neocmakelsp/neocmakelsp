pub fn format_othercommand(input: tree_sitter::Node, source: &str) -> String {
    let mut localline = input.start_position().row;
    let newsource: Vec<&str> = source.lines().collect();
    let mut output = String::new();
    let mut cursor = input.walk();
    let nodecount = input.child_count();
    for child in input.children(&mut cursor) {
        let starty = child.start_position().row;
        let endy = child.end_position().row;
        let startx = child.start_position().column;
        let endx = child.end_position().column;
        let new_text = if starty == endy {
            newsource[starty][startx..endx].to_string()
        } else {
            super::node_to_string(child, source)
        };
        if child.kind() == "identifier" {
            output.push_str(&new_text);
        } else if new_text == "(" {
            output.push('(');
        } else if new_text == ")" {
            if nodecount > 3 {
                output.pop();
            }
            output.push_str(")");
        } else if starty > localline {
            localline = starty;
            output.push_str(&format!("\n  {} ", new_text));
        } else {
            output.push_str(&format!("{} ", new_text));
        }
    }
    output
}
