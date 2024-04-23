/// buildin Commands and vars
use anyhow::Result;
use once_cell::sync::Lazy;
use std::process::Command;
use std::{collections::HashMap, iter::zip};
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind};

/// CMake build in commands
pub static BUILDIN_COMMAND: Lazy<Result<Vec<CompletionItem>>> = Lazy::new(|| {
    let re = regex::Regex::new(r"[z-zA-z]+\n-+").unwrap();
    let output = Command::new("cmake")
        .arg("--help-commands")
        .output()?
        .stdout;
    let temp = String::from_utf8_lossy(&output);
    let keys: Vec<_> = re
        .find_iter(&temp)
        .map(|message| {
            let temp: Vec<&str> = message.as_str().split('\n').collect();
            temp[0]
        })
        .collect();
    let contents: Vec<_> = re.split(&temp).collect();
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
    Ok(completes
        .iter()
        .map(|(akey, message)| CompletionItem {
            label: akey.to_string(),
            kind: Some(CompletionItemKind::MODULE),
            detail: Some(message.to_string()),
            ..Default::default()
        })
        .collect())
});

/// cmake buildin vars
pub static BUILDIN_VARIABLE: Lazy<Result<Vec<CompletionItem>>> = Lazy::new(|| {
    let re = regex::Regex::new(r"[z-zA-z]+\n-+").unwrap();
    let output = Command::new("cmake")
        .arg("--help-variables")
        .output()?
        .stdout;
    let temp = String::from_utf8_lossy(&output);
    let key: Vec<_> = re
        .find_iter(&temp)
        .map(|message| {
            let temp: Vec<&str> = message.as_str().split('\n').collect();
            temp[0]
        })
        .collect();
    let content: Vec<_> = re.split(&temp).collect();
    let context = &content[1..];
    Ok(zip(key, context)
        .map(|(akey, message)| CompletionItem {
            label: akey.to_string(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some(message.to_string()),
            ..Default::default()
        })
        .collect())
});

/// Cmake buildin modules
pub static BUILDIN_MODULE: Lazy<Result<Vec<CompletionItem>>> = Lazy::new(|| {
    let re = regex::Regex::new(r"[z-zA-z]+\n-+").unwrap();
    let output = Command::new("cmake").arg("--help-modules").output()?.stdout;
    let temp = String::from_utf8_lossy(&output);
    let key: Vec<_> = re
        .find_iter(&temp)
        .map(|message| {
            let temp: Vec<&str> = message.as_str().split('\n').collect();
            temp[0]
        })
        .collect();
    let content: Vec<_> = re.split(&temp).collect();
    let context = &content[1..];
    Ok(zip(key, context)
        .map(|(akey, message)| CompletionItem {
            label: akey.to_string(),
            kind: Some(CompletionItemKind::MODULE),
            detail: Some(message.to_string()),
            ..Default::default()
        })
        .collect())
});
#[cfg(test)]
mod tests {
    use std::iter::zip;
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
    use std::process::Command;
    #[test]
    fn tst_cmakecommand_buildin() {
        // NOTE: In case the command fails, ignore test
        let Ok(output) = Command::new("cmake").arg("--help-commands").output() else {
            return;
        };
        let re = regex::Regex::new(r"[z-zA-z]+\n-+").unwrap();
        let output = output.stdout;
        let temp = String::from_utf8_lossy(&output);
        let _key: Vec<_> = re.find_iter(&temp).collect();
        let splits: Vec<_> = re.split(&temp).collect();

        //for akey in key {
        //    println!("{}", akey.as_str());
        //}
        let _newsplit = &splits[1..];
        //for split in newsplit.iter() {
        //    println!("{split}");
        //}
    }
}
