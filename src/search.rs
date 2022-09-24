use crate::utils::CMAKE_PACKAGES;
use cli_table::{format::Justify, Cell, CellStruct, Style, Table};
pub fn search_result(tosearch: &str) -> cli_table::TableDisplay {
    let tofind = regex::Regex::new(&tosearch).unwrap();
    CMAKE_PACKAGES
        .iter()
        .filter(|source| tofind.is_match(&source.name))
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
