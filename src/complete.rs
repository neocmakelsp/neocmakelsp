mod builtin;
mod findpackage;
mod includescanner;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use builtin::{BUILTIN_COMMAND, BUILTIN_MODULE, BUILTIN_VARIABLE};
use dashmap::DashMap;
use tokio::sync::Mutex;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionResponse, Documentation, MessageType, Position,
    Uri,
};
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

use crate::consts::TREESITTER_CMAKE_LANGUAGE;
use crate::fileapi;
use crate::languageserver::get_or_update_buffer_contents;
use crate::scansubs::TREE_MAP;
use crate::utils::treehelper::{PositionType, ToPoint, get_pos_type};
use crate::utils::{
    CACHE_CMAKE_PACKAGES_WITHKEYS, gen_module_pattern, include_is_module,
    remove_quotation_and_replace_placeholders,
};

pub type CompleteKV = HashMap<PathBuf, Vec<CompletionItem>>;

/// NOTE: collect the all completeitems in this PathBuf
/// Include the top CMakeList.txt
pub static COMPLETE_CACHE: LazyLock<Arc<Mutex<CompleteKV>>> =
    LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

#[cfg(unix)]
const PKG_IMPORT_TARGET: &str = "IMPORTED_TARGET";

pub fn init_builtin_command() {
    let _ = &*BUILTIN_COMMAND;
}
pub fn init_builtin_module() {
    let _ = &*BUILTIN_MODULE;
}

pub fn init_builtin_variable() {
    let _ = &*BUILTIN_VARIABLE;
}

pub fn init_system_modules() {
    let _ = &*crate::utils::CMAKE_PACKAGES_WITHKEY;
    let _ = &*crate::utils::CMAKE_PACKAGES;
    #[cfg(unix)]
    {
        let _ = &*crate::utils::packagepkgconfig::PKG_CONFIG_PACKAGES;
        let _ = &*crate::utils::packagepkgconfig::PKG_CONFIG_PACKAGES_WITHKEY;
    }
}

pub fn rst_doc_read(doc: &str, filename: &str) -> Vec<CompletionItem> {
    doc.lines()
        .filter(|line| line.starts_with(".. command:: "))
        .map(|line| &line[13..])
        .map(|line| CompletionItem {
            label: line.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some("Command".to_string()),
            documentation: Some(Documentation::String(format!(
                "defined command from {filename}\n{doc}"
            ))),
            ..Default::default()
        })
        .collect()
}

pub async fn update_cache<P: AsRef<Path>>(path: P, context: &str) -> Vec<CompletionItem> {
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let thetree = parse.parse(context, None);
    let tree = thetree.unwrap();
    let Some(result_data) = getsubcomplete(
        tree.root_node(),
        context,
        path.as_ref(),
        PositionType::VarOrFun,
        None,
        &mut Vec::new(),
        &mut Vec::new(),
        true,
        true,
    ) else {
        return Vec::new();
    };
    let mut cache = COMPLETE_CACHE.lock().await;
    cache.insert(path.as_ref().to_path_buf(), result_data.clone());
    result_data
}

pub async fn get_cached_completion<P: AsRef<Path>>(
    path: P,
    documents: &DashMap<Uri, String>,
) -> Vec<CompletionItem> {
    let mut path = path.as_ref().to_path_buf();
    let mut completions = Vec::new();

    let tree_map = TREE_MAP.lock().await;

    while let Some(parent) = tree_map.get(&path) {
        let complete_cache = COMPLETE_CACHE.lock().await;
        if let Some(data) = complete_cache.get(parent) {
            completions.append(&mut data.clone());
        } else if let Ok(context) = get_or_update_buffer_contents(parent, documents).await {
            drop(complete_cache);
            completions.append(&mut update_cache(parent, context.as_str()).await);
            path.clone_from(parent);
            continue;
        }
        path.clone_from(parent);
    }

    completions
}

/// get the complete messages
pub async fn getcomplete<P: AsRef<Path>>(
    source: &str,
    location: Position,
    client: &tower_lsp::Client,
    local_path: P,
    find_cmake_in_package: bool,
    documents: &DashMap<Uri, String>,
) -> Option<CompletionResponse> {
    let local_path = local_path.as_ref();
    let mut parse = tree_sitter::Parser::new();
    parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
    let thetree = parse.parse(source, None);
    let tree = thetree.unwrap();
    let mut complete: Vec<CompletionItem> = vec![];

    let current_point = location.to_point();
    let postype = get_pos_type(current_point, tree.root_node(), source);
    match postype {
        PositionType::VarOrFun
        | PositionType::TargetLink
        | PositionType::TargetInclude
        | PositionType::ArgumentOrList => {
            let mut cached_completion = get_cached_completion(local_path, documents).await;
            if !cached_completion.is_empty() {
                complete.append(&mut cached_completion);
            }
            if let Some(mut cmake_cache) = fileapi::get_complete_data() {
                complete.append(&mut cmake_cache);
            }
            if let Some(mut message) = getsubcomplete(
                tree.root_node(),
                source,
                Path::new(local_path),
                postype,
                Some(location),
                &mut Vec::new(),
                &mut Vec::new(),
                true,
                find_cmake_in_package,
            ) {
                complete.append(&mut message);
            }

            if let Ok(messages) = &*BUILTIN_COMMAND
                && !matches!(postype, PositionType::ArgumentOrList)
            {
                complete.append(&mut messages.clone());
            }
            if let Ok(messages) = &*BUILTIN_VARIABLE {
                complete.append(&mut messages.clone());
            }
        }
        PositionType::FindPackageSpace(space) => {
            complete.append(&mut findpackage::completion_items_with_prefix(space));
        }
        PositionType::FindPackage => {
            complete.append(&mut findpackage::CMAKE_SOURCE.clone());
        }
        #[cfg(unix)]
        PositionType::FindPkgConfig => {
            complete.append(&mut findpackage::PKGCONFIG_SOURCE.clone());
        }
        PositionType::Include => {
            let mut cached_completion = get_cached_completion(local_path, documents).await;
            if !cached_completion.is_empty() {
                complete.append(&mut cached_completion);
            }
            if let Some(mut cmake_cache) = fileapi::get_complete_data() {
                complete.append(&mut cmake_cache);
            }
            if let Ok(messages) = &*BUILTIN_MODULE {
                complete.append(&mut messages.clone());
            }
        }
        PositionType::Comment => {
            client.log_message(MessageType::INFO, "Empty").await;
            return None;
        }
        _ => {}
    }

    if complete.is_empty() {
        client.log_message(MessageType::INFO, "Empty").await;
        None
    } else {
        Some(CompletionResponse::Array(complete))
    }
}

const LINE_COMMENT_QUERY: &str = r#"(
    (line_comment) @comment
)"#;

const BRACKET_COMMENT_QUERY: &str = r#"(
    (bracket_comment) @comment
)"#;

const MACRO_QUERY: &str = r#"(
   (macro_command
       (argument_list) @argument_list
   )
)"#;

const FUNCTION_QUERY: &str = r#"(
   (function_command
       (argument_list) @argument_list
   )
)"#;

const NORMAL_COMMAND_QUERY: &str = r#"
(
    (normal_command
        (identifier) @identifier
        (argument_list) @argument_list
    )
)
"#;

struct LineCommentNode<'a> {
    content: &'a str,
    node: Node<'a>,
}
struct BracketCommentNode<'a> {
    content: &'a str,
}

struct MacroNode<'a> {
    name: &'a str,
    arguments: Vec<Node<'a>>,
}

#[derive(Debug)]
struct FunNode<'a> {
    name: &'a str,
    arguments: Vec<Node<'a>>,
}

struct NormalCommandNode<'a> {
    identifier: &'a str,
    identifier_node: Option<Node<'a>>,
    first_arg: &'a str,
    args: Vec<Node<'a>>,
}

fn get_comments<'a>(source: &'a [u8], node: Node<'a>, max_height: u32) -> Vec<LineCommentNode<'a>> {
    // NOTE: prepare comments
    let mut comments = vec![];
    let query_comment = Query::new(&TREESITTER_CMAKE_LANGUAGE, LINE_COMMENT_QUERY).unwrap();
    let mut cursor_comments = QueryCursor::new();
    let mut matches_comments = cursor_comments.matches(&query_comment, node, source);

    'out: while let Some(m) = matches_comments.next() {
        for e in m.captures {
            let node = e.node;
            if node.start_position().row as u32 > max_height {
                continue 'out;
            }
            let content = node
                .utf8_text(source)
                .unwrap()
                .strip_prefix("#")
                .unwrap()
                .trim();
            comments.push(LineCommentNode { content, node });
        }
    }
    comments
}

fn get_bracket_comments<'a>(
    source: &'a [u8],
    node: Node<'a>,
    max_height: u32,
) -> Vec<BracketCommentNode<'a>> {
    // NOTE: prepare comments
    let mut comments = vec![];
    let query_comment = Query::new(&TREESITTER_CMAKE_LANGUAGE, BRACKET_COMMENT_QUERY).unwrap();
    let mut cursor_comments = QueryCursor::new();
    let mut matches_comments = cursor_comments.matches(&query_comment, node, source);

    'out: while let Some(m) = matches_comments.next() {
        for e in m.captures {
            let node = e.node;
            if node.start_position().row as u32 > max_height {
                continue 'out;
            }
            comments.push(BracketCommentNode {
                content: node.utf8_text(source).unwrap(),
            });
        }
    }
    comments
}
fn get_macros<'a>(source: &'a [u8], node: Node<'a>, max_height: u32) -> Vec<MacroNode<'a>> {
    let mut macros = vec![];
    let query_macro = Query::new(&TREESITTER_CMAKE_LANGUAGE, MACRO_QUERY).unwrap();
    let mut cursor_macro = QueryCursor::new();
    let mut matches_macro = cursor_macro.matches(&query_macro, node, source);

    'out: while let Some(m) = matches_macro.next() {
        let mut macro_node = MacroNode {
            name: "",
            arguments: vec![],
        };
        for e in m.captures {
            let node = e.node;
            if node.start_position().row as u32 > max_height {
                continue 'out;
            }
            let mut walk = node.walk();
            for child in node.children(&mut walk) {
                macro_node.arguments.push(child);
            }
            let Some(first_arg) = node.child(0) else {
                continue 'out;
            };
            macro_node.name = first_arg.utf8_text(source).unwrap();
        }
        macros.push(macro_node);
    }
    macros
}

fn get_normal_commands<'a>(
    source: &'a [u8],
    node: Node<'a>,
    max_height: u32,
) -> Vec<NormalCommandNode<'a>> {
    let mut macros = vec![];
    let query_macro = Query::new(&TREESITTER_CMAKE_LANGUAGE, NORMAL_COMMAND_QUERY).unwrap();
    let mut cursor_macro = QueryCursor::new();
    let mut matches_macro = cursor_macro.matches(&query_macro, node, source);

    'out: while let Some(m) = matches_macro.next() {
        let mut normal_command = NormalCommandNode {
            identifier: "",
            identifier_node: None,
            first_arg: "",
            args: vec![],
        };
        for e in m.captures {
            let node = e.node;
            if node.start_position().row as u32 > max_height {
                continue 'out;
            }
            for command in m.captures {
                let node = command.node;
                if node.kind() == "identifier" {
                    normal_command.identifier = node.utf8_text(source).unwrap();
                    normal_command.identifier_node = Some(node);
                    continue;
                }
                if node.kind() == "argument_list" {
                    let mut walk = node.walk();
                    for child in node.children(&mut walk) {
                        normal_command.args.push(child);
                    }
                    let Some(first_arg) = node.child(0) else {
                        continue 'out;
                    };
                    normal_command.first_arg = first_arg.utf8_text(source).unwrap();
                }
            }
        }
        macros.push(normal_command);
    }
    macros
}

fn get_funs<'a>(source: &'a [u8], node: Node<'a>, max_height: u32) -> Vec<FunNode<'a>> {
    let mut funs = vec![];
    let query_fun = Query::new(&TREESITTER_CMAKE_LANGUAGE, FUNCTION_QUERY).unwrap();
    let mut cursor_fun = QueryCursor::new();
    let mut matches_fun = cursor_fun.matches(&query_fun, node, source);

    'out: while let Some(m) = matches_fun.next() {
        let mut fun_node = FunNode {
            name: "",
            arguments: vec![],
        };
        for e in m.captures {
            let node = e.node;
            if node.start_position().row as u32 > max_height {
                continue 'out;
            }
            let mut walk = node.walk();
            for child in node.children(&mut walk) {
                fun_node.arguments.push(child);
            }
            let Some(first_arg) = node.child(0) else {
                continue 'out;
            };
            fun_node.name = first_arg.utf8_text(source).unwrap();
        }
        funs.push(fun_node);
    }
    funs
}

/// NOTE: postype can only be VarOrFun | TargetLink | TargetInclude | ArgumentOrList
/// get the variable from the loop
/// use position to make only can complete which has show before
#[allow(clippy::too_many_arguments)]
fn getsubcomplete<P: AsRef<Path>>(
    input: tree_sitter::Node,
    source: &str,
    local_path: P,
    postype: PositionType,
    location: Option<Position>,
    include_files: &mut Vec<PathBuf>,
    complete_packages: &mut Vec<String>,
    should_in: bool, // if is searched to findpackage, it should not in
    find_cmake_in_package: bool,
) -> Option<Vec<CompletionItem>> {
    assert!(matches!(
        postype,
        PositionType::VarOrFun
            | PositionType::TargetLink
            | PositionType::TargetInclude
            | PositionType::ArgumentOrList
    ));
    let local_path = local_path.as_ref();
    if let Some(location) = location
        && input.start_position().row as u32 > location.line
    {
        return None;
    }
    let max_height: u32 = location.map(|l| l.line).unwrap_or(u32::MAX);
    let source_bytes = source.as_bytes();
    let mut complete: Vec<CompletionItem> = vec![];

    // NOTE: prepare
    let bracket_comments = get_bracket_comments(source_bytes, input, max_height);
    let comments = get_comments(source_bytes, input, max_height);

    let macros = get_macros(source_bytes, input, max_height);
    let functions = get_funs(source_bytes, input, max_height);
    let normal_commands = get_normal_commands(source_bytes, input, max_height);
    // NOTE: check bracket_comments
    for bracket_comment in bracket_comments {
        complete.append(&mut rst_doc_read(
            &bracket_comment.content.to_string(),
            local_path.file_name().unwrap().to_str().unwrap(),
        ));
    }

    // NOTE: check functions
    for fun in functions {
        let name = fun.name;
        let row = fun.arguments[0].start_position().row;

        let mut document_info = format!("defined function\nfrom: {}", local_path.display());
        if let Some(line_comment) = comments
            .iter()
            .find(|c| c.node.start_position().row + 1 == row)
            .map(|c| c.content)
        {
            document_info = format!("{}\n\n{}", document_info, line_comment);
        }
        complete.push(CompletionItem {
            label: name.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some("Function".to_string()),
            documentation: Some(Documentation::String(document_info)),
            ..Default::default()
        });
    }

    // NOTE: check macros
    for macro_node in macros {
        let name = macro_node.name;
        let row = macro_node.arguments[0].start_position().row;

        let mut document_info = format!("defined macro\nfrom: {}", local_path.display());
        if let Some(line_comment) = comments
            .iter()
            .find(|c| c.node.start_position().row - 1 == row)
            .map(|c| c.content)
        {
            document_info = format!("{}\n\n{}", document_info, line_comment);
        }
        complete.push(CompletionItem {
            label: name.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some("Function".to_string()),
            documentation: Some(Documentation::String(document_info)),
            ..Default::default()
        });
    }

    // NOTE: check normal_commands
    for command in normal_commands {
        let name = command.identifier.to_lowercase();
        if name == "include" {
            let Some(file_name) = remove_quotation_and_replace_placeholders(command.first_arg)
            else {
                continue;
            };
            let (is_builtin, subpath) = {
                if !include_is_module(&file_name) {
                    (false, local_path.parent().unwrap().join(file_name))
                } else {
                    let Some(glob_pattern) = gen_module_pattern(&file_name) else {
                        continue;
                    };
                    let Some(path) = glob::glob(&glob_pattern)
                        .into_iter()
                        .flatten()
                        .flatten()
                        .next()
                    else {
                        continue;
                    };
                    (true, path)
                }
            };
            if include_files.contains(&subpath) {
                continue;
            }
            if let Ok(true) = subpath.try_exists() {
                if let Some(mut comps) = includescanner::scanner_include_complete(
                    &subpath,
                    postype,
                    include_files,
                    complete_packages,
                    find_cmake_in_package,
                    is_builtin,
                ) {
                    complete.append(&mut comps);
                }
                include_files.push(subpath);
            }
        } else if name == "mark_as_advanced" {
            for arg in command.args {
                let variable = arg.utf8_text(source_bytes).unwrap();
                complete.push(CompletionItem {
                    label: variable.to_string(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: Some("Variable".to_string()),
                    documentation: Some(Documentation::String(format!(
                        "defined var\nfrom: {}",
                        local_path.display()
                    ))),
                    ..Default::default()
                });
            }
        } else {
            if name == "set" || name == "options" {
                let name = command.first_arg;
                let row = command.identifier_node.unwrap().start_position().row;
                let mut document_info = format!("defined variable\nfrom: {}", local_path.display());

                if let Some(line_comment) = comments
                    .iter()
                    .find(|c| c.node.start_position().row + 1 == row)
                    .map(|c| c.content)
                {
                    document_info = format!("{}\n\n{}", document_info, line_comment);
                }
                complete.push(CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::VALUE),
                    detail: Some("Value".to_string()),
                    documentation: Some(Documentation::String(document_info)),
                    ..Default::default()
                });
            }
            if name == "find_package" && should_in {
                let package_name = command.first_arg;
                let argument_count = command.args.len();
                let mut component_part = Vec::new();
                let mut cmakepackages = Vec::new();

                let components_packages = {
                    if argument_count >= 2 {
                        let mut support_component = false;
                        let mut components_packages = Vec::new();
                        for index in 1..argument_count {
                            let package_prefix_node = command.args[index];
                            let component = package_prefix_node.utf8_text(source_bytes).unwrap();
                            if component == "COMPONENTS" {
                                support_component = true;
                            } else if component != "REQUIRED" {
                                component_part.push(component.to_string());
                                components_packages.push(format!("{package_name}::{component}"));
                            }
                        }
                        if support_component {
                            Some(components_packages)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };
                if find_cmake_in_package && components_packages.is_some() {
                    for package in component_part {
                        cmakepackages.push(format!("{package_name}{package}"));
                    }
                } else {
                    cmakepackages.push(package_name.to_string());
                }
                // modern cmake like Qt5::Core
                if let Some(components) = components_packages {
                    for component in components {
                        complete.push(CompletionItem {
                            label: component,
                            kind: Some(CompletionItemKind::VARIABLE),
                            detail: Some("Variable".to_string()),
                            documentation: Some(Documentation::String(format!(
                                "package from: {package_name}",
                            ))),
                            ..Default::default()
                        });
                    }
                }

                if matches!(postype, PositionType::TargetLink | PositionType::VarOrFun) {
                    complete.push(CompletionItem {
                        label: format!("{package_name}_LIBRARIES"),
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: Some("Variable".to_string()),
                        documentation: Some(Documentation::String(format!(
                            "package: {package_name}",
                        ))),
                        ..Default::default()
                    });
                }

                if matches!(
                    postype,
                    PositionType::TargetInclude | PositionType::VarOrFun
                ) {
                    complete.push(CompletionItem {
                        label: format!("{package_name}_INCLUDE_DIRS"),
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: Some("Variable".to_string()),
                        documentation: Some(Documentation::String(format!(
                            "package: {package_name}",
                        ))),
                        ..Default::default()
                    });
                }
                for package in cmakepackages {
                    if complete_packages.contains(&package) {
                        continue;
                    }
                    complete_packages.push(package.clone());
                    let Some(mut completeitem) = get_cmake_package_complete(
                        package.as_str(),
                        postype,
                        include_files,
                        complete_packages,
                    ) else {
                        continue;
                    };
                    complete.append(&mut completeitem);
                }
            }

            #[cfg(unix)]
            if name == "pkg_check_modules" {
                let args = command.args;
                let package_names: Vec<&str> = args
                    .iter()
                    .map(|arg| arg.utf8_text(source_bytes).unwrap())
                    .collect();
                let package_name = package_names[0];

                let modernpkgconfig = package_names.contains(&PKG_IMPORT_TARGET);
                if modernpkgconfig
                    && matches!(postype, PositionType::VarOrFun | PositionType::TargetLink)
                {
                    complete.push(CompletionItem {
                        label: format!("PkgConfig::{package_name}"),
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: Some("Package".to_string()),
                        documentation: Some(Documentation::String(format!(
                            "package: {package_name}",
                        ))),
                        ..Default::default()
                    });
                }
                if matches!(postype, PositionType::TargetLink | PositionType::VarOrFun) {
                    complete.push(CompletionItem {
                        label: format!("{package_name}_LIBRARIES"),
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: Some("Package".to_string()),
                        documentation: Some(Documentation::String(format!(
                            "package: {package_name}",
                        ))),
                        ..Default::default()
                    });
                }
                if matches!(
                    postype,
                    PositionType::TargetInclude | PositionType::VarOrFun
                ) {
                    complete.push(CompletionItem {
                        label: format!("{package_name}_INCLUDE_DIRS"),
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: Some("Package".to_string()),
                        documentation: Some(Documentation::String(format!(
                            "package: {package_name}",
                        ))),
                        ..Default::default()
                    });
                }
            }
        }
    }
    if complete.is_empty() {
        None
    } else {
        Some(complete)
    }
}

fn get_cmake_package_complete(
    package_name: &str,
    postype: PositionType,
    include_files: &mut Vec<PathBuf>,
    complete_packages: &mut Vec<String>,
) -> Option<Vec<CompletionItem>> {
    let packageinfo = CACHE_CMAKE_PACKAGES_WITHKEYS.get(package_name)?;
    let mut complete_infos = Vec::new();

    for path in packageinfo.tojump.iter() {
        let Some(mut packages) = includescanner::scanner_package_complete(
            path,
            postype,
            include_files,
            complete_packages,
        ) else {
            continue;
        };
        complete_infos.append(&mut packages);
    }

    Some(complete_infos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rst_doc_read_test() {
        let doc = r#"
#[=======================================================================[.rst:
CMakePackageConfigHelpers
-------------------------

Helpers functions for creating config files that can be included by other
projects to find and use a package.

Adds the :command:`configure_package_config_file()` and
:command:`write_basic_package_version_file()` commands.

Generating a Package Configuration File
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. command:: configure_package_config_file

 Create a config file for a project::

   configure_package_config_file(<input> <output>
     INSTALL_DESTINATION <path>
     [PATH_VARS <var1> <var2> ... <varN>]
     [NO_SET_AND_CHECK_MACRO]
     [NO_CHECK_REQUIRED_COMPONENTS_MACRO]
     [INSTALL_PREFIX <path>]
     )

``configure_package_config_file()`` should be used instead of the plain
:command:`configure_file()` command when creating the ``<PackageName>Config.cmake``
or ``<PackageName>-config.cmake`` file for installing a project or library.
It helps making the resulting package relocatable by avoiding hardcoded paths
in the installed ``Config.cmake`` file.

In a ``FooConfig.cmake`` file there may be code like this to make the install
destinations know to the using project:

.. code-block:: cmake

   set(FOO_INCLUDE_DIR   "@CMAKE_INSTALL_FULL_INCLUDEDIR@" )
   set(FOO_DATA_DIR   "@CMAKE_INSTALL_PREFIX@/@RELATIVE_DATA_INSTALL_DIR@" )
   set(FOO_ICONS_DIR   "@CMAKE_INSTALL_PREFIX@/share/icons" )
   #...logic to determine installedPrefix from the own location...
   set(FOO_CONFIG_DIR  "${installedPrefix}/@CONFIG_INSTALL_DIR@" )

All 4 options shown above are not sufficient, since the first 3 hardcode the
absolute directory locations, and the 4th case works only if the logic to
determine the ``installedPrefix`` is correct, and if ``CONFIG_INSTALL_DIR``
contains a relative path, which in general cannot be guaranteed.  This has the
effect that the resulting ``FooConfig.cmake`` file would work poorly under
Windows and OSX, where users are used to choose the install location of a
binary package at install time, independent from how
:variable:`CMAKE_INSTALL_PREFIX` was set at build/cmake time.

Using ``configure_package_config_file`` helps.  If used correctly, it makes
the resulting ``FooConfig.cmake`` file relocatable.  Usage:

1. write a ``FooConfig.cmake.in`` file as you are used to
2. insert a line containing only the string ``@PACKAGE_INIT@``
3. instead of ``set(FOO_DIR "@SOME_INSTALL_DIR@")``, use
   ``set(FOO_DIR "@PACKAGE_SOME_INSTALL_DIR@")`` (this must be after the
   ``@PACKAGE_INIT@`` line)
4. instead of using the normal :command:`configure_file()`, use
   ``configure_package_config_file()``



The ``<input>`` and ``<output>`` arguments are the input and output file, the
same way as in :command:`configure_file()`.

The ``<path>`` given to ``INSTALL_DESTINATION`` must be the destination where
the ``FooConfig.cmake`` file will be installed to.  This path can either be
absolute, or relative to the ``INSTALL_PREFIX`` path.

The variables ``<var1>`` to ``<varN>`` given as ``PATH_VARS`` are the
variables which contain install destinations.  For each of them the macro will
create a helper variable ``PACKAGE_<var...>``.  These helper variables must be
used in the ``FooConfig.cmake.in`` file for setting the installed location.
They are calculated by ``configure_package_config_file`` so that they are
always relative to the installed location of the package.  This works both for
relative and also for absolute locations.  For absolute locations it works
only if the absolute location is a subdirectory of ``INSTALL_PREFIX``.

.. versionadded:: 3.1
  If the ``INSTALL_PREFIX`` argument is passed, this is used as base path to
  calculate all the relative paths.  The ``<path>`` argument must be an absolute
  path.  If this argument is not passed, the :variable:`CMAKE_INSTALL_PREFIX`
  variable will be used instead.  The default value is good when generating a
  FooConfig.cmake file to use your package from the install tree.  When
  generating a FooConfig.cmake file to use your package from the build tree this
  option should be used.

By default ``configure_package_config_file`` also generates two helper macros,
``set_and_check()`` and ``check_required_components()`` into the
``FooConfig.cmake`` file.

``set_and_check()`` should be used instead of the normal ``set()`` command for
setting directories and file locations.  Additionally to setting the variable
it also checks that the referenced file or directory actually exists and fails
with a ``FATAL_ERROR`` otherwise.  This makes sure that the created
``FooConfig.cmake`` file does not contain wrong references.
When using the ``NO_SET_AND_CHECK_MACRO``, this macro is not generated
into the ``FooConfig.cmake`` file.

``check_required_components(<PackageName>)`` should be called at the end of
the ``FooConfig.cmake`` file. This macro checks whether all requested,
non-optional components have been found, and if this is not the case, sets
the ``Foo_FOUND`` variable to ``FALSE``, so that the package is considered to
be not found.  It does that by testing the ``Foo_<Component>_FOUND``
variables for all requested required components.  This macro should be
called even if the package doesn't provide any components to make sure
users are not specifying components erroneously.  When using the
``NO_CHECK_REQUIRED_COMPONENTS_MACRO`` option, this macro is not generated
into the ``FooConfig.cmake`` file.

For an example see below the documentation for
:command:`write_basic_package_version_file()`.

Generating a Package Version File
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. command:: write_basic_package_version_file

 Create a version file for a project::

   write_basic_package_version_file(<filename>
     [VERSION <major.minor.patch>]
     COMPATIBILITY <AnyNewerVersion|SameMajorVersion|SameMinorVersion|ExactVersion>
     [ARCH_INDEPENDENT] )


Writes a file for use as ``<PackageName>ConfigVersion.cmake`` file to
``<filename>``.  See the documentation of :command:`find_package()` for
details on this.

``<filename>`` is the output filename, it should be in the build tree.
``<major.minor.patch>`` is the version number of the project to be installed.

If no ``VERSION`` is given, the :variable:`PROJECT_VERSION` variable is used.
If this hasn't been set, it errors out.

The ``COMPATIBILITY`` mode ``AnyNewerVersion`` means that the installed
package version will be considered compatible if it is newer or exactly the
same as the requested version.  This mode should be used for packages which
are fully backward compatible, also across major versions.
If ``SameMajorVersion`` is used instead, then the behavior differs from
``AnyNewerVersion`` in that the major version number must be the same as
requested, e.g.  version 2.0 will not be considered compatible if 1.0 is
requested.  This mode should be used for packages which guarantee backward
compatibility within the same major version.
If ``SameMinorVersion`` is used, the behavior is the same as
``SameMajorVersion``, but both major and minor version must be the same as
requested, e.g version 0.2 will not be compatible if 0.1 is requested.
If ``ExactVersion`` is used, then the package is only considered compatible if
the requested version matches exactly its own version number (not considering
the tweak version).  For example, version 1.2.3 of a package is only
considered compatible to requested version 1.2.3.  This mode is for packages
without compatibility guarantees.
If your project has more elaborated version matching rules, you will need to
write your own custom ``ConfigVersion.cmake`` file instead of using this
macro.

.. versionadded:: 3.11
  The ``SameMinorVersion`` compatibility mode.

.. versionadded:: 3.14
  If ``ARCH_INDEPENDENT`` is given, the installed package version will be
  considered compatible even if it was built for a different architecture than
  the requested architecture.  Otherwise, an architecture check will be performed,
  and the package will be considered compatible only if the architecture matches
  exactly.  For example, if the package is built for a 32-bit architecture, the
  package is only considered compatible if it is used on a 32-bit architecture,
  unless ``ARCH_INDEPENDENT`` is given, in which case the package is considered
  compatible on any architecture.

.. note:: ``ARCH_INDEPENDENT`` is intended for header-only libraries or similar
  packages with no binaries.

.. versionadded:: 3.19
  The version file generated by ``AnyNewerVersion``, ``SameMajorVersion`` and
  ``SameMinorVersion`` arguments of ``COMPATIBILITY`` handle the version range
  if any is specified (see :command:`find_package` command for the details).
  ``ExactVersion`` mode is incompatible with version ranges and will display an
  author warning if one is specified.

Internally, this macro executes :command:`configure_file()` to create the
resulting version file.  Depending on the ``COMPATIBILITY``, the corresponding
``BasicConfigVersion-<COMPATIBILITY>.cmake.in`` file is used.
Please note that these files are internal to CMake and you should not call
:command:`configure_file()` on them yourself, but they can be used as starting
point to create more sophisticated custom ``ConfigVersion.cmake`` files.

Example Generating Package Files
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

Example using both :command:`configure_package_config_file` and
``write_basic_package_version_file()``:

``CMakeLists.txt``:

.. code-block:: cmake

   include(GNUInstallDirs)
   set(INCLUDE_INSTALL_DIR ${CMAKE_INSTALL_INCLUDEDIR}/Foo
       CACHE PATH "Location of header files" )
   set(SYSCONFIG_INSTALL_DIR ${CMAKE_INSTALL_SYSCONFDIR}/foo
       CACHE PATH "Location of configuration files" )
   #...
   include(CMakePackageConfigHelpers)
   configure_package_config_file(FooConfig.cmake.in
     ${CMAKE_CURRENT_BINARY_DIR}/FooConfig.cmake
     INSTALL_DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/Foo
     PATH_VARS INCLUDE_INSTALL_DIR SYSCONFIG_INSTALL_DIR)
   write_basic_package_version_file(
     ${CMAKE_CURRENT_BINARY_DIR}/FooConfigVersion.cmake
     VERSION 1.2.3
     COMPATIBILITY SameMajorVersion )
   install(FILES ${CMAKE_CURRENT_BINARY_DIR}/FooConfig.cmake
                 ${CMAKE_CURRENT_BINARY_DIR}/FooConfigVersion.cmake
           DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/Foo )

``FooConfig.cmake.in``:

::

   set(FOO_VERSION x.y.z)
   ...
   @PACKAGE_INIT@
   ...
   set_and_check(FOO_INCLUDE_DIR "@PACKAGE_INCLUDE_INSTALL_DIR@")
   set_and_check(FOO_SYSCONFIG_DIR "@PACKAGE_SYSCONFIG_INSTALL_DIR@")

   check_required_components(Foo)
#]=======================================================================]
        "#;
        assert_eq!(rst_doc_read(doc, "FileExample.cmake").len(), 2);
    }

    use crate::utils::LineCommentTmp;
    #[test]
    fn comment_mark_test() {
        let temp = LineCommentTmp {
            end_y: 1,
            comments: vec![],
        };

        assert!(!temp.is_node_comment(2));

        let temp = LineCommentTmp {
            end_y: 1,
            comments: vec!["# ABCD"],
        };
        assert!(temp.is_node_comment(2));
        assert!(!temp.is_node_comment(1));
        assert!(!temp.is_node_comment(0));
        assert_eq!(temp.comment(), "ABCD");
    }

    #[test]
    fn test_complete() {
        use std::fs::File;
        use std::io::Write;

        use tempfile::tempdir;

        let file_info = r#"
set(AB "100")
function(bb)
endfunction()
    "#;

        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let thetree = parse.parse(file_info, None).unwrap();
        let dir = tempdir().unwrap();
        let root_cmake = dir.path().join("CMakeList.txt");
        let mut file = File::create(&root_cmake).unwrap();
        writeln!(file, "{}", file_info).unwrap();
        let data = getsubcomplete(
            thetree.root_node(),
            file_info,
            &root_cmake,
            PositionType::VarOrFun,
            None,
            &mut vec![],
            &mut vec![],
            false,
            false,
        )
        .unwrap();
        assert_eq!(
            data,
            vec![
                CompletionItem {
                    label: "bb".to_string(),
                    label_details: None,
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some("Function".to_string()),
                    documentation: Some(Documentation::String(format!(
                        "defined function\nfrom: {}",
                        root_cmake.display()
                    ))),
                    deprecated: None,
                    preselect: None,
                    sort_text: None,
                    filter_text: None,
                    insert_text: None,
                    insert_text_format: None,
                    insert_text_mode: None,
                    text_edit: None,
                    additional_text_edits: None,
                    command: None,
                    commit_characters: None,
                    data: None,
                    tags: None
                },
                CompletionItem {
                    label: "AB".to_string(),
                    label_details: None,
                    kind: Some(CompletionItemKind::VALUE),
                    detail: Some("Value".to_string()),
                    documentation: Some(Documentation::String(format!(
                        "defined variable\nfrom: {}",
                        root_cmake.display()
                    ))),
                    deprecated: None,
                    preselect: None,
                    sort_text: None,
                    filter_text: None,
                    insert_text: None,
                    insert_text_format: None,
                    insert_text_mode: None,
                    text_edit: None,
                    additional_text_edits: None,
                    command: None,
                    commit_characters: None,
                    data: None,
                    tags: None
                },
            ]
        );
    }

    #[test]
    fn test_complete_win() {
        use std::fs::File;
        use std::io::Write;

        use tempfile::tempdir;

        let file_info = "set(AB \"100\")\r\n# test hello \r\nfunction(bb)\r\nendfunction()";

        let mut parse = tree_sitter::Parser::new();
        parse.set_language(&TREESITTER_CMAKE_LANGUAGE).unwrap();
        let thetree = parse.parse(file_info, None).unwrap();
        let dir = tempdir().unwrap();
        let root_cmake = dir.path().join("CMakeList.txt");
        let mut file = File::create(&root_cmake).unwrap();
        writeln!(file, "{}", &file_info).unwrap();
        let data = getsubcomplete(
            thetree.root_node(),
            file_info,
            &root_cmake,
            PositionType::VarOrFun,
            None,
            &mut vec![],
            &mut vec![],
            false,
            false,
        )
        .unwrap();
        assert_eq!(
            data,
            vec![
                CompletionItem {
                    label: "bb".to_string(),
                    label_details: None,
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some("Function".to_string()),
                    documentation: Some(Documentation::String(format!(
                        "defined function\nfrom: {}\n\ntest hello",
                        root_cmake.display()
                    ))),
                    deprecated: None,
                    preselect: None,
                    sort_text: None,
                    filter_text: None,
                    insert_text: None,
                    insert_text_format: None,
                    insert_text_mode: None,
                    text_edit: None,
                    additional_text_edits: None,
                    command: None,
                    commit_characters: None,
                    data: None,
                    tags: None
                },
                CompletionItem {
                    label: "AB".to_string(),
                    label_details: None,
                    kind: Some(CompletionItemKind::VALUE),
                    detail: Some("Value".to_string()),
                    documentation: Some(Documentation::String(format!(
                        "defined variable\nfrom: {}",
                        root_cmake.display()
                    ))),
                    deprecated: None,
                    preselect: None,
                    sort_text: None,
                    filter_text: None,
                    insert_text: None,
                    insert_text_format: None,
                    insert_text_mode: None,
                    text_edit: None,
                    additional_text_edits: None,
                    command: None,
                    commit_characters: None,
                    data: None,
                    tags: None
                },
            ]
        );
    }
}
