pub mod builtin;
mod findpackage;
mod includescanner;
use std::collections::HashMap;
use std::fs::read_dir;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use builtin::{BUILTIN_COMMAND, BUILTIN_MODULE, BUILTIN_VARIABLE};
use dashmap::DashMap;
use glob::glob;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionResponse, Documentation, Location, MessageType,
    Position, Range as LspRange, Uri,
};

use crate::config::CommandCase;
use crate::consts::TREESITTER_CMAKE_LANGUAGE;
use crate::fileapi;
use crate::languageserver::{get_extra_files, get_or_update_buffer_contents};
use crate::scansubs::TREE_MAP;
use crate::utils::query::{
    FunMarcoArg, ToLspRange, get_bracket_comments, get_functions, get_line_comments, get_macros,
    get_normal_commands,
};
use crate::utils::treehelper::{
    CurrentNodeInfo, PositionType, ToPoint, ToPosition, location_range_contain,
};
use crate::utils::{
    CACHE_CMAKE_PACKAGES_WITHKEYS, NeoStrExt, gen_module_pattern, include_is_module,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PCacheInfo {
    range: LspRange,
    is_definition: bool,
    location: Option<Location>,
    completion: CompletionItem,
}

impl PCacheInfo {
    pub fn location(&self) -> Option<Location> {
        self.location.clone()
    }
    pub const fn range(&self) -> LspRange {
        self.range
    }
    pub const fn completion(&self) -> &CompletionItem {
        &self.completion
    }
    pub fn new(completion: CompletionItem, range: impl ToLspRange) -> Self {
        Self {
            range: range.lsp_range(),
            location: None,
            completion,
            is_definition: true,
        }
    }

    // NOTE: since we will always has the document
    pub fn document(&self) -> &str {
        let document = self.documentation.as_ref().unwrap();
        match document {
            Documentation::String(context) => context.as_str(),
            Documentation::MarkupContent(context) => &context.value,
        }
    }

    pub const fn not_definition(mut self) -> Self {
        self.is_definition = false;
        self
    }

    pub fn with_location(mut self, location: Location) -> Self {
        self.location = Some(location);
        self
    }
}

impl Deref for PCacheInfo {
    type Target = CompletionItem;
    fn deref(&self) -> &Self::Target {
        &self.completion
    }
}

impl From<PCacheInfo> for CompletionItem {
    fn from(val: PCacheInfo) -> Self {
        val.completion
    }
}

impl From<&PCacheInfo> for CompletionItem {
    fn from(val: &PCacheInfo) -> Self {
        val.completion.clone()
    }
}

pub type CompleteKV = HashMap<PathBuf, HashMap<String, PCacheInfo>>;

/// NOTE: collect the all completeitems in this PathBuf
/// Include the top CMakeList.txt
pub static COMPLETION_CACHE: LazyLock<Arc<Mutex<CompleteKV>>> =
    LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

static LABEL_FILITER: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\$\{.*\}").unwrap());

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

trait CompletionFilter {
    fn filter_cache(self, command_case: CommandCase) -> Self
    where
        Self: Sized,
    {
        if matches!(command_case, CommandCase::Ignore) {
            return self;
        }
        self.filter_cache_command_case(command_case)
    }
    fn filter_cache_command_case(self, command_case: CommandCase) -> Self;
    fn filter_builtin(self, command_case: CommandCase) -> Self
    where
        Self: Sized,
    {
        if matches!(command_case, CommandCase::Ignore) {
            return self;
        }
        self.filter_builtin_command_case(command_case)
    }
    fn filter_builtin_command_case(self, command_case: CommandCase) -> Self;
}

impl CompletionFilter for HashMap<String, PCacheInfo> {
    fn filter_cache_command_case(self, command_case: CommandCase) -> Self {
        self.into_iter()
            .map(|(key, item)| {
                let mut label = item.label.clone();
                if item
                    .kind
                    .is_some_and(|kind| kind == CompletionItemKind::Function)
                {
                    label = command_case.operator(&label);
                }
                (
                    key,
                    PCacheInfo {
                        completion: CompletionItem {
                            label,
                            ..item.completion
                        },
                        ..item
                    },
                )
            })
            .collect()
    }
    fn filter_builtin_command_case(self, command_case: CommandCase) -> Self {
        self.into_iter()
            .filter(|(_, item)| {
                item.data.as_ref().is_some_and(|data| {
                    serde_json::from_value::<CommandCase>(data.clone())
                        .is_ok_and(|case| case == command_case)
                })
            })
            .collect()
    }
}
impl CompletionFilter for Vec<PCacheInfo> {
    fn filter_cache_command_case(self, command_case: CommandCase) -> Self {
        self.into_iter()
            .map(|item| {
                let mut label = item.label.clone();
                if item
                    .kind
                    .is_some_and(|kind| kind == CompletionItemKind::Function)
                {
                    label = command_case.operator(&label);
                }
                PCacheInfo {
                    completion: CompletionItem {
                        label,
                        ..item.completion
                    },
                    ..item
                }
            })
            .collect()
    }
    fn filter_builtin_command_case(self, command_case: CommandCase) -> Self {
        self.into_iter()
            .filter(|item| {
                item.data.as_ref().is_some_and(|data| {
                    serde_json::from_value::<CommandCase>(data.clone())
                        .is_ok_and(|case| case == command_case)
                })
            })
            .collect()
    }
}

impl CompletionFilter for Vec<CompletionItem> {
    fn filter_cache_command_case(self, command_case: CommandCase) -> Self {
        self.into_iter()
            .map(|item| {
                let mut label = item.label;
                if item
                    .kind
                    .is_some_and(|kind| kind == CompletionItemKind::Function)
                {
                    label = command_case.operator(&label);
                }
                CompletionItem { label, ..item }
            })
            .collect()
    }
    fn filter_builtin_command_case(self, command_case: CommandCase) -> Self {
        self.into_iter()
            .filter(|item| {
                item.data.as_ref().is_some_and(|data| {
                    serde_json::from_value::<CommandCase>(data.clone())
                        .is_ok_and(|case| case == command_case)
                })
            })
            .collect()
    }
}

pub fn rst_doc_read(doc: &str, filename: &str) -> Vec<PCacheInfo> {
    doc.lines()
        .filter(|line| line.starts_with(".. command:: "))
        .map(|line| &line[13..])
        .map(|line| {
            PCacheInfo::new(
                CompletionItem {
                    label: line.to_string(),
                    kind: Some(CompletionItemKind::Function),
                    detail: Some("Command".to_string()),
                    documentation: Some(Documentation::String(format!(
                        "defined command from {filename}\n{doc}"
                    ))),
                    ..Default::default()
                },
                0,
            )
            .not_definition()
        })
        .collect()
}

pub async fn update_cache<P: AsRef<Path>>(path: P, context: &str) -> HashMap<String, PCacheInfo> {
    update_cache_inner(path, context, &mut Vec::new(), &mut Vec::new()).await
}

pub async fn update_cache_inner<P: AsRef<Path>>(
    path: P,
    context: &str,
    include_files: &mut Vec<PathBuf>,
    complete_packages: &mut Vec<String>,
) -> HashMap<String, PCacheInfo> {
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
        include_files,
        complete_packages,
        true,
        true,
    ) else {
        return HashMap::new();
    };
    let mut cache = COMPLETION_CACHE.lock().await;
    cache.insert(path.as_ref().to_path_buf(), result_data.clone());
    result_data
}

pub struct CachedInfo {
    pub completions: HashMap<String, PCacheInfo>,
    pub include_files: Vec<PathBuf>,
    pub complete_packages: Vec<String>,
}

/// NOTE: it scans the cached data, and filter the scanned packages and include cmake files
pub async fn get_cached_information<P: AsRef<Path>>(
    path: P,
    documents: &DashMap<Uri, String>,
) -> CachedInfo {
    let mut include_files = Vec::new();
    let mut complete_packages = Vec::new();
    let mut extra_files = get_extra_files();
    let mut path = path.as_ref().to_path_buf();
    let mut completions = HashMap::new();

    let tree_map = TREE_MAP.lock().await;

    let mut update_cache_fn = async |file_path: &PathBuf| {
        if !file_path.ends_with("CMakeLists.txt") {
            include_files.push(file_path.clone());
        }
        let complete_cache = COMPLETION_CACHE.lock().await;
        if let Some(data) = complete_cache.get(file_path) {
            completions.extend(data.clone());
        } else if let Ok(context) = get_or_update_buffer_contents(file_path, documents).await {
            drop(complete_cache);
            completions.extend(
                update_cache_inner(
                    file_path,
                    context.as_str(),
                    &mut include_files,
                    &mut complete_packages,
                )
                .await,
            );
        }
    };

    while let Some(parent) = tree_map.get(&path) {
        if let Some(index) = extra_files.iter().position(|pa| pa == parent) {
            extra_files.remove(index);
        }
        update_cache_fn(parent).await;
        path.clone_from(parent);
    }

    for extra in extra_files {
        update_cache_fn(&extra).await;
    }

    CachedInfo {
        completions,
        include_files,
        complete_packages,
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TriggerType {
    Normal,
    Path,
}

impl From<Option<String>> for TriggerType {
    fn from(value: Option<String>) -> Self {
        let value_ref = value.as_ref();
        match value_ref.map(|v| v.as_str()) {
            Some("/") => Self::Path,
            _ => Self::Normal,
        }
    }
}

/// get the complete messages
pub async fn getcompletions<P: AsRef<Path>>(
    source: &str,
    triggered: impl Into<TriggerType>,
    position: Position,
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
    let mut completions: Vec<CompletionItem> = vec![];

    let current_point = position.to_point();
    let node_info = CurrentNodeInfo::get(source, tree.root_node(), current_point);
    let postype = node_info.pos_type();
    if matches!(triggered.into(), TriggerType::Path)
        // NOTE: skip SubDir
        && !matches!(postype, PositionType::SubDir)
        && let Some(promopt) = node_info.content()
        && let Some(prompt) = promopt.try_replace_placeholders()
        && let Some(list) = get_path_completions(&prompt, local_path)
        && !list.is_empty()
    {
        completions.extend(list);
        return Some(CompletionResponse::CompletionItemList(completions));
    }
    let command_case = crate::config::CONFIG.command_case;
    match postype {
        PositionType::VarOrFun | PositionType::TargetLink | PositionType::TargetInclude => {
            let CachedInfo {
                completions: cached_completion,
                mut include_files,
                mut complete_packages,
            } = get_cached_information(local_path, documents).await;
            if !cached_completion.is_empty() {
                completions.extend(
                    cached_completion
                        .filter_cache(command_case)
                        .into_values()
                        .map(|info| info.completion)
                        .collect::<Vec<CompletionItem>>(),
                );
            }
            if let Some(cmake_cache) = fileapi::get_complete_data() {
                completions.extend(cmake_cache);
            }
            if let Some(message) = getsubcomplete(
                tree.root_node(),
                source,
                Path::new(local_path),
                postype,
                Some(position),
                &mut include_files,
                &mut complete_packages,
                true,
                find_cmake_in_package,
            ) {
                let messages: Vec<CompletionItem> =
                    message.into_values().map(|info| info.completion).collect();
                completions.extend(messages);
            }

            if !node_info.in_argument_list() {
                let messages = (*BUILTIN_COMMAND).clone().filter_builtin(command_case);
                completions.extend(messages);
            }
            if let Ok(messages) = &*BUILTIN_VARIABLE {
                completions.extend(messages.clone());
            }
        }
        PositionType::FindPackageSpace(space) => {
            completions.extend(findpackage::completion_items_with_prefix(space));
        }
        PositionType::FindPackage => {
            completions.extend(findpackage::CMAKE_SOURCE.clone());
        }
        #[cfg(unix)]
        PositionType::FindPkgConfig => {
            completions.extend(findpackage::PKGCONFIG_SOURCE.clone());
        }
        PositionType::Include => {
            let CachedInfo {
                completions: cached_completions,
                ..
            } = get_cached_information(local_path, documents).await;
            if !cached_completions.is_empty() {
                completions.extend(
                    cached_completions
                        .into_values()
                        .map(|info| info.completion)
                        .collect::<Vec<CompletionItem>>(),
                );
            }
            if let Some(cmake_cache) = fileapi::get_complete_data() {
                completions.extend(cmake_cache);
            }
            if let Ok(messages) = &*BUILTIN_MODULE {
                completions.extend(messages.clone());
            }
            if let Some(prompt) = node_info.content()
                && node_info.is_first_argument()
                && let Some(prompt) = prompt.try_replace_placeholders()
                && let Some(list) = get_include_completions(&prompt, local_path)
            {
                completions.extend(list);
            }
        }
        PositionType::SubDir => {
            if let Some(promopt) = node_info.content()
                && node_info.is_first_argument()
                && let Some(prompt) = promopt.try_replace_placeholders()
                && let Some(list) = get_subdir_completions(&prompt, local_path)
            {
                completions.extend(list);
            }
        }
        PositionType::Comment => {
            client.log_message(MessageType::Info, "Empty").await;
            return None;
        }
        _ => {}
    }

    if completions.is_empty() {
        client.log_message(MessageType::Info, "Empty").await;
        None
    } else {
        Some(CompletionResponse::CompletionItemList(completions))
    }
}

fn get_include_completions<P: AsRef<Path>>(
    prompt: &str,
    local_path: P,
) -> Option<Vec<CompletionItem>> {
    let current_dir = local_path.as_ref().parent()?;
    let dir_str = current_dir.to_str()?;
    let pattern1 = format!("{dir_str}/{prompt}*/**/*.cmake");

    let mut lists: Vec<CompletionItem> = glob(&pattern1)
        .ok()?
        .flatten()
        .filter_map(|path| pathdiff::diff_paths(path, current_dir))
        .map(|pa| {
            let label = pa.to_string_lossy().to_string();
            CompletionItem {
                label,
                kind: Some(CompletionItemKind::File),
                ..Default::default()
            }
        })
        .collect();
    let pattern2 = format!("{dir_str}/{prompt}*.cmake");
    let lists2: Vec<CompletionItem> = glob(&pattern2)
        .ok()?
        .flatten()
        .filter_map(|pa| {
            let pa = pathdiff::diff_paths(pa, current_dir)?;
            let label = pa.to_string_lossy().to_string();
            Some(CompletionItem {
                label,
                kind: Some(CompletionItemKind::File),
                ..Default::default()
            })
        })
        .collect();
    lists.extend(lists2);
    Some(lists)
}

fn get_subdir_completions<P: AsRef<Path>>(
    prompt: &str,
    local_path: P,
) -> Option<Vec<CompletionItem>> {
    let current_dir = local_path.as_ref().parent()?;
    let dir_str = current_dir.to_str()?;
    let pattern = format!("{dir_str}/{prompt}*/**/CMakeLists.txt");
    Some(
        glob(&pattern)
            .ok()?
            .flatten()
            .filter_map(|path| {
                path.parent()
                    .and_then(|path| pathdiff::diff_paths(path, current_dir))
            })
            .map(|pa| {
                let label = pa.to_string_lossy().to_string();
                CompletionItem {
                    label,
                    kind: Some(CompletionItemKind::Folder),
                    ..Default::default()
                }
            })
            .collect(),
    )
}

fn get_path_completions<SubP: AsRef<Path>, P: AsRef<Path>>(
    prompt: SubP,
    local_path: P,
) -> Option<Vec<CompletionItem>> {
    let mut next_dir = prompt.as_ref().to_owned();
    if next_dir.is_relative() {
        let current_dir = local_path.as_ref().parent()?;
        next_dir = current_dir.join(next_dir);
    }

    if !next_dir.is_dir() {
        return None;
    }
    let mut completions = vec![];

    for entry in read_dir(next_dir).ok()?.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let is_file = metadata.is_file();
        let is_dir = metadata.is_dir();
        if !is_file && !is_dir {
            continue;
        }
        let name = entry.file_name();
        let label = name.to_string_lossy().to_string();
        completions.push(CompletionItem {
            label,
            kind: if is_dir {
                Some(CompletionItemKind::Folder)
            } else {
                Some(CompletionItemKind::File)
            },
            ..Default::default()
        });
    }

    Some(completions)
}

/// TODO: move it to scansubs
/// We can add some information to mark if this variable is a function or just a valuable, or just
/// an argument.
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
) -> Option<HashMap<String, PCacheInfo>> {
    assert!(matches!(
        postype,
        PositionType::VarOrFun | PositionType::TargetLink | PositionType::TargetInclude
    ));
    let local_path = local_path.as_ref();
    let source_bytes = source.as_bytes();
    let mut completions: HashMap<String, PCacheInfo> = HashMap::new();

    let end_location = location.map(|p| p.to_point());

    // NOTE: prepare
    let bracket_comments = get_bracket_comments(source_bytes, input, end_location);
    let comments = get_line_comments(source_bytes, input, end_location);

    let macros = get_macros(source_bytes, input, end_location);
    let functions = get_functions(source_bytes, input, end_location);
    let normal_commands = get_normal_commands(source_bytes, input, end_location);
    // NOTE: check bracket_comments
    for bracket_comment in bracket_comments {
        let comments = rst_doc_read(
            bracket_comment.content,
            local_path.file_name().unwrap().to_str().unwrap(),
        );
        for comment in comments {
            completions.insert(comment.label.clone(), comment);
        }
    }
    use tower_lsp::lsp_types::Range;
    // NOTE: check functions
    for fun in functions {
        let name = fun.name;
        let row = fun.node.start_position().row;
        let column = fun.node.start_position().column;

        let mut document_info = format!("defined function\nfrom: {}", local_path.display());
        let mut variable_info = format!("defined variable\nfrom: {}", local_path.display());
        if let Some(line_comment) = comments
            .iter()
            .find(|c| {
                c.node.start_position().row + 1 == row && c.node.start_position().column == column
            })
            .map(|c| c.content)
        {
            document_info = format!("{}\n\n{}", document_info, line_comment);
            variable_info = format!("{}\n\n{}", variable_info, line_comment);
        }
        let node_location = Location {
            uri: Uri::from_file_path(local_path).unwrap(),
            range: Range {
                start: fun.node.start_position().to_position(),
                end: fun.node.end_position().to_position(),
            },
        };
        completions.insert(
            name.to_owned(),
            PCacheInfo::new(
                CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::Function),
                    detail: Some("Function".to_string()),
                    documentation: Some(Documentation::String(document_info)),
                    ..Default::default()
                },
                fun.node.range(),
            )
            .with_location(node_location),
        );
        if let Some(location) = location
            && location_range_contain(location.to_point(), fun.node)
        {
            for FunMarcoArg { content, node } in fun.args(source_bytes) {
                let node_location = Location {
                    uri: Uri::from_file_path(local_path).unwrap(),
                    range: Range {
                        start: node.start_position().to_position(),
                        end: node.end_position().to_position(),
                    },
                };
                completions.insert(
                    content.to_owned(),
                    PCacheInfo::new(
                        CompletionItem {
                            label: content.to_string(),
                            kind: Some(CompletionItemKind::Variable),
                            detail: Some("VARIABLE".to_string()),
                            documentation: Some(Documentation::String(variable_info.clone())),
                            ..Default::default()
                        },
                        node.range(),
                    )
                    .with_location(node_location)
                    .not_definition(),
                );
            }
        }
    }

    // NOTE: check macros
    for macro_node in macros {
        let name = macro_node.name;
        let row = macro_node.node.start_position().row;
        let column = macro_node.node.start_position().column;

        let mut document_info = format!("defined macro\nfrom: {}", local_path.display());
        let mut variable_info = format!("defined variable\nfrom: {}", local_path.display());
        if let Some(line_comment) = comments
            .iter()
            .find(|c| {
                c.node.start_position().row + 1 == row && c.node.start_position().column == column
            })
            .map(|c| c.content)
        {
            document_info = format!("{}\n\n{}", document_info, line_comment);
            variable_info = format!("{}\n\n{}", variable_info, line_comment);
        }
        let node_location = Location {
            uri: Uri::from_file_path(local_path).unwrap(),
            range: Range {
                start: macro_node.node.start_position().to_position(),
                end: macro_node.node.end_position().to_position(),
            },
        };
        completions.insert(
            name.to_owned(),
            PCacheInfo::new(
                CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::Function),
                    detail: Some("Function".to_string()),
                    documentation: Some(Documentation::String(document_info)),
                    ..Default::default()
                },
                macro_node.node.range(),
            )
            .with_location(node_location),
        );
        if let Some(location) = location
            && location_range_contain(location.to_point(), macro_node.node)
        {
            for FunMarcoArg { content, node } in macro_node.args(source_bytes) {
                let node_location = Location {
                    uri: Uri::from_file_path(local_path).unwrap(),
                    range: Range {
                        start: node.start_position().to_position(),
                        end: node.end_position().to_position(),
                    },
                };
                completions.insert(
                    content.to_owned(),
                    PCacheInfo::new(
                        CompletionItem {
                            label: content.to_string(),
                            kind: Some(CompletionItemKind::Variable),
                            detail: Some("VARIABLE".to_string()),
                            documentation: Some(Documentation::String(variable_info.clone())),
                            ..Default::default()
                        },
                        node.range(),
                    )
                    .with_location(node_location)
                    .not_definition(),
                );
            }
        }
    }

    // NOTE: check normal_commands
    for command in normal_commands {
        let name = command.identifier.to_lowercase();
        if name == "include" {
            let Some(first_arg) = command.first_arg else {
                continue;
            };
            let Some(file_name) = first_arg.try_replace_placeholders() else {
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
            if matches!(subpath.try_exists(), Ok(true)) {
                if let Some(comps) = includescanner::scanner_include_complete(
                    &subpath,
                    postype,
                    include_files,
                    complete_packages,
                    find_cmake_in_package,
                    is_builtin,
                ) {
                    for (key, comp) in comps {
                        completions.insert(key, comp);
                    }
                }
                include_files.push(subpath);
            }
        } else if name == "mark_as_advanced" {
            for arg in command.args {
                let variable = arg.utf8_text(source_bytes).unwrap();
                let node_location = Location {
                    uri: Uri::from_file_path(local_path).unwrap(),
                    range: Range {
                        start: arg.start_position().to_position(),
                        end: arg.end_position().to_position(),
                    },
                };
                completions.insert(
                    variable.to_owned(),
                    PCacheInfo::new(
                        CompletionItem {
                            label: variable.to_string(),
                            kind: Some(CompletionItemKind::Variable),
                            detail: Some("Variable".to_string()),
                            documentation: Some(Documentation::String(format!(
                                "defined var\nfrom: {}",
                                local_path.display()
                            ))),
                            ..Default::default()
                        },
                        arg.range(),
                    )
                    .with_location(node_location),
                );
            }
        } else {
            if name == "set" || name == "option" {
                let Some(name) = command.first_arg else {
                    continue;
                };
                // NOTE: some label is defined in macro with this way
                // I do not want to address macro, because the lsp does not need to be that clever
                // So I just do not complete them
                if LABEL_FILITER.is_match(name) {
                    continue;
                }
                let row = command.node.start_position().row;
                let column = command.node.start_position().column;
                let mut document_info = format!("defined variable\nfrom: {}", local_path.display());

                if let Some(line_comment) = comments
                    .iter()
                    .find(|c| {
                        c.node.start_position().row + 1 == row
                            && c.node.start_position().column == column
                    })
                    .map(|c| c.content)
                {
                    document_info = format!("{}\n\n{}", document_info, line_comment);
                }
                let node_location = Location {
                    uri: Uri::from_file_path(local_path).unwrap(),
                    range: Range {
                        start: command.args[0].start_position().to_position(),
                        end: command.args[0].end_position().to_position(),
                    },
                };
                completions.insert(
                    name.to_owned(),
                    PCacheInfo::new(
                        CompletionItem {
                            label: name.to_string(),
                            kind: Some(CompletionItemKind::Value),
                            detail: Some("Value".to_string()),
                            documentation: Some(Documentation::String(document_info)),
                            ..Default::default()
                        },
                        command.node.range(),
                    )
                    .with_location(node_location),
                );
            }
            if name == "find_package" && should_in {
                let Some(package_name) = command.first_arg else {
                    continue;
                };
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
                        completions.insert(
                            component.to_owned(),
                            PCacheInfo::new(
                                CompletionItem {
                                    label: component,
                                    kind: Some(CompletionItemKind::Variable),
                                    detail: Some("Variable".to_string()),
                                    documentation: Some(Documentation::String(format!(
                                        "package from: {package_name}",
                                    ))),
                                    ..Default::default()
                                },
                                command.node.range(),
                            )
                            .not_definition(),
                        );
                    }
                }

                if matches!(postype, PositionType::TargetLink | PositionType::VarOrFun) {
                    let label = format!("{package_name}_LIBRARIES");
                    completions.insert(
                        label.clone(),
                        PCacheInfo::new(
                            CompletionItem {
                                label,
                                kind: Some(CompletionItemKind::Variable),
                                detail: Some("Variable".to_string()),
                                documentation: Some(Documentation::String(format!(
                                    "package: {package_name}",
                                ))),
                                ..Default::default()
                            },
                            command.node.range(),
                        )
                        .not_definition(),
                    );
                }

                if matches!(
                    postype,
                    PositionType::TargetInclude | PositionType::VarOrFun
                ) {
                    let label = format!("{package_name}_INCLUDE_DIRS");
                    completions.insert(
                        label.clone(),
                        PCacheInfo::new(
                            CompletionItem {
                                label,
                                kind: Some(CompletionItemKind::Variable),
                                detail: Some("Variable".to_string()),
                                documentation: Some(Documentation::String(format!(
                                    "package: {package_name}",
                                ))),
                                ..Default::default()
                            },
                            command.node.range(),
                        ),
                    );
                }
                for package in cmakepackages {
                    if complete_packages.contains(&package) {
                        continue;
                    }
                    complete_packages.push(package.clone());
                    let Some(complete_items) = get_cmake_package_complete(
                        package.as_str(),
                        postype,
                        include_files,
                        complete_packages,
                    ) else {
                        continue;
                    };
                    for (key, complete_item) in complete_items {
                        completions.insert(key, complete_item);
                    }
                }
            }

            #[cfg(unix)]
            if name == "pkg_check_modules" {
                let args = command.args;
                let package_names: Vec<&str> = args
                    .iter()
                    .map(|arg| arg.utf8_text(source_bytes).unwrap())
                    .collect();
                if package_names.is_empty() {
                    continue;
                }
                let package_name = package_names[0];

                let modernpkgconfig = package_names.contains(&PKG_IMPORT_TARGET);
                if modernpkgconfig
                    && matches!(postype, PositionType::VarOrFun | PositionType::TargetLink)
                {
                    let label = format!("PkgConfig::{package_name}");
                    completions.insert(
                        label.clone(),
                        PCacheInfo::new(
                            CompletionItem {
                                label,
                                kind: Some(CompletionItemKind::Variable),
                                detail: Some("Package".to_string()),
                                documentation: Some(Documentation::String(format!(
                                    "package: {package_name}",
                                ))),
                                ..Default::default()
                            },
                            command.node.range(),
                        )
                        .not_definition(),
                    );
                }
                if matches!(postype, PositionType::TargetLink | PositionType::VarOrFun) {
                    let label = format!("{package_name}_LIBRARIES");
                    completions.insert(
                        label.clone(),
                        PCacheInfo::new(
                            CompletionItem {
                                label,
                                kind: Some(CompletionItemKind::Variable),
                                detail: Some("Package".to_string()),
                                documentation: Some(Documentation::String(format!(
                                    "package: {package_name}",
                                ))),
                                ..Default::default()
                            },
                            command.node.range(),
                        )
                        .not_definition(),
                    );
                }
                if matches!(
                    postype,
                    PositionType::TargetInclude | PositionType::VarOrFun
                ) {
                    let label = format!("{package_name}_INCLUDE_DIRS");
                    completions.insert(
                        label.clone(),
                        PCacheInfo::new(
                            CompletionItem {
                                label,
                                kind: Some(CompletionItemKind::Variable),
                                detail: Some("Package".to_string()),
                                documentation: Some(Documentation::String(format!(
                                    "package: {package_name}",
                                ))),
                                ..Default::default()
                            },
                            command.node.range(),
                        )
                        .not_definition(),
                    );
                }
            }
        }
    }
    if completions.is_empty() {
        None
    } else {
        Some(completions)
    }
}

fn get_cmake_package_complete(
    package_name: &str,
    postype: PositionType,
    include_files: &mut Vec<PathBuf>,
    complete_packages: &mut Vec<String>,
) -> Option<HashMap<String, PCacheInfo>> {
    let packageinfo = CACHE_CMAKE_PACKAGES_WITHKEYS.get(package_name)?;
    let mut complete_infos = HashMap::new();

    for path in packageinfo.tojump.iter() {
        let Some(packages) = includescanner::scanner_package_complete(
            path,
            postype,
            include_files,
            complete_packages,
        ) else {
            continue;
        };
        complete_infos.extend(packages);
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

    #[test]
    fn test_complete() {
        use std::fs::File;
        use std::io::Write;

        use tempfile::tempdir;

        let file_info = r#"
set(AB "100")
set(AB${CD} "100") # illegal
# it is a comment for AB
set(AB "100") # it will only show for once a time
# Fun bb
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
        let data: Vec<CompletionItem> = getsubcomplete(
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
        .unwrap()
        .into_values()
        .map(|info| info.completion)
        .collect();
        let test_data = vec![
            CompletionItem {
                label: "bb".to_string(),
                label_details: None,
                kind: Some(CompletionItemKind::Function),
                detail: Some("Function".to_string()),
                documentation: Some(Documentation::String(format!(
                    "defined function\nfrom: {}\n\nFun bb",
                    root_cmake.display()
                ))),
                text_edit_text: None,
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
                tags: None,
                ..Default::default()
            },
            CompletionItem {
                label: "AB".to_string(),
                label_details: None,
                kind: Some(CompletionItemKind::Value),
                detail: Some("Value".to_string()),
                documentation: Some(Documentation::String(format!(
                    "defined variable\nfrom: {}\n\nit is a comment for AB",
                    root_cmake.display()
                ))),
                text_edit_text: None,
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
                tags: None,
                ..Default::default()
            },
        ];
        assert_eq!(data.len(), test_data.len());
        for test_item in test_data {
            assert!(data.contains(&test_item));
        }
        let data = data.filter_cache(CommandCase::Upper);
        let test_data = vec![
            CompletionItem {
                label: "BB".to_string(),
                label_details: None,
                kind: Some(CompletionItemKind::Function),
                detail: Some("Function".to_string()),
                documentation: Some(Documentation::String(format!(
                    "defined function\nfrom: {}\n\nFun bb",
                    root_cmake.display()
                ))),
                text_edit_text: None,
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
                tags: None,
                ..Default::default()
            },
            CompletionItem {
                label: "AB".to_string(),
                label_details: None,
                kind: Some(CompletionItemKind::Value),
                detail: Some("Value".to_string()),
                documentation: Some(Documentation::String(format!(
                    "defined variable\nfrom: {}\n\nit is a comment for AB",
                    root_cmake.display()
                ))),
                text_edit_text: None,
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
                tags: None,
                ..Default::default()
            },
        ];
        assert_eq!(data.len(), test_data.len());
        for test_item in test_data {
            assert!(data.contains(&test_item));
        }
    }

    #[test]
    fn builtin_filter() {
        let item1 = CompletionItem {
            label: "lowerfun".to_string(),
            label_details: None,
            kind: Some(CompletionItemKind::Function),
            detail: Some("Function".to_string()),
            documentation: Some(Documentation::String(
                "defined function\nfrom: abc.cmake\n\nFun bb".to_string(),
            )),
            text_edit_text: None,
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
            data: Some(serde_json::to_value(CommandCase::Lower).unwrap()),
            tags: None,
            ..Default::default()
        };
        let item2 = CompletionItem {
            label: "UPPER_FN".to_string(),
            label_details: None,
            kind: Some(CompletionItemKind::Function),
            detail: Some("Function".to_string()),
            documentation: Some(Documentation::String(
                "defined function\nfrom: abc.cmake\n\nFun bb".to_string(),
            )),
            text_edit_text: None,
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
            data: Some(serde_json::to_value(CommandCase::Upper).unwrap()),
            tags: None,
            ..Default::default()
        };
        let test_data = vec![item1.clone(), item2.clone()];
        let test_data1 = test_data.clone().filter_builtin(CommandCase::Lower);
        assert_eq!(test_data1.len(), 1);
        assert_eq!(test_data1[0], item1);
        let test_data2 = test_data.clone().filter_builtin(CommandCase::Upper);
        assert_eq!(test_data2.len(), 1);
        assert_eq!(test_data2[0], item2);
        let test_data3 = test_data.clone().filter_builtin(CommandCase::Ignore);
        assert_eq!(test_data3.len(), 2);
        assert_eq!(test_data3, test_data);
    }
    #[test]
    fn label_filter() {
        assert!(LABEL_FILITER.is_match("abc${asbsd}abcd"));
        assert!(LABEL_FILITER.is_match("${asbsd}abcd"));
        assert!(LABEL_FILITER.is_match("abc${asbsd}"));
        assert!(LABEL_FILITER.is_match("${asbsd}"));
        assert!(!LABEL_FILITER.is_match("{asbsd}"));
        assert!(!LABEL_FILITER.is_match("asbsd"));
        assert!(!LABEL_FILITER.is_match("asbsd_abcd"));
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
        let test_data = vec![
            CompletionItem {
                label: "bb".to_string(),
                label_details: None,
                kind: Some(CompletionItemKind::Function),
                detail: Some("Function".to_string()),
                documentation: Some(Documentation::String(format!(
                    "defined function\nfrom: {}\n\ntest hello",
                    root_cmake.display()
                ))),
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
                text_edit_text: None,
                tags: None,
                ..Default::default()
            },
            CompletionItem {
                label: "AB".to_string(),
                label_details: None,
                kind: Some(CompletionItemKind::Value),
                detail: Some("Value".to_string()),
                documentation: Some(Documentation::String(format!(
                    "defined variable\nfrom: {}",
                    root_cmake.display()
                ))),
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
                tags: None,
                text_edit_text: None,

                ..Default::default()
            },
        ];
        assert_eq!(data.len(), test_data.len());
        for test_item in test_data {
            assert!(
                data.values()
                    .find(|item| item.completion == test_item)
                    .is_some()
            );
        }
    }
}
