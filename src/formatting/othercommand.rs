pub fn format_othercommand(
    input: tree_sitter::Node,
    source: &str,
    spacelen: u32,
    usespace: bool,
) -> String {
    let unit = super::get_space(spacelen, usespace);
    let mut localline = input.start_position().row;
    let newsource: Vec<&str> = source.lines().collect();
    let mut output = String::new();
    let mut cursor = input.walk();
    let nodecount = input.child_count();
    let mut beforeisleftblank = false;
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
            beforeisleftblank = true;
            output.push('(');
        } else if new_text == ")" {
            if nodecount > 3 {
                output.pop();
            }
            if starty > localline {
                output.push('\n');
            }
            output.push(')');
        } else if starty > localline {
            if !beforeisleftblank {
                output.pop();
            }
            beforeisleftblank = false;
            localline = starty;
            output.push_str(&format!("\n{unit}{new_text} "));
        } else {
            beforeisleftblank = false;
            output.push_str(&format!("{new_text} "));
        }
    }
    output
}

#[test]
fn tst_format_base() {
    let source = include_str!("../../assert/base/formatbefore.cmake");
    let sourceafter = include_str!("../../assert/base/formatafter.cmake");
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(tree_sitter_cmake::language()).unwrap();
    let tree = parse.parse(source, None).unwrap();
    let mut formatstr = super::get_format_cli(tree.root_node(), source, 1, false).unwrap();
    formatstr.push('\n');
    assert_eq!(formatstr.as_str(), sourceafter);
}
