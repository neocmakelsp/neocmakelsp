pub fn format_project(input: tree_sitter::Node, source: &str) -> String {
    let mut output = String::new();
    let newsource: Vec<&str> = source.lines().collect();
    let mut cursor = input.walk();
    for child in input.children(&mut cursor) {
        let childy = child.start_position().row;
        let startx = child.start_position().column;
        let endx = child.end_position().column;
        let new_text = newsource[childy][startx..endx].to_string();
        output.push_str(&format!("{} ", new_text));
    }
    output
}
