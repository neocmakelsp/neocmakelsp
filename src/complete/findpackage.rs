use once_cell::sync::Lazy;
static CMAKE_SOURCE: Lazy<Vec<String>> = Lazy::new(|| {
    vec![
        "/usr/lib/cmake".to_string(),
        "/usr/local/lib/cmake".to_string(),
        "/usr/lib/x86_64-linux-gnu".to_string(),
    ]
});
