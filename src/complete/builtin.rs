use std::collections::HashMap;
use std::iter::zip;
use std::process::Command;
use std::sync::LazyLock;

use anyhow::Result;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, Documentation, InsertTextFormat, MarkupContent, MarkupKind,
    ParameterInformation, ParameterLabel,
};

use crate::languageserver::to_use_snippet;

// As regex can't resolve nested parameter struct, parse it manually
fn split_parameters(raw_parameters_string: &str) -> Vec<&str> {
    let raw_parameters_string = raw_parameters_string.trim();
    let parameters_char_vec: Vec<char> = raw_parameters_string.chars().collect();
    let mut i = 0;
    let mut result = Vec::new();
    while i < parameters_char_vec.len() {
        let para_begin = i;
        match parameters_char_vec[i] {
            '.' => {
                while let Some('.') = parameters_char_vec.get(i + 1) {
                    i += 1;
                }
            }
            bracket @ ('<' | '[' | '{') => {
                let mut bracket_num = 1;
                let opposite_bracket = match bracket {
                    '<' => '>',
                    '[' => ']',
                    _ => '}',
                };
                while let Some(c) = parameters_char_vec.get(i + 1) {
                    i += 1;
                    match (c, bracket_num) {
                        (x, _) if *x == bracket => bracket_num += 1,
                        (x, 1) if *x == opposite_bracket => break,
                        (x, _) if *x == opposite_bracket => bracket_num -= 1,
                        _ => (),
                    }
                }
                while let Some('.') = parameters_char_vec.get(i + 1) {
                    i += 1;
                }
            }
            'A'..='z' => {
                while let Some(c) = parameters_char_vec.get(i + 1) {
                    if ('A'..='z').contains(c) {
                        i += 1;
                    } else {
                        break;
                    }
                }
            }
            // handle comments
            '#' => {
                while let Some(c) = parameters_char_vec.get(i + 1) {
                    i += 1;
                    if *c == '\n' {
                        break;
                    }
                }
                i += 1;
                continue;
            }
            _ => {
                i += 1;
                continue;
            }
        }
        if i >= parameters_char_vec.len() {
            result.push(&raw_parameters_string[para_begin..parameters_char_vec.len()]);
            break;
        } else {
            result.push(&raw_parameters_string[para_begin..=i]);
            i += 1;
        }
    }
    result
}

fn gen_builtin_command_signature_resource(
    raw_document: &str,
) -> HashMap<&str, CommandSignatureResource<'_>> {
    let re = regex::Regex::new(r"[a-zA-Z_]+\r?\n-+").unwrap();
    let keys: Vec<_> = re
        .find_iter(raw_document)
        .map(|message| {
            let temp: Vec<&str> = message.as_str().lines().map(|m| m.trim()).collect();
            temp[0]
        })
        .collect();
    let contents: Vec<_> = re.split(raw_document).skip(1).collect();
    let temp_iter = keys.iter().zip(contents).filter_map(|(&key, content)| {
        let r_match_signature = regex::Regex::new(
            format!(r"\n\s+(?P<signature>{}\((?P<parameters>[^)]*)\))", key).as_str(),
        )
        .unwrap();
        let capture = r_match_signature.captures(content)?;
        let signature = capture.name("signature").unwrap().as_str();
        let raw_parameters = capture.name("parameters").unwrap().as_str();
        let parameters = split_parameters(raw_parameters);
        Some((
            key,
            CommandSignatureResource::new(signature, parameters, content.trim()),
        ))
    });

    #[cfg(unix)]
    return temp_iter
        .chain({
            [(
                "pkg_check_modules",
                CommandSignatureResource::new(
                    "pkg_check_modules()",
                    vec![],
                    "please findpackage PkgConfig first",
                ),
            )]
        })
        .collect();
    #[cfg(windows)]
    return temp_iter.collect();
}

fn gen_builtin_commands() -> Result<Vec<CompletionItem>> {
    let res = &*BUILTIN_COMMAND_SIGNATURE_RES;
    let client_support_snippet = to_use_snippet();

    Ok(res
        .iter()
        .flat_map(|(name, commandinfo)| {
            let insert_text_format;
            let detail;
            let insert_text;
            let uppercase_insert_text;

            if client_support_snippet {
                detail = "Function (Snippet)";
                insert_text_format = InsertTextFormat::SNIPPET;
                let para = commandinfo
                    .parameters
                    .iter()
                    .enumerate()
                    .map(|(i, s)| format!("${{{}:{s}}}", i + 1))
                    .collect::<Vec<String>>()
                    .join(" ");
                insert_text = format!("{name}({})", para);
                uppercase_insert_text = format!("{}({})", name.to_uppercase(), para);
            } else {
                detail = "Function";
                insert_text_format = InsertTextFormat::PLAIN_TEXT;
                insert_text = name.to_string();
                uppercase_insert_text = name.to_uppercase();
            }

            [
                CompletionItem {
                    label: name.to_lowercase(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(detail.to_string()),
                    documentation: commandinfo.gen_document(),
                    insert_text: Some(insert_text),
                    insert_text_format: Some(insert_text_format),
                    ..Default::default()
                },
                CompletionItem {
                    label: name.to_uppercase(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(detail.to_string()),
                    documentation: commandinfo.gen_document(),
                    insert_text: Some(uppercase_insert_text),
                    insert_text_format: Some(insert_text_format),
                    ..Default::default()
                },
            ]
        })
        .collect())
}

fn gen_builtin_variables(raw_info: &str) -> Vec<CompletionItem> {
    let re = regex::Regex::new(r"((?:[A-Z_]|<LANG>)+)\r?\n-+").unwrap();
    let mut key_iter = re.captures_iter(raw_info).peekable();
    let mut result = Vec::<CompletionItem>::new();

    let mut push_item = |label: String, doc: &str| {
        result.push(CompletionItem {
            label,
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some("Variable".to_string()),
            documentation: Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: doc.trim().to_string(),
            })),
            ..Default::default()
        });
    };

    while let Some(key) = key_iter.next()
        && let Some(variable_name) = key.get(1)
    {
        let variable_name = variable_name.as_str();

        let next_start = key_iter
            .peek()
            .map(|m| m.get(0).unwrap().start())
            .unwrap_or_else(|| raw_info.len());
        let doc = &raw_info[key.get(0).unwrap().end()..next_start];

        if variable_name.contains("<LANG>") {
            push_item(variable_name.replace("<LANG>", "CXX"), doc);
            push_item(variable_name.replace("<LANG>", "C"), doc);
        } else {
            push_item(variable_name.to_string(), doc);
        }
    }
    result
}

fn gen_builtin_modules(raw_info: &str) -> Result<Vec<CompletionItem>> {
    let re = regex::Regex::new(r"[a-zA-Z_]+\r?\n-+").unwrap();
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
    pub signature: &'a str,
    pub parameters: Vec<&'a str>,
    // document: Option<Documentation>,
    pub raw_doc: &'a str,
}

impl<'a> CommandSignatureResource<'a> {
    const fn new(signature: &'a str, parameters: Vec<&'a str>, raw_doc: &'a str) -> Self {
        CommandSignatureResource {
            signature,
            parameters,
            raw_doc,
        }
    }

    pub fn gen_document(&self) -> Option<Documentation> {
        if self.raw_doc.is_empty() {
            None
        } else {
            Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: self.raw_doc.to_string(),
            }))
        }
    }

    pub fn gen_parameters(&self) -> Option<Vec<ParameterInformation>> {
        if self.parameters.is_empty() {
            None
        } else {
            Some(
                self.parameters
                    .iter()
                    .map(|&para| ParameterInformation {
                        label: ParameterLabel::Simple(para.to_string()),
                        documentation: None,
                    })
                    .collect(),
            )
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
pub static BUILTIN_COMMAND_SIGNATURE_RES: LazyLock<HashMap<&str, CommandSignatureResource>> =
    LazyLock::new(|| gen_builtin_command_signature_resource(CMAKE_COMMANDS_HELP.as_ref().unwrap()));

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
    Ok(gen_builtin_variables(&temp))
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

    #[cfg(not(windows))]
    const NEW_LINE: &str = "\n";
    #[cfg(windows)]
    const NEW_LINE: &str = "\r\n";
    #[test]
    fn test_split_parameters() {
        // test1 (parameters of add_executable)
        let raw_parameters = r"<name> <options>... <sources>...";
        let result = split_parameters(raw_parameters);
        assert_eq!(result, vec!["<name>", "<options>...", "<sources>..."]);

        // test2 (parameters of set_property)
        let raw_parameters = r"<GLOBAL                      |
               DIRECTORY [<dir>]           |
               TARGET    [<target1> ...]   |
               SOURCE    [<src1> ...]
                         [DIRECTORY <dirs> ...]
                         [TARGET_DIRECTORY <targets> ...] |
               INSTALL   [<file1> ...]     |
               TEST      [<test1> ...]
                         [DIRECTORY <dir>] |
               CACHE     [<entry1> ...]    >
              [APPEND] [APPEND_STRING]
              PROPERTY <name> [<value1> ...]";
        let result = split_parameters(raw_parameters);
        assert_eq!(
            result,
            vec![
                r"<GLOBAL                      |
               DIRECTORY [<dir>]           |
               TARGET    [<target1> ...]   |
               SOURCE    [<src1> ...]
                         [DIRECTORY <dirs> ...]
                         [TARGET_DIRECTORY <targets> ...] |
               INSTALL   [<file1> ...]     |
               TEST      [<test1> ...]
                         [DIRECTORY <dir>] |
               CACHE     [<entry1> ...]    >",
                "[APPEND]",
                "[APPEND_STRING]",
                "PROPERTY",
                "<name>",
                "[<value1> ...]"
            ]
        );

        // test something with comments
        let raw_parameters = r"<variable>
               [CONFIGURATION <config>]
               [PARALLEL_LEVEL <parallel>]
               [TARGET <target>]
               [PROJECT_NAME <projname>] # legacy, causes warning
              ";
        let result = split_parameters(raw_parameters);
        assert_eq!(
            result,
            vec![
                "<variable>",
                "[CONFIGURATION <config>]",
                "[PARALLEL_LEVEL <parallel>]",
                "[TARGET <target>]",
                "[PROJECT_NAME <projname>]"
            ]
        );
    }

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
        assert!(output.is_ok());
    }

    #[test]
    fn test_gen_builtin_command_signature_resource() {
        let res = gen_builtin_command_signature_resource(include_str!(
            "../../assets_for_test/cmake_help_commands.txt"
        ));
        let tested_command = res.get("set_property").unwrap();
        println!(
            "{}\n\n\n{}\n\n\n{}",
            tested_command.signature,
            tested_command.parameters.join(NEW_LINE),
            tested_command.raw_doc
        );
        #[cfg(not(windows))]
        assert_eq!(
            tested_command.signature,
            r"set_property({GLOBAL                                    |
               DIRECTORY [<dir>]                         |
               TARGET    <target>...                     |
               FILE_SET  <file_set>... TARGET <target>   |
               SOURCE    <source>...
                         [DIRECTORY <dirs> ...]
                         [TARGET_DIRECTORY <targets>...] |
               INSTALL   <file>...                       |
               TEST      <test>...
                         [DIRECTORY <dir>]               |
               CACHE     <entry>...}
              [APPEND] [APPEND_STRING]
              PROPERTY <name> [<value>...])"
        );
        #[cfg(not(windows))]
        let temp = [
            r"{GLOBAL                                    |
               DIRECTORY [<dir>]                         |
               TARGET    <target>...                     |
               FILE_SET  <file_set>... TARGET <target>   |
               SOURCE    <source>...
                         [DIRECTORY <dirs> ...]
                         [TARGET_DIRECTORY <targets>...] |
               INSTALL   <file>...                       |
               TEST      <test>...
                         [DIRECTORY <dir>]               |
               CACHE     <entry>...}",
            "[APPEND]",
            "[APPEND_STRING]",
            "PROPERTY",
            "<name>",
            "[<value>...]",
        ];
        #[cfg(not(windows))]
        for (i, &item) in tested_command.parameters.iter().enumerate() {
            assert_eq!(item, temp[i]);
        }
    }

    #[test]
    fn test_cmake_variables_builtin() {
        let raw_doc = r"
CMAKE_COMMAND
-------------

The full path to the ``cmake(1)`` executable.

CMAKE_<LANG>_COMPILER
---------------------

The full path to the compiler for ``LANG``.

This is the command that will be used as the ``<LANG>`` compiler.  Once
set, you can not change this variable.
";
        let output = gen_builtin_variables(raw_doc);
        assert_eq!(output[0].label, "CMAKE_COMMAND");
        let Documentation::MarkupContent(temp) = output[0].documentation.as_ref().unwrap() else {
            panic!();
        };
        assert_eq!(
            temp.value,
            "The full path to the ``cmake(1)`` executable.".to_string()
        );

        let Documentation::MarkupContent(temp) = output[1].documentation.as_ref().unwrap() else {
            panic!();
        };
        assert_eq!(output[1].label, "CMAKE_CXX_COMPILER");
        assert_eq!(
            temp.value,
            r"The full path to the compiler for ``LANG``.

This is the command that will be used as the ``<LANG>`` compiler.  Once
set, you can not change this variable."
                .to_string()
        );
        assert_eq!(output[2].label, "CMAKE_C_COMPILER");
    }

    #[test]
    fn test_cmake_modules_builtin() {
        // NOTE: In case the command fails, ignore test
        let output = include_str!("../../assets_for_test/cmake_help_commands.txt");

        let output = gen_builtin_modules(output);

        assert!(output.is_ok());
    }
}
