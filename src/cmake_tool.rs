use std::path::PathBuf;

use crate::fileapi::{
    self,
    target::{TARGET_REGEX, Target, TargetType},
};
use dialoguer::{FuzzySelect, theme::ColorfulTheme};

#[derive(Debug, Clone, Copy)]
pub enum HelperKind {
    Run,
    Build,
}

fn get_target_info(target: Option<String>, kind: HelperKind) -> Option<Target> {
    match target {
        Some(name) => fileapi::get_target_data(&name),
        None => {
            let data = fileapi::get_all_targets()?;
            let data_vec: Vec<&Target> = data
                .values()
                .filter(|target| match kind {
                    HelperKind::Build => true,
                    HelperKind::Run => target.info.target_type() == TargetType::Executable,
                })
                .collect();
            let index = FuzzySelect::with_theme(&ColorfulTheme::default())
                .with_prompt("Choose target")
                .default(0)
                .items(&data_vec)
                .interact()
                .ok()?;
            Some(data_vec[index].clone())
        }
    }
}
pub fn prepare_helper(target: Option<String>, dir: PathBuf, kind: HelperKind) -> Option<Target> {
    let build_dir = dir.join("build");
    let cache_path = build_dir
        .join(".cmake")
        .join("api")
        .join("v1")
        .join("reply");
    if cache_path.is_dir() {
        use std::fs;
        if let Ok(entries) = fs::read_dir(cache_path) {
            for entry in entries.flatten() {
                let file_path = entry.path();
                if file_path.is_file() {
                    let Some(file_name) = file_path.file_name() else {
                        continue;
                    };
                    let file_name = file_name.to_string_lossy().to_string();

                    if TARGET_REGEX.is_match(&file_name) {
                        fileapi::update_target_data(file_path);
                    }
                }
            }
        }
    }
    get_target_info(target, kind)
}
pub fn help_build(target: Option<String>, dir: PathBuf) -> anyhow::Result<()> {
    let Some(target_info) = prepare_helper(target, dir, HelperKind::Build) else {
        eprintln!("cannot find target");
        return Ok(());
    };

    let mut command = std::process::Command::new("cmake")
        .arg("--build")
        .arg("build")
        .arg("--target")
        .arg(target_info.name)
        .spawn()?;
    command.wait()?;
    Ok(())
}

pub fn help_run(target: Option<String>, dir: PathBuf, args: Vec<String>) -> anyhow::Result<()> {
    let build_dir = dir.join("build");
    let Some(target_info) = prepare_helper(target, dir, HelperKind::Run) else {
        eprintln!("cannot find target");
        return Ok(());
    };

    let Some(artifact) = target_info.info.artifacts().first() else {
        eprintln!("target does not contain a runnable path");
        return Ok(());
    };
    let target_path = build_dir.join(&artifact.path);
    let mut command = std::process::Command::new(target_path).args(args).spawn()?;
    command.wait()?;
    Ok(())
}
