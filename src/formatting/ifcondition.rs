pub fn format_ifcondition(
    input: tree_sitter::Node,
    source: &str,
    spacelen: u32,
    usespace: bool,
) -> String {
    let newsource: Vec<&str> = source.lines().collect();
    let space = super::get_space(spacelen, usespace);
    let mut output = String::new();
    let mut cursor = input.walk();
    for child in input.children(&mut cursor) {
        match child.kind() {
            "if_command" => {
                let childy = child.start_position().row;
                let startx = child.start_position().column;
                let endx = child.end_position().column;
                let new_text = &newsource[childy][startx..endx];
                output.push_str(new_text);
            }
            "endif_command" | "else_command" | "elseif_command" => {
                let childy = child.start_position().row;
                let startx = child.start_position().column;
                let endx = child.end_position().column;
                let new_text = &newsource[childy][startx..endx];
                output.push('\n');
                output.push_str(new_text);
            }
            _ => {
                let node_format = super::get_format_from_node(child, source, spacelen, usespace);
                let node_format: Vec<&str> = node_format.lines().collect();
                for unit in node_format {
                    output.push_str(&format!("\n{space}{unit}"));
                }
            }
        }
    }
    output
}
