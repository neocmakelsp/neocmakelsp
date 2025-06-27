use std::collections::HashMap;
use std::iter::zip;
use std::process::Command;
use std::sync::LazyLock;

/// builtin Commands and vars
use anyhow::Result;
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Documentation, InsertTextFormat};

use crate::languageserver::to_use_snippet;

fn shorter_var(arg: &str) -> String {
    let mut shorter = arg.to_string();
    let args = arg.split('\n').next().unwrap_or("");
    if args.len() > 20 {
        shorter = format!("{}...", &args[0..20]);
    }
    if shorter.contains(' ') {
        shorter = format!("(arg_type: <{shorter}>)");
    }
    shorter = format!("<{shorter}>");
    shorter
}

fn handle_sharp_bracket(arg: &str) -> &str {
    let left_unique = arg.starts_with("<");
    let right_unique = arg.ends_with(">");
    match (left_unique, right_unique) {
        (true, true) => &arg[1..arg.len() - 1],
        (true, false) => &arg[1..],
        (false, true) => &arg[..arg.len() - 1],
        (false, false) => arg,
    }
}

fn handle_square_bracket(arg: &str) -> &str {
    let left_unique = arg.starts_with("[");
    let right_unique = arg.ends_with("]");
    match (left_unique, right_unique) {
        (true, true) => &arg[1..arg.len() - 1],
        (true, false) => &arg[1..],
        (false, true) => &arg[..arg.len() - 1],
        (false, false) => arg,
    }
}

static SNIPPET_GEN_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"<[^>]+>").unwrap());

fn convert_to_lsp_snippet(input: &str) -> String {
    let mut result = String::new();
    let mut last_pos = 0; // Keep track of the last match position
    let mut i = 1;
    for caps in SNIPPET_GEN_REGEX.captures_iter(input) {
        if let Some(matched) = caps.get(0) {
            let var_name_pre = matched.as_str(); // Extract captured variable
            let var_name_pre2 = handle_sharp_bracket(var_name_pre);
            let var_name_pre3 = handle_square_bracket(var_name_pre2);
            let var_name = shorter_var(var_name_pre3);

            // Add text before the match
            result.push_str(&input[last_pos..matched.start()]);

            // Replace the variable
            result.push_str(&format!("${{{i}:{var_name}}}"));
            // Update last position to after this match
            last_pos = matched.end();
            i += 1;
        }
    }

    // Add remaining part of the string
    result.push_str(&input[last_pos..]);

    result
}

#[test]
fn tst_convert_to_lsp_snippet() {
    let snippet_example = r#"define_property(<GLOBAL | DIRECTORY | TARGET | SOURCE |
                  TEST | VARIABLE | CACHED_VARIABLE>
                  PROPERTY <name> [INHERITED]
                  [BRIEF_DOCS <brief-doc> [docs...]]
                  [FULL_DOCS <full-doc> [docs...]]
                  [INITIALIZE_FROM_VARIABLE <variable>])"#;
    let snippet_result = convert_to_lsp_snippet(snippet_example);
    let snippet_target = r#"define_property(${1:<(arg_type: <GLOBAL | DIRECTORY |...>)>}
                  PROPERTY ${2:<name>} [INHERITED]
                  [BRIEF_DOCS ${3:<brief-doc>} [docs...]]
                  [FULL_DOCS ${4:<full-doc>} [docs...]]
                  [INITIALIZE_FROM_VARIABLE ${5:<variable>}])"#;
    assert_eq!(snippet_result, snippet_target);
}

fn gen_builtin_commands(raw_info: &str) -> Result<Vec<CompletionItem>> {
    let re = regex::Regex::new(r"[a-zA-z]+\n-+").unwrap();
    let keys: Vec<_> = re
        .find_iter(raw_info)
        .map(|message| {
            let temp: Vec<&str> = message.as_str().split('\n').collect();
            temp[0]
        })
        .collect();
    let contents: Vec<_> = re.split(raw_info).collect();
    let contents = &contents[1..].to_vec();

    let mut completes = HashMap::new();
    for (key, content) in keys.iter().zip(contents) {
        let small_key = key.to_lowercase();
        let big_key = key.to_uppercase();
        completes.insert(small_key, content.to_string());
        completes.insert(big_key, content.to_string());
    }
    #[cfg(unix)]
    {
        completes.insert(
            "pkg_check_modules".to_string(),
            "please findpackage PkgConfig first".to_string(),
        );
        completes.insert(
            "PKG_CHECK_MODULES".to_string(),
            "please findpackage PkgConfig first".to_string(),
        );
    }

    let client_support_snippet = to_use_snippet();

    Ok(completes
        .iter()
        .map(|(akey, message)| {
            let mut insert_text_format = InsertTextFormat::PLAIN_TEXT;
            let mut insert_text = akey.to_string();
            let mut detail = "Function".to_string();
            let s = format!(r"\n\s+(?P<signature>{akey}\([^)]*\))");
            let r_match_signature = regex::Regex::new(s.as_str()).unwrap();

            // snippets only work for lower case for now...
            if client_support_snippet
                && insert_text
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '_')
            {
                insert_text = match r_match_signature.captures(message) {
                    Some(m) => {
                        insert_text_format = InsertTextFormat::SNIPPET;
                        detail += " (Snippet)";
                        convert_to_lsp_snippet(m.name("signature").unwrap().as_str())
                    }
                    _ => akey.to_string(),
                }
            };

            CompletionItem {
                label: akey.to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(detail),
                documentation: Some(Documentation::String(message.to_string())),
                insert_text: Some(insert_text),
                insert_text_format: Some(insert_text_format),
                ..Default::default()
            }
        })
        .collect())
}

fn gen_builtin_variables(raw_info: &str) -> Result<Vec<CompletionItem>> {
    let re = regex::Regex::new(r"[z-zA-z]+\n-+").unwrap();
    let key: Vec<_> = re
        .find_iter(raw_info)
        .map(|message| {
            let temp: Vec<&str> = message.as_str().split('\n').collect();
            temp[0]
        })
        .collect();
    let content: Vec<_> = re.split(raw_info).collect();
    let context = &content[1..];
    Ok(zip(key, context)
        .map(|(akey, message)| CompletionItem {
            label: akey.to_string(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some("Variable".to_string()),
            documentation: Some(Documentation::String(message.to_string())),
            ..Default::default()
        })
        .collect())
}

fn gen_builtin_modules(raw_info: &str) -> Result<Vec<CompletionItem>> {
    let re = regex::Regex::new(r"[z-zA-z]+\n-+").unwrap();
    let key: Vec<_> = re
        .find_iter(raw_info)
        .map(|message| {
            let temp: Vec<&str> = message.as_str().split('\n').collect();
            temp[0]
        })
        .collect();
    let content: Vec<_> = re.split(raw_info).collect();
    let context = &content[1..];
    Ok(zip(key, context)
        .map(|(akey, message)| CompletionItem {
            label: akey.to_string(),
            kind: Some(CompletionItemKind::MODULE),
            detail: Some("Module".to_string()),
            documentation: Some(Documentation::String(message.to_string())),
            ..Default::default()
        })
        .collect())
}

/// CMake builtin commands
pub static BUILTIN_COMMAND: LazyLock<Result<Vec<CompletionItem>>> = LazyLock::new(|| {
    let output = Command::new("cmake")
        .arg("--help-commands")
        .output()?
        .stdout;
    let temp = String::from_utf8_lossy(&output);
    gen_builtin_commands(&temp)
});

/// cmake builtin vars
pub static BUILTIN_VARIABLE: LazyLock<Result<Vec<CompletionItem>>> = LazyLock::new(|| {
    let output = Command::new("cmake")
        .arg("--help-variables")
        .output()?
        .stdout;
    let temp = String::from_utf8_lossy(&output);
    gen_builtin_variables(&temp)
});

/// Cmake builtin modules
pub static BUILTIN_MODULE: LazyLock<Result<Vec<CompletionItem>>> = LazyLock::new(|| {
    let output = Command::new("cmake").arg("--help-modules").output()?.stdout;
    let temp = String::from_utf8_lossy(&output);
    gen_builtin_modules(&temp)
});

#[cfg(test)]
mod tests {
    use std::iter::zip;

    use super::gen_builtin_commands;
    use crate::complete::builtin::{gen_builtin_modules, gen_builtin_variables};
    #[test]
    fn tst_regex() {
        let re = regex::Regex::new(r"-+").unwrap();
        assert!(re.is_match("---------"));
        assert!(re.is_match("-------------------"));
        let temp = "javascrpt---------it is";
        let splits: Vec<_> = re.split(temp).collect();
        let aftersplit = vec!["javascrpt", "it is"];
        for (split, after) in zip(splits, aftersplit) {
            assert_eq!(split, after);
        }
    }

    #[test]
    fn tst_cmake_command_builtin() {
        // NOTE: In case the command fails, ignore test
        let output = include_str!("../../assets_for_test/cmake_help_commands.txt");

        let output = gen_builtin_commands(output);

        assert!(output.is_ok());
    }

    #[test]
    fn tst_cmake_variables_builtin() {
        // NOTE: In case the command fails, ignore test
        let output = include_str!("../../assets_for_test/cmake_help_variables.txt");

        let output = gen_builtin_variables(output);

        assert!(output.is_ok());
    }

    #[test]
    fn tst_cmake_modules_builtin() {
        // NOTE: In case the command fails, ignore test
        let output = include_str!("../../assets_for_test/cmake_help_commands.txt");

        let output = gen_builtin_modules(output);

        assert!(output.is_ok());
    }
}
