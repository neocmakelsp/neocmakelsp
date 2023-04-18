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
    let mut not_format = false;
    let mut start_line = input.start_position().row;
    for child in input.children(&mut cursor) {
        // NOTE: re add the origin empty lines
        let child_start_line = child.start_position().row;
        let child_end_line = child.end_position().row;
        if child_start_line - start_line > 1 {
            for _ in start_line..child_start_line - 1 {
                output.push('\n');
            }
        }
        start_line = child_end_line;

        match child.kind() {
            "if_command" => {
                let mut ifcommandtext = String::new();
                let mut ifcursor = child.walk();
                let childverybegin = child.start_position().row;
                let mut childstarty = child.start_position().row;
                let mut childstartx = child.start_position().column;
                let mut childendx = child.start_position().column;
                macro_rules! formatifcommand {
                    () => {
                        let new_text = &newsource[childstarty][childstartx..childendx];
                        if childstarty != childverybegin {
                            ifcommandtext.push('\n');
                            ifcommandtext.push_str(&space);
                        }
                        ifcommandtext.push_str(&new_text);
                    };
                }
                for ifchild in child.children(&mut ifcursor) {
                    if ifchild.start_position().row != childstarty {
                        formatifcommand!();
                        childstarty = ifchild.start_position().row;
                        childstartx = ifchild.start_position().column;
                        childendx = ifchild.end_position().column;
                    } else {
                        childendx = ifchild.end_position().column;
                    }
                }
                formatifcommand!();
                output.push_str(&ifcommandtext);
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
                let mut is_mark_not_format = false;
                let node_format = if not_format {
                    is_mark_not_format = false;
                    super::get_origin_source(child, source)
                } else {
                    if super::is_notformat_mark(child, source) {
                        is_mark_not_format = true;
                    }
                    super::get_format_from_node(child, source, spacelen, usespace)
                };
                if not_format {
                    output.push('\n');
                    output.push_str(&node_format);
                } else {
                    let node_format: Vec<&str> = node_format.lines().collect();
                    for unit in node_format {
                        if unit.is_empty() {
                            output.push('\n')
                        } else {
                            output.push_str(&format!("\n{space}{unit}"));
                        }
                    }
                }
                not_format = is_mark_not_format;
            }
        }
    }
    output
}
