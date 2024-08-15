use treesitter_kind_collector::tree_sitter_consts;

#[tree_sitter_consts("misc/node-types.json")]
struct TmpCollector;

pub use TmpCollector::*;
