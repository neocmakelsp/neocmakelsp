use crate::utils::{CMakePackage, CACHE_CMAKE_PACKAGES};
use cli_table::{format::Justify, Cell, CellStruct, Style, Table};
pub fn search_result(tosearch: &str) -> cli_table::TableDisplay {
    let tofind = regex::Regex::new(&tosearch.to_lowercase()).unwrap();
    CACHE_CMAKE_PACKAGES
        .iter()
        .filter(|source| tofind.is_match(&source.name.to_lowercase()))
        .map(|source| match &source.version {
            Some(version) => vec![
                source.name.clone().cell(),
                source.filepath.clone().cell().justify(Justify::Left),
                version.cell().justify(Justify::Left),
            ],
            None => vec![
                source.name.clone().cell(),
                source.filepath.clone().cell().justify(Justify::Left),
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
        .display()
        .unwrap()
}

pub fn search_result_tojson(tosearch: &str) -> String {
    let tofind = regex::Regex::new(&tosearch.to_lowercase()).unwrap();
    let output: Vec<CMakePackage> = CACHE_CMAKE_PACKAGES
        .iter()
        .filter(|source| tofind.is_match(&source.name.to_lowercase()))
        .cloned()
        .collect();
    serde_json::to_string(&output).unwrap()
}
