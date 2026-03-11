use std::collections::HashMap;
use std::iter::zip;
use std::process::Command;
use std::sync::LazyLock;

use anyhow::Result;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, Documentation, InsertTextFormat, MarkupContent, MarkupKind,
};

use crate::languageserver::to_use_snippet;

// static SPIT_PRAMETER_REGEX: LazyLock<regex::Regex> =
//     LazyLock::new(|| regex::Regex::new(r"<[^>]+>\.*").unwrap());
// // (<[^>]+>(?:\.\.\.)?)|(\.{3})|(\[.*?\])|([A-Z][A-Z_]+)

// As regex can't resolve nested parameter struct, parse it manually
fn split_parameters(command_label: &str) -> Vec<&str> {
    let _command_label = command_label.trim();
    todo!()
}

fn gen_builtin_command_signature_resource(
    raw_document: &str,
) -> HashMap<String, CommandSignatureResource<'_>> {
    // WARN: This regex is directly copied from the original gen_builtin_commands()
    // But is might be wrong. Cause [A-z] contains [a-z]
    // And this also contains [ \ ] ^ _ `
    let re = regex::Regex::new(r"[a-zA-z]+\n-+").unwrap();
    let keys: Vec<_> = re
        .find_iter(raw_document)
        .map(|message| {
            let temp: Vec<&str> = message.as_str().split('\n').collect();
            temp[0]
        })
        .collect();
    let contents: Vec<_> = re.split(raw_document).skip(1).collect();
    keys.iter()
        .zip(contents)
        .flat_map(|(&key, content)| {
            let r_match_signature = regex::Regex::new(
                format!(r"\n\s+(?P<signature>{}\((?P<parameters>[^)]*)\))", key).as_str(),
            )
            .unwrap();
            r_match_signature
                .captures_iter(content)
                .map(|capture| {
                    let signature = capture.name("signature").unwrap().as_str();
                    let raw_parameters = capture.name("parameters").unwrap().as_str();
                    let parameters = split_parameters(raw_parameters);
                    (
                        key.to_string(),
                        CommandSignatureResource::new(signature, parameters, content.trim()),
                    )
                })
                .collect::<Vec<(String, CommandSignatureResource)>>()
        })
        .chain({
            #[cfg(unix)]
            [(
                "pkg_check_modules".to_string(),
                CommandSignatureResource::new(
                    "pkg_check_modules()",
                    vec![],
                    "please findpackage PkgConfig first",
                ),
            )]
        })
        .collect()
}

fn gen_builtin_commands() -> Result<Vec<CompletionItem>> {
    let res = (BUILTIN_COMMAND_SIGNATURE_RES.as_ref()).unwrap();
    let client_support_snippet = to_use_snippet();

    Ok(res
        .iter()
        .flat_map(|(name, commandinfo)| {
            let insert_text_format;
            let detail;
            let insert_text;

            if client_support_snippet {
                detail = "Function (Snippet)";
                insert_text_format = InsertTextFormat::SNIPPET;
                insert_text = format!(
                    "{name}({})",
                    commandinfo
                        .parameters
                        .iter()
                        .enumerate()
                        .map(|(i, s)| format!("${{{i}:{s}}}"))
                        .collect::<Vec<String>>()
                        .join(" ")
                );
            } else {
                detail = "Function";
                insert_text_format = InsertTextFormat::PLAIN_TEXT;
                insert_text = name.clone() + "()";
            }

            [
                CompletionItem {
                    label: commandinfo.signature.to_lowercase(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(detail.to_string()),
                    documentation: commandinfo.gen_document(),
                    insert_text: Some(insert_text.clone()),
                    insert_text_format: Some(insert_text_format),
                    ..Default::default()
                },
                CompletionItem {
                    label: commandinfo.signature.to_uppercase(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(detail.to_string()),
                    documentation: commandinfo.gen_document(),
                    insert_text: Some(insert_text),
                    insert_text_format: Some(insert_text_format),
                    ..Default::default()
                },
            ]
        })
        .collect())
}

fn gen_builtin_variables(raw_info: &str) -> Result<Vec<CompletionItem>> {
    // WARN: same problem as the regex above
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
            documentation: Some(Documentation::String(message.trim().to_string())),
            ..Default::default()
        })
        .collect())
}

fn gen_builtin_modules(raw_info: &str) -> Result<Vec<CompletionItem>> {
    // WARN: same problem as the regex above
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
            documentation: Some(Documentation::String(message.trim().to_string())),
            ..Default::default()
        })
        .collect())
}

pub struct CommandSignatureResource<'a> {
    signature: &'a str,
    parameters: Vec<&'a str>,
    // document: Option<Documentation>,
    raw_doc: &'a str,
}

impl<'a> CommandSignatureResource<'a> {
    fn new(signature: &'a str, parameters: Vec<&'a str>, raw_doc: &'a str) -> Self {
        CommandSignatureResource {
            signature,
            parameters,
            raw_doc,
        }
    }

    fn gen_document(&self) -> Option<Documentation> {
        if self.raw_doc.is_empty() {
            Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: self.raw_doc.to_string(),
            }))
        } else {
            None
        }
    }
}

static CMAKE_COMMANDS_HELP: LazyLock<Result<String>> = LazyLock::new(|| {
    let output = Command::new("cmake")
        .arg("--help-commands")
        .output()?
        .stdout;
    Ok(String::from_utf8_lossy(&output).to_string())
});
/// Resource for generating builtin signatures and commands
/// the key is command name, not signature
pub static BUILTIN_COMMAND_SIGNATURE_RES: LazyLock<
    Result<HashMap<String, CommandSignatureResource>>,
> = LazyLock::new(|| {
    Ok(gen_builtin_command_signature_resource(
        CMAKE_COMMANDS_HELP.as_ref().unwrap(),
    ))
});

/// CMake builtin commands
pub static BUILTIN_COMMAND: LazyLock<Result<Vec<CompletionItem>>> =
    LazyLock::new(gen_builtin_commands);

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

    use super::*;
    use crate::complete::builtin::{gen_builtin_modules, gen_builtin_variables};

    #[test]
    fn test_regex() {
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
    fn test_cmake_command_builtin() {
        // NOTE: In case the command fails, ignore test
        // let output = include_str!("../../assets_for_test/cmake_help_commands.txt");

        let output = gen_builtin_commands();
        // let output = gen_builtin_commands(output);

        assert!(output.is_ok());
    }

    #[test]
    fn test_cmake_variables_builtin() {
        // NOTE: In case the command fails, ignore test
        let output = include_str!("../../assets_for_test/cmake_help_variables.txt");

        let output = gen_builtin_variables(output);

        assert!(output.is_ok());
    }

    #[test]
    fn test_cmake_modules_builtin() {
        // NOTE: In case the command fails, ignore test
        let output = include_str!("../../assets_for_test/cmake_help_commands.txt");

        let output = gen_builtin_modules(output);

        assert!(output.is_ok());
    }
}
