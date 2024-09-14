use std::path::{Path, PathBuf};

use crate::Url;
use tower_lsp::lsp_types::DocumentLink;
use tower_lsp::lsp_types::Position;
use tower_lsp::lsp_types::Range;

use crate::consts::TREESITTER_CMAKE_LANGUAGE;
use crate::utils::{gen_module_pattern, remove_quotation_and_replace_placeholders};
use crate::CMakeNodeKinds;

const LINK_NODE_KIND: &[&str] = &["include", "add_subdirectory"];

pub fn document_link_search<P: AsRef<Path>>(
    source: &str,
    current_file: P,
) -> Option<Vec<DocumentLink>> {
    let mut links = vec![];
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let thetree = parse.parse(source, None)?;
    let documents: Vec<&str> = source.lines().collect();
    let file_parent = current_file.as_ref().parent()?;
    document_link_search_inner(&documents, thetree.root_node(), &mut links, &file_parent);
    if links.is_empty() {
        return None;
    }
    Some(links)
}

pub fn document_link_search_inner<P: AsRef<Path>>(
    source: &Vec<&str>,
    node: tree_sitter::Node,
    links: &mut Vec<DocumentLink>,
    current_parent: &P,
) {
    let mut course = node.walk();
    for child in node.children(&mut course) {
        match child.kind() {
            CMakeNodeKinds::IF_CONDITION | CMakeNodeKinds::FOREACH_LOOP | CMakeNodeKinds::BODY => {
                document_link_search_inner(source, child, links, current_parent)
            }
            CMakeNodeKinds::NORMAL_COMMAND => {
                let h = child.start_position().row;
                let cmd_id = child.child(0).unwrap();
                let x = cmd_id.start_position().column;
                let y = cmd_id.end_position().column;
                let name = source[h][x..y].to_lowercase();
                if !LINK_NODE_KIND.contains(&name.as_str()) || child.child_count() < 4 {
                    continue;
                }
                let ids = child.child(2).unwrap();
                let is_subdirectory = name == "add_subdirectory";
                let h = ids.start_position().row;
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let filename = &source[h][x..y];
                let Some(filename) = remove_quotation_and_replace_placeholders(filename) else {
                    continue;
                };
                let (final_uri, buildin) = if is_subdirectory {
                    (
                        current_parent
                            .as_ref()
                            .join(filename)
                            .join("CMakeLists.txt"),
                        false,
                    )
                } else {
                    let Some((cmake_path, buildin)) =
                        convert_include_cmake(&filename, current_parent)
                    else {
                        continue;
                    };
                    (cmake_path, buildin)
                };
                if !final_uri.exists() {
                    continue;
                }
                let tooltip = if buildin {
                    Some(format!("buildin module, link: {}", final_uri.display()))
                } else {
                    Some(format!("link: {}", final_uri.display()))
                };
                let range = Range {
                    start: Position {
                        line: h as u32,
                        character: x as u32,
                    },
                    end: Position {
                        line: h as u32,
                        character: y as u32,
                    },
                };
                links.push(DocumentLink {
                    range,
                    target: Some(Url::from_file_path(final_uri).unwrap()),
                    tooltip,
                    data: None,
                })
            }
            _ => {}
        }
    }
}

fn convert_include_cmake<P: AsRef<Path>>(name: &str, current_parent: P) -> Option<(PathBuf, bool)> {
    if name.ends_with(".cmake") {
        return Some((current_parent.as_ref().join(name), false));
    }
    let global_pattern = gen_module_pattern(name)?;
    Some((
        glob::glob(&global_pattern)
            .into_iter()
            .flatten()
            .flatten()
            .next()?,
        true,
    ))
}

#[test]
fn tst_document_link_search() {
    use crate::fileapi::cache::Cache;
    use crate::fileapi::CACHE_DATA;
    use std::fs;

    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();

    let json_value = format!(
        "{{
    \"kind\" : \"cache\",
    \"version\" :
    {{
        \"major\" : 2,
        \"minor\" : 0
    }},
    \"entries\" :
    [
        {{
            \"name\" : \"ROOT_DIR\",
            \"properties\" :
            [
            ],
            \"type\" : \"FILEPATH\",
            \"value\" : \"{}\"
        }}
    ]
    }}",
        dir.path().display()
    );
    let template_cache: Cache = serde_json::from_str(&json_value).unwrap();
    CACHE_DATA.set(template_cache).unwrap();
    let jump_file_src = r#"
set(ABCD 1234)
message(INFO "${ABCD}")
set(ROOT_DIR "ABCD" STRING CACHE "ROOTDIR")
include("${ROOT_DIR}/hello.cmake")
add_subdirectory(abcd_test)
"#;

    let top_cmake = dir.path().join("CMakeLists.txt");
    let hello_cmake = dir.path().join("hello.cmake");
    File::create_new(&hello_cmake).unwrap();
    let mut top_file = File::create_new(&top_cmake).unwrap();
    top_file.write(jump_file_src.as_bytes()).unwrap();
    let subdir = dir.path().join("abcd_test");
    fs::create_dir_all(&subdir).unwrap();
    let subdir_file = subdir.join("CMakeLists.txt");
    File::create_new(&subdir_file).unwrap();
    let mut links = vec![];
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let thetree = parse.parse(jump_file_src, None).unwrap();
    let documents: Vec<&str> = jump_file_src.lines().collect();
    document_link_search_inner(&documents, thetree.root_node(), &mut links, &dir.path());

    assert_eq!(
        links,
        vec![
            DocumentLink {
                range: Range {
                    start: Position {
                        line: 4,
                        character: 8
                    },
                    end: Position {
                        line: 4,
                        character: 33
                    }
                },
                target: Some(Url::from_file_path(&hello_cmake).unwrap()),
                tooltip: Some(format!("link: {}", hello_cmake.display())),
                data: None
            },
            DocumentLink {
                range: Range {
                    start: Position {
                        line: 5,
                        character: 17
                    },
                    end: Position {
                        line: 5,
                        character: 26
                    }
                },
                target: Some(Url::from_file_path(&subdir_file).unwrap()),
                tooltip: Some(format!("link: {}", subdir_file.display())),
                data: None
            },
        ]
    )
}
