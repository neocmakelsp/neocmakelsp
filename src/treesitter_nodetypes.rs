use treesitter_kind_collector::tree_sitter_kinds;

#[tree_sitter_kinds("misc/node-types.json")]
struct TmpCollector;

pub use TmpCollector::*;
