use anyhow::Result;
use cli_table::format::Justify;
use cli_table::{Cell, CellStruct, Style, Table};
use regex::Regex;

use crate::utils::{CACHE_CMAKE_PACKAGES, CMakePackage};

pub fn search_result(tosearch: &str) -> Result<cli_table::TableDisplay> {
    let tofind = Regex::new(&tosearch.to_lowercase())?;
    Ok(CACHE_CMAKE_PACKAGES
        .iter()
        .filter(|source| tofind.is_match(&source.name.to_lowercase()))
        .map(|source| match &source.version {
            Some(version) => vec![
                source.name.clone().cell(),
                source.location.path().cell().justify(Justify::Left),
                version.cell().justify(Justify::Left),
            ],
            None => vec![
                source.name.clone().cell(),
                source.location.path().cell().justify(Justify::Left),
                "Unknown".cell().justify(Justify::Left),
            ],
        })
        .collect::<Vec<Vec<CellStruct>>>()
        .table()
        .title(vec![
            "PackageName".cell().justify(Justify::Left).bold(true),
            "Location".cell().justify(Justify::Center).bold(true),
            "Version".cell().justify(Justify::Center).bold(true),
        ])
        .bold(true)
        .display()?)
}

pub fn search_result_tojson(tosearch: &str) -> Result<String> {
    let tofind = Regex::new(&tosearch.to_lowercase())?;
    let output: Vec<CMakePackage> = CACHE_CMAKE_PACKAGES
        .iter()
        .filter(|source| tofind.is_match(&source.name.to_lowercase()))
        .cloned()
        .collect();
    Ok(serde_json::to_string(&output)?)
}

#[cfg(test)]
mod search_test {
    use super::*;
    use crate::utils::CACHE_CMAKE_PACKAGES_WITHKEYS;

    #[test]
    fn search_result_test_1() {
        let search_result = search_result_tojson("bash").unwrap();
        let data = CACHE_CMAKE_PACKAGES_WITHKEYS
            .get("bash-completion-fake")
            .unwrap();
        let result_json = serde_json::to_string(&vec![data]).unwrap();
        assert_eq!(search_result, result_json);
    }

    #[test]
    fn search_result_test_2() {
        let search_result = search_result_tojson("zzzz").unwrap();
        let result_json = r#"[]"#;
        assert_eq!(search_result, result_json);
    }

    #[test]
    fn search_cli_pass_test() -> Result<()> {
        search_result("bash")?;
        search_result("ttt")?;
        search_result("eee")?;
        Ok(())
    }
}
