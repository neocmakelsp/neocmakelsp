use anyhow::Result;
use lsp_types::{CompletionItem, CompletionItemKind};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::iter::zip;
use std::process::Command;
pub static BUILDIN_COMMAND: Lazy<Result<Vec<CompletionItem>>> = Lazy::new(|| {
    let re = regex::Regex::new(r"[z-zA-z]+\n-+").unwrap();
    let output = Command::new("cmake")
        .arg("--help-commands")
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
    let content: Vec<_> = re.split(&temp).into_iter().collect();
    let context = &content[1..];
    Ok(zip(key, context)
        .into_iter()
        .map(|(akey, message)| CompletionItem {
            label: akey.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(message.to_string()),
            ..Default::default()
        })
        .collect())
});
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
    let content: Vec<_> = re.split(&temp).into_iter().collect();
    let context = &content[1..];
    Ok(zip(key, context)
        .into_iter()
        .map(|(akey, message)| CompletionItem {
            label: akey.to_string(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some(message.to_string()),
            ..Default::default()
        })
        .collect())
});
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
    let content: Vec<_> = re.split(&temp).into_iter().collect();
    let context = &content[1..];
    Ok(zip(key, context)
        .into_iter()
        .map(|(akey, message)| CompletionItem {
            label: akey.to_string(),
            kind: Some(CompletionItemKind::MODULE),
            detail: Some(message.to_string()),
            ..Default::default()
        })
        .collect())
});
#[allow(unused)]
pub static MESSAGE_STORAGE: Lazy<HashMap<String, String>> = Lazy::new(|| {
    let mut storage : HashMap<String, String> = HashMap::new();
    let re = regex::Regex::new(r"[z-zA-z]+\n-+").unwrap();
    if let Ok(output) = Command::new("cmake").arg("--help-commands").output() {
        let output = output.stdout;
        let temp = String::from_utf8_lossy(&output);
        let key: Vec<_> = re
            .find_iter(&temp)
            .map(|message| {
                let temp: Vec<&str> = message.as_str().split('\n').collect();
                temp[0]
            })
            .collect();
        let content: Vec<_> = re.split(&temp).into_iter().collect();
        let context = &content[1..];
        for (akey, message) in zip(key, context) {
            storage.entry(akey.to_string()).or_insert(message.to_string());
        }
    }
    if let Ok(output) = Command::new("cmake").arg("--help-variables").output() {
        let output = output.stdout;
        let temp = String::from_utf8_lossy(&output);
        let key: Vec<_> = re
            .find_iter(&temp)
            .map(|message| {
                let temp: Vec<&str> = message.as_str().split('\n').collect();
                temp[0]
            })
            .collect();
        let content: Vec<_> = re.split(&temp).into_iter().collect();
        let context = &content[1..];
        for (akey, message) in zip(key, context) {
            storage.entry(akey.to_string()).or_insert(message.to_string());
        }
    }
    if let Ok(output) = Command::new("cmake").arg("--help-modules").output() {
        let output = output.stdout;
        let temp = String::from_utf8_lossy(&output);
        let key: Vec<_> = re
            .find_iter(&temp)
            .map(|message| {
                let temp: Vec<&str> = message.as_str().split('\n').collect();
                temp[0]
            })
            .collect();
        let content: Vec<_> = re.split(&temp).into_iter().collect();
        let context = &content[1..];
        for (akey, message) in zip(key, context) {
            storage.entry(akey.to_string()).or_insert(message.to_string());
        }
    }
    storage
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
        let splits: Vec<_> = re.split(temp).into_iter().collect();
        let aftersplit = vec!["javascrpt", "it is"];
        for (split, after) in zip(splits, aftersplit) {
            assert_eq!(split, after);
        }
    }
    use std::process::Command;
    #[test]
    fn tst_cmakecommand_buildin() {
        let re = regex::Regex::new(r"[z-zA-z]+\n-+").unwrap();
        let output = Command::new("cmake")
            .arg("--help-commands")
            .output()
            .unwrap()
            .stdout;
        let temp = String::from_utf8_lossy(&output);
        let key: Vec<_> = re.find_iter(&temp).collect();
        let splits: Vec<_> = re.split(&temp).into_iter().collect();

        for akey in key {
            println!("{}", akey.as_str());
        }
        let newsplit = &splits[1..];
        for split in newsplit.iter() {
            println!("{split}");
        }
    }
}
