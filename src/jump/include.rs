use std::path::Path;

use lsp_types::Uri;
use tower_lsp::lsp_types;

use super::{Location, gen_module_pattern};
use crate::utils::include_is_module;

pub(super) fn cmpinclude<P: AsRef<Path>>(localpath: P, subpath: &str) -> Option<Vec<Location>> {
    let target = if !include_is_module(subpath) {
        let root_dir = localpath.as_ref().parent()?;
        root_dir.join(subpath)
    } else {
        let glob_pattern = gen_module_pattern(subpath)?;
        glob::glob(glob_pattern.as_str())
            .into_iter()
            .flatten()
            .flatten()
            .next()?
    };

    if target.exists() {
        Some(vec![Location {
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
            },
            uri: Uri::from_file_path(target).unwrap(),
        }])
    } else {
        None
    }
}
