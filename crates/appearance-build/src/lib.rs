use std::path::{Path, PathBuf};

use appearance_asset_database::asset_paths::CRATES;
use hassle_rs::compile_hlsl;
pub use xshell::Shell;

const ASSET_DIR_IGNORE: &[&str] = &["target", ".gitignore", ".vscode"];

fn find_asset_dirs(shell: &Shell, dir: &PathBuf, asset_dirs: &mut Vec<PathBuf>) {
    let files = shell.read_dir(dir).unwrap();

    for file in &files {
        if file.is_dir() {
            if ASSET_DIR_IGNORE.contains(&file.file_name().unwrap().to_str().unwrap()) {
                continue;
            }

            if file.file_name().unwrap() == "assets" {
                asset_dirs.push(file.to_owned());
            } else {
                find_asset_dirs(shell, file, asset_dirs);
            }
        }
    }
}

fn copy_assets(shell: &Shell, root_dir: &Path, dir: &PathBuf, target_dir: &PathBuf) {
    let files = shell.read_dir(dir).unwrap();

    for file in &files {
        if !file.is_dir() {
            let local_dir = file.strip_prefix(root_dir).unwrap();
            let dst = target_dir.join(local_dir);
            shell.create_dir(dst.parent().unwrap()).unwrap();

            let mut is_shader_file = false;

            let file_name = file.file_name().unwrap().to_str().unwrap();
            if file_name.ends_with(".cs.hlsl")
                || file_name.ends_with(".vs.hlsl")
                || file_name.ends_with(".fs.hlsl")
            {
                let target_profile = if file_name.ends_with(".cs.hlsl") {
                    "cs_6_5"
                } else if file_name.ends_with(".vs.hlsl") {
                    "vs_6_5"
                } else if file_name.ends_with(".fs.hlsl") {
                    "ps_6_5"
                } else {
                    panic!()
                };

                let src = std::fs::read_to_string(file).expect("Invalid shader name.");

                let mut include_dirs: Vec<String> = CRATES
                    .iter()
                    .map(|crate_name| format!("-I crates/{}/assets/shaders", crate_name))
                    .collect();

                let parent_dir = file.parent().unwrap().to_str().unwrap();
                include_dirs.push(format!("-I {}", parent_dir));

                let mut args = vec!["-spirv", "-WX"];
                for include_dir in &include_dirs {
                    args.push(include_dir);
                }

                let spirv = match compile_hlsl(file_name, &src, "main", target_profile, &args, &[])
                {
                    Ok(spirv) => spirv,
                    Err(e) => {
                        println!("cargo::error={}", e);
                        panic!();
                    }
                };

                std::fs::write(&dst, spirv).unwrap();
                is_shader_file = true;
            }

            if !is_shader_file {
                shell.copy_file(file, dst.clone()).unwrap();
            }
        } else {
            copy_assets(shell, root_dir, file, target_dir);
        }
    }
}

pub fn build(manifest_dir: &str) {
    println!("cargo:rerun-if-changed=NULL");

    let shell = Shell::new().unwrap();
    let root_dir = Path::new(manifest_dir).parent().unwrap().parent().unwrap();
    shell.change_dir(root_dir);
    std::env::set_current_dir(root_dir).unwrap();

    let parent_dir = Path::new(manifest_dir)
        .parent()
        .unwrap()
        .file_name()
        .unwrap();
    let is_app = parent_dir != "crates";

    if is_app {
        let mut asset_dirs = vec![];
        find_asset_dirs(&shell, &shell.current_dir(), &mut asset_dirs);

        let target_dir = root_dir.join(Path::new(&format!(
            "target/{}/assets",
            std::env::var("PROFILE").unwrap()
        )));
        for asset_dir in &asset_dirs {
            copy_assets(&shell, root_dir, asset_dir, &target_dir);
        }
    }
}
