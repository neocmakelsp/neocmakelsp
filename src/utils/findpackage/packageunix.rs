// here is the logic of findpackage on linux
//
pub const PREFIX: [&str; 2] = ["/usr", "/usr/local"];
pub const ARCH: [&str; 1] = ["x86_64-linux-gnu"];
pub const SHARE: &str = "share";
pub const LIBS: [&str; 3] = ["lib", "lib32", "lib64"];
fn temp() {
    for prefix in PREFIX {
        for lib in LIBS {
            if let Ok(paths) = std::fs::read_dir(format!("{}/{}", prefix, lib)) {
                for path in paths {
                    if let Ok(pathunit) = path {
                        if pathunit.metadata().unwrap().is_dir() {

                        }
                    }
                }
            }
        }
    }
}
