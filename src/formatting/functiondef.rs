pub fn format_functiondef(
    input: tree_sitter::Node,
    source: &str,
    spacelen: u32,
    usespace: bool,
) -> String {
    let space = super::get_space(spacelen, usespace);
    let newsource: Vec<&str> = source.lines().collect();
    let mut output = String::new();
    let mut cursor = input.walk();
    let mut not_format = false;
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
                let mut is_mark_not_format = false;
                let node_format = if not_format {
                    is_mark_not_format = false;
                    super::get_origin_source(child, source)
                } else {
                    if super::is_notformat_mark(child, source) {
                        is_mark_not_format = true;
                    }
                    super::get_format_from_node(child, source, 2, false)
                };
                if not_format {
                    output.push('\n');
                    output.push_str(&node_format);
                } else {
                    let node_format: Vec<&str> = node_format.lines().collect();
                    for unit in node_format {
                        output.push_str(&format!("\n{space}{unit}"));
                    }
                }
                not_format = is_mark_not_format;
            }
        }
    }
    output
}
