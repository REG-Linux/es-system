use std::collections::HashSet;
use std::fs;
use std::path::Path;

use indexmap::IndexMap;

use crate::models::{System, SystemDefault, yaml_value_to_string};
use crate::requirements;
use crate::xml;

const DEFAULT_PARENTPATH: &str = "/userdata/roms";
const DEFAULT_COMMAND: &str = "emulatorlauncher %CONTROLLERSCONFIG% -system %SYSTEM% -rom %ROM% -gameinfoxml %GAMEINFOXML% -systemname %SYSTEMNAME%";

/// Generate the complete es_systems.cfg XML content.
pub fn generate(
    systems: &IndexMap<String, System>,
    config: &HashSet<String>,
    systems_config: &IndexMap<String, SystemDefault>,
    arch_systems_config: &IndexMap<String, SystemDefault>,
) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\n");
    xml.push_str("<systemList>\n");

    let mut sorted_names: Vec<&String> = systems.keys().collect();
    sorted_names.sort();

    for name in sorted_names {
        if let Some(data) = systems.get(name) {
            // Compute default emulator/core
            let default_emulator = arch_systems_config
                .get(name)
                .and_then(|d| d.emulator.as_ref())
                .or_else(|| systems_config.get(name).and_then(|d| d.emulator.as_ref()));
            let default_core = arch_systems_config
                .get(name)
                .and_then(|d| d.core.as_ref())
                .or_else(|| systems_config.get(name).and_then(|d| d.core.as_ref()));

            xml.push_str(&gen_system(name, data, config, default_emulator, default_core));
        }
    }

    xml.push_str("</systemList>\n");
    xml
}

/// Generate a single <system> XML block.
fn gen_system(
    name: &str,
    data: &System,
    config: &HashSet<String>,
    default_emulator: Option<&String>,
    default_core: Option<&String>,
) -> String {
    let emulators_txt = list_emulators(data, config, default_emulator, default_core);
    if emulators_txt.is_empty() && !data.force {
        return String::new();
    }

    let path_value = system_path(name, data);
    let platform_value = system_platform(name, data);
    let extensions = list_extensions(data, false);
    let group_value = system_group(data);
    let command = command_name(data);

    let mut txt = String::new();
    txt.push_str("  <system>\n");
    txt.push_str(&format!("        <fullname>{}</fullname>\n", xml::escape(&data.name)));
    txt.push_str(&format!("        <name>{}</name>\n", name));
    txt.push_str(&format!("        <manufacturer>{}</manufacturer>\n", xml::escape(&data.manufacturer)));
    txt.push_str(&format!("        <release>{}</release>\n", xml::escape(&yaml_value_to_string(&data.release))));
    txt.push_str(&format!("        <hardware>{}</hardware>\n", xml::escape(&data.hardware)));
    if !extensions.is_empty() {
        if !path_value.is_empty() {
            txt.push_str(&format!("        <path>{}</path>\n", path_value));
        }
        txt.push_str(&format!("        <extension>{}</extension>\n", extensions));
        txt.push_str(&format!("        <command>{}</command>\n", command));
    }
    if !platform_value.is_empty() {
        txt.push_str(&format!("        <platform>{}</platform>\n", xml::escape(&platform_value)));
    }
    txt.push_str(&format!("        <theme>{}</theme>\n", theme_name(name, data)));
    if !group_value.is_empty() {
        txt.push_str(&format!("        <group>{}</group>\n", xml::escape(&group_value)));
    }
    txt.push_str(&emulators_txt);
    txt.push_str("  </system>\n");
    txt
}

fn system_path(name: &str, data: &System) -> String {
    match &data.path {
        Some(p) if p.starts_with('/') => p.clone(),
        Some(p) => format!("{}/{}", DEFAULT_PARENTPATH, p),
        None => format!("{}/{}", DEFAULT_PARENTPATH, name),
    }
}

/// Returns the subdirectory for ROM folders (relative), or None if no dir needed.
pub fn system_sub_roms_dir(name: &str, data: &System) -> Option<String> {
    match &data.path {
        Some(p) if p.starts_with('/') => None, // absolute path, don't create
        Some(p) => Some(p.clone()),
        None => Some(name.to_string()),
    }
}

fn system_platform(name: &str, data: &System) -> String {
    match &data.platform {
        Some(p) => p.clone(),
        None => name.to_string(),
    }
}

fn theme_name(name: &str, data: &System) -> String {
    match &data.theme {
        Some(t) => t.clone(),
        None => name.to_string(),
    }
}

fn command_name(data: &System) -> String {
    match &data.command {
        Some(c) => c.clone(),
        None => DEFAULT_COMMAND.to_string(),
    }
}

fn system_group(data: &System) -> String {
    data.group.as_deref().unwrap_or("").to_string()
}

/// List extensions as ".ext1 .ext2 ..." string.
pub fn list_extensions(data: &System, uppercase: bool) -> String {
    let mut result = String::new();
    for (i, ext) in data.extensions.iter().enumerate() {
        if i > 0 {
            result.push(' ');
        }
        let lower = ext.to_lowercase();
        result.push('.');
        result.push_str(&lower);
        if uppercase {
            result.push_str(" .");
            result.push_str(&ext.to_uppercase());
        }
    }
    result
}

/// Generate the <emulators> XML block for a system.
fn list_emulators(
    data: &System,
    config: &HashSet<String>,
    default_emulator: Option<&String>,
    default_core: Option<&String>,
) -> String {
    let mut emulators_txt = String::new();

    let mut sorted_emulators: Vec<&String> = data.emulators.keys().collect();
    sorted_emulators.sort();

    for emulator in sorted_emulators {
        let emulator_data = &data.emulators[emulator];

        let mut emulator_header = format!("            <emulator name=\"{}\">\n", emulator);
        emulator_header.push_str("                <cores>\n");

        let mut cores_txt = String::new();
        let mut sorted_cores: Vec<&String> = emulator_data.keys().collect();
        sorted_cores.sort();

        for core in sorted_cores {
            let core_data = &emulator_data[core];
            if requirements::is_valid(config, &core_data.require_any_of) {
                let mut incompatible = String::new();
                if !core_data.incompatible_extensions.is_empty() {
                    let exts: Vec<String> = core_data
                        .incompatible_extensions
                        .iter()
                        .map(|e| format!(".{}", e.to_lowercase()))
                        .collect();
                    incompatible = format!(" incompatible_extensions=\"{}\"", exts.join(" "));
                }

                let is_default = default_emulator.map_or(false, |e| e == emulator)
                    && default_core.map_or(false, |c| c == core);

                if is_default {
                    cores_txt.push_str(&format!(
                        "                    <core default=\"true\"{}>{}</core>\n",
                        incompatible, core
                    ));
                } else {
                    cores_txt.push_str(&format!(
                        "                    <core{}>{}</core>\n",
                        incompatible, core
                    ));
                }
            }
        }

        if cores_txt.is_empty() {
            continue;
        }

        emulators_txt.push_str(&emulator_header);
        emulators_txt.push_str(&cores_txt);
        emulators_txt.push_str("                </cores>\n");
        emulators_txt.push_str("            </emulator>\n");
    }

    if emulators_txt.is_empty() {
        return String::new();
    }

    let mut result = String::from("        <emulators>\n");
    result.push_str(&emulators_txt);
    result.push_str("        </emulators>\n");
    result
}

/// Check if any emulator/core in this system has valid requirements.
pub fn need_folder(data: &System, config: &HashSet<String>) -> bool {
    for emulator_data in data.emulators.values() {
        for core_data in emulator_data.values() {
            if requirements::is_valid(config, &core_data.require_any_of) {
                return true;
            }
        }
    }
    false
}

/// Write the es_systems.cfg file.
pub fn write_file(content: &str, path: &Path) {
    fs::write(path, content).expect("Failed to write es_systems.cfg");
}
