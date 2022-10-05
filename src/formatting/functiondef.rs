pub fn format_functiondef(input: tree_sitter::Node, source: &str) -> String {
    let newsource: Vec<&str> = source.lines().collect();
    let mut output = String::new();
    let mut cursor = input.walk();
    for child in input.children(&mut cursor) {
        match child.kind() {
            "function_command" => {
                let childy = child.start_position().row;
                let startx = child.start_position().column;
                let endx = child.end_position().column;
                let new_text = &newsource[childy][startx..endx];
                output.push_str(new_text);
            }
            "endfunction_command" => {
                let childy = child.start_position().row;
                let startx = child.start_position().column;
                let endx = child.end_position().column;
                let new_text = &newsource[childy][startx..endx];
                output.push('\n');
                output.push_str(new_text);
            }
            _ => {
                let node_format = super::get_format_from_node(child, source);
                let node_format: Vec<&str> = node_format.lines().collect();
                for unit in node_format {
                    output.push_str(&format!("\n  {}", unit));
                }
            }
        }
    }
    output
}
