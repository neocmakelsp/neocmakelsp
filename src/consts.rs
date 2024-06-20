use once_cell::sync::Lazy;

pub static TREESITTER_CMAKE_LANGUAGE: Lazy<tree_sitter::Language> =
    Lazy::new(tree_sitter_cmake::language);
