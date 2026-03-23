use std::fs;
use std::path::Path;

use crate::generate_systems;
use crate::models::System;

/// Create ROM directory for a system, optionally copying from template.
/// Python: createFolders()
pub fn create_folders(system: &str, data: &System, source: &Path, target: &Path) {
    let subdir = match generate_systems::system_sub_roms_dir(system, data) {
        Some(s) => s,
        None => return,
    };

    let target_dir = target.join(&subdir);
    let source_dir = source.join(&subdir);

    if !target_dir.exists() {
        if source_dir.is_dir() {
            // Copy the template directory
            copy_dir_recursive(&source_dir, &target_dir);
        } else {
            fs::create_dir_all(&target_dir).ok();
        }
    }
}

/// Write _info.txt for a system in its ROM directory.
/// Python: infoSystem()
pub fn write_info(system: &str, data: &System, roms_dir: &Path) {
    let subdir = match generate_systems::system_sub_roms_dir(system, data) {
        Some(s) => s,
        None => return,
    };

    let mut info = String::new();
    info.push_str(&format!("## SYSTEM {} ##\n", data.name.to_uppercase()));
    info.push_str("-------------------------------------------------------------------------------\n");
    info.push_str(&format!(
        "ROM files extensions accepted: \"{}\"\n",
        generate_systems::list_extensions(data, false)
    ));
    if let Some(comment) = &data.comment_en {
        info.push('\n');
        info.push_str(comment);
    }
    info.push_str("-------------------------------------------------------------------------------\n");
    info.push_str(&format!(
        "Extensions des fichiers ROMs permises: \"{}\"\n",
        generate_systems::list_extensions(data, false)
    ));
    if let Some(comment) = &data.comment_fr {
        info.push('\n');
        info.push_str(comment);
    }

    let info_path = roms_dir.join(&subdir).join("_info.txt");
    fs::write(info_path, info).ok();
}

/// Recursively copy a directory tree.
fn copy_dir_recursive(src: &Path, dst: &Path) {
    if let Err(e) = fs::create_dir_all(dst) {
        eprintln!("Failed to create directory {}: {}", dst.display(), e);
        return;
    }
    let entries = match fs::read_dir(src) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Failed to read directory {}: {}", src.display(), e);
            return;
        }
    };
    for entry in entries.flatten() {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path);
        } else {
            fs::copy(&src_path, &dst_path).ok();
        }
    }
}
