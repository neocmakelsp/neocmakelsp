pub mod treehelper;
pub mod types;
use anyhow::Result;
use once_cell::sync::Lazy;
use std::collections::HashMap;
pub enum FileType {
    Dir,
    File,
}
impl std::fmt::Display for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileType::Dir => write!(f, "Dir"),
            FileType::File => write!(f, "File"),
        }
    }
}
pub struct CMakePackage {
    pub name: String,
    pub filetype: FileType,
    pub filepath: String,
}
pub static CMAKE_PACKAGES: Lazy<Result<Vec<CMakePackage>>> = Lazy::new(|| {
    let paths = std::fs::read_dir("/usr/lib/cmake/")?;
    Ok(paths
        .into_iter()
        .map(|apath| {
            let message_unit = apath.unwrap();

            let mut filename = message_unit.file_name().to_str().unwrap().to_string();
            let filetype = if message_unit.metadata().unwrap().is_dir() {
                FileType::Dir
            } else {
                filename = filename.split('.').collect::<Vec<&str>>()[0].to_string();
                FileType::File
            };
            let filepath = message_unit.path().to_str().unwrap().to_string();

            CMakePackage {
                name: filename,
                filetype,
                filepath,
            }
        })
        .collect())
});

pub static CMAKE_PACKAGES_WITHKEY: Lazy<Result<HashMap<String, CMakePackage>>> = Lazy::new(|| {
    let mut storage: HashMap<String, CMakePackage> = HashMap::new();
    let paths = std::fs::read_dir("/usr/lib/cmake/")?;
    for apath in paths {
        let message_unit = apath.unwrap();

        let mut filename = message_unit.file_name().to_str().unwrap().to_string();
        let filetype = if message_unit.metadata().unwrap().is_dir() {
            FileType::Dir
        } else {
            filename = filename.split('.').collect::<Vec<&str>>()[0].to_string();
            FileType::File
        };

        let filepath = message_unit.path().to_str().unwrap().to_string();
        storage
            .entry(filename.clone())
            .or_insert_with(|| CMakePackage {
                name: filename,
                filetype,
                filepath,
            });
    }
    Ok(storage)
});
