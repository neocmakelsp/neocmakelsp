use std::sync::LazyLock;

pub static TREESITTER_CMAKE_LANGUAGE: LazyLock<tree_sitter::Language> =
    LazyLock::new(|| tree_sitter_cmake::LANGUAGE.into());
