pub fn format_definition(input: tree_sitter::Node, source: &str) -> String {
    let newsource: Vec<&str> = source.lines().collect();
    let mut output = String::new();
    let mut cursor = input.walk();
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
            // NOTE: pop the more " "
            output.pop();
            output.push(')');
        } else {
            output.push_str(&new_text);
            if !new_text.ends_with('=') {
                output.push(' ');
            }
        }
    }
    output
}
