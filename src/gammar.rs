use std::fs;
/// checkerror the gammer error
/// if there is error , it will return the position of the error
use std::path::{Path, PathBuf};
pub fn checkerror(
    local_path: &Path,
    source: &str,
    input: tree_sitter::Node,
) -> Option<Vec<(tree_sitter::Point, tree_sitter::Point, String)>> {
    let newsource: Vec<&str> = source.lines().collect();
    if input.is_error() {
        Some(vec![(input.start_position(), input.end_position(), "Gammer Error".to_string())])
    } else {
        let mut course = input.walk();
        {
            let mut output = vec![];
            for node in input.children(&mut course) {
                if let Some(mut tran) = checkerror(local_path, source, node) {
                    output.append(&mut tran);
                }
                if node.kind() == "normal_command" {
                    let h = node.start_position().row;
                    let ids = node.child(0).unwrap();
                    //let ids = ids.child(2).unwrap();
                    let x = ids.start_position().column;
                    let y = ids.end_position().column;
                    let name = &newsource[h][x..y];
                    if name == "include" {
                        if node.child_count() >= 4 {
                            let ids = node.child(2).unwrap();
                            if ids.start_position().row == ids.end_position().row {
                                let h = ids.start_position().row;
                                let x = ids.start_position().column;
                                let y = ids.end_position().column;
                                let name = &newsource[h][x..y];
                                if name.split('.').count() != 1 {
                                    let subpath = local_path.parent().unwrap().join(name);
                                    match cmake_try_exists(&subpath) {
                                        Ok(true) => {
                                            if scanner_include_error(&subpath) {
                                                output.push((
                                                    node.start_position(),
                                                    node.end_position(),
                                                    "Contain Error in include file".to_string()
                                                ));
                                            }
                                        }
                                        _ => {
                                            output
                                                .push((node.start_position(), node.end_position(),"include file is not exist or cannot access".to_string()));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if output.is_empty() {
                None
            } else {
                Some(output)
            }
        }
    }
}

fn cmake_try_exists(input: &PathBuf) -> std::io::Result<bool> {
    match std::fs::metadata(input) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}
fn scanner_include_error(path: &PathBuf) -> bool {
    match fs::read_to_string(path) {
        Ok(content) => {
            let mut parse = tree_sitter::Parser::new();
            parse.set_language(tree_sitter_cmake::language()).unwrap();
            let thetree = parse.parse(content.clone(), None);
            let tree = thetree.unwrap();
            tree.root_node().has_error()
        }
        Err(_) => true,
    }
}
