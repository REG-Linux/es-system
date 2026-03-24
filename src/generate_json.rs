use std::collections::HashSet;
use std::fs;
use std::path::Path;

use indexmap::IndexMap;
use serde_json::{json, Value as JsonValue, Map};

use crate::models::*;
use crate::requirements;

// ═══════════════════════════════════════════════════════════════════════════════
// es_systems.json
// ═══════════════════════════════════════════════════════════════════════════════

const DEFAULT_PARENTPATH: &str = "/userdata/roms";
const DEFAULT_COMMAND: &str = "emulatorlauncher %CONTROLLERSCONFIG% -system %SYSTEM% -rom %ROM% -gameinfoxml %GAMEINFOXML% -systemname %SYSTEMNAME%";

/// Generate es_systems as a JSON value.
pub fn generate_systems(
    systems: &IndexMap<String, System>,
    config: &HashSet<String>,
    systems_config: &IndexMap<String, SystemDefault>,
    arch_systems_config: &IndexMap<String, SystemDefault>,
) -> JsonValue {
    let mut system_list = Vec::new();

    let mut sorted_names: Vec<&String> = systems.keys().collect();
    sorted_names.sort();

    for name in sorted_names {
        let data = match systems.get(name) {
            Some(d) => d,
            None => continue,
        };

        let default_emulator = arch_systems_config
            .get(name)
            .and_then(|d| d.emulator.as_ref())
            .or_else(|| systems_config.get(name).and_then(|d| d.emulator.as_ref()));
        let default_core = arch_systems_config
            .get(name)
            .and_then(|d| d.core.as_ref())
            .or_else(|| systems_config.get(name).and_then(|d| d.core.as_ref()));

        if let Some(sys) = gen_system_json(name, data, config, default_emulator, default_core) {
            system_list.push(sys);
        }
    }

    json!({ "systemList": system_list })
}

fn gen_system_json(
    name: &str,
    data: &System,
    config: &HashSet<String>,
    default_emulator: Option<&String>,
    default_core: Option<&String>,
) -> Option<JsonValue> {
    let emulators = list_emulators_json(data, config, default_emulator, default_core);
    if emulators.is_none() && !data.force {
        return None;
    }

    let path_value = system_path(name, data);
    let platform_value = system_platform(name, data);
    let extensions = list_extensions_vec(data);
    let group_value = data.group.as_deref().unwrap_or("");
    let command = data.command.as_deref().unwrap_or(DEFAULT_COMMAND);
    let theme = data.theme.as_deref().unwrap_or(name);

    let mut sys = serde_json::Map::new();
    sys.insert("name".into(), json!(name));
    sys.insert("fullname".into(), json!(&data.name));
    sys.insert("manufacturer".into(), json!(&data.manufacturer));
    sys.insert("release".into(), json!(yaml_value_to_string(&data.release)));
    sys.insert("hardware".into(), json!(&data.hardware));

    if !extensions.is_empty() {
        if !path_value.is_empty() {
            sys.insert("path".into(), json!(path_value));
        }
        sys.insert("extension".into(), json!(extensions));
        sys.insert("command".into(), json!(command));
    }
    if !platform_value.is_empty() {
        sys.insert("platform".into(), json!(platform_value));
    }
    sys.insert("theme".into(), json!(theme));
    if !group_value.is_empty() {
        sys.insert("group".into(), json!(group_value));
    }
    if let Some(emus) = emulators {
        sys.insert("emulators".into(), emus);
    }

    Some(JsonValue::Object(sys))
}

fn list_emulators_json(
    data: &System,
    config: &HashSet<String>,
    default_emulator: Option<&String>,
    default_core: Option<&String>,
) -> Option<JsonValue> {
    let mut emulators_out = Vec::new();

    let mut sorted_emulators: Vec<&String> = data.emulators.keys().collect();
    sorted_emulators.sort();

    for emulator in sorted_emulators {
        let emulator_data = &data.emulators[emulator];
        let mut cores_out = Vec::new();

        let mut sorted_cores: Vec<&String> = emulator_data.keys().collect();
        sorted_cores.sort();

        for core in sorted_cores {
            let core_data = &emulator_data[core];
            if requirements::is_valid(config, &core_data.require_any_of) {
                let is_default = default_emulator.map_or(false, |e| e == emulator)
                    && default_core.map_or(false, |c| c == core);

                let mut core_obj = serde_json::Map::new();
                core_obj.insert("name".into(), json!(core));
                if is_default {
                    core_obj.insert("default".into(), json!(true));
                }
                if !core_data.incompatible_extensions.is_empty() {
                    let exts: Vec<String> = core_data
                        .incompatible_extensions
                        .iter()
                        .map(|e| format!(".{}", e.to_lowercase()))
                        .collect();
                    core_obj.insert("incompatible_extensions".into(), json!(exts));
                }
                cores_out.push(JsonValue::Object(core_obj));
            }
        }

        if !cores_out.is_empty() {
            emulators_out.push(json!({
                "name": emulator,
                "cores": cores_out,
            }));
        }
    }

    if emulators_out.is_empty() {
        None
    } else {
        Some(json!(emulators_out))
    }
}

fn system_path(name: &str, data: &System) -> String {
    match &data.path {
        Some(p) if p.starts_with('/') => p.clone(),
        Some(p) => format!("{}/{}", DEFAULT_PARENTPATH, p),
        None => format!("{}/{}", DEFAULT_PARENTPATH, name),
    }
}

fn system_platform(name: &str, data: &System) -> String {
    match &data.platform {
        Some(p) => p.clone(),
        None => name.to_string(),
    }
}

fn list_extensions_vec(data: &System) -> Vec<String> {
    data.extensions
        .iter()
        .map(|e| format!(".{}", e.to_lowercase()))
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════════════
// es_features.json
// ═══════════════════════════════════════════════════════════════════════════════

/// Generate es_features as a JSON value.
pub fn generate_features(
    features: &IndexMap<String, EmulatorFeatures>,
    arch: &str,
) -> JsonValue {
    let mut out = Vec::new();

    for (emulator, emu_data) in features {
        let mut emu_obj = Map::new();

        // Tag type
        if emulator == "global" {
            emu_obj.insert("type".into(), json!("globalFeatures"));
        } else if emulator == "shared" {
            emu_obj.insert("type".into(), json!("sharedFeatures"));
        } else {
            emu_obj.insert("type".into(), json!("emulator"));
            emu_obj.insert("name".into(), json!(emulator));
        }

        if !emu_data.features.is_empty() {
            emu_obj.insert("features".into(), json!(&emu_data.features));
        }

        // Cores
        if !emu_data.cores.is_empty() {
            let mut cores_out = Vec::new();
            for (core_name, core_data) in &emu_data.cores {
                let mut core_obj = Map::new();
                core_obj.insert("name".into(), json!(core_name));
                if !core_data.features.is_empty() {
                    core_obj.insert("features".into(), json!(&core_data.features));
                }
                if !core_data.shared.is_empty() {
                    core_obj.insert("shared".into(), json!(&core_data.shared));
                }
                if !core_data.cfeatures.is_empty() {
                    core_obj.insert("cfeatures".into(), cfeatures_json(&core_data.cfeatures, arch));
                }
                if !core_data.systems.is_empty() {
                    core_obj.insert("systems".into(), systems_features_json(&core_data.systems, arch));
                }
                cores_out.push(JsonValue::Object(core_obj));
            }
            emu_obj.insert("cores".into(), json!(cores_out));
        }

        // Systems at emulator level
        if !emu_data.systems.is_empty() {
            emu_obj.insert("systems".into(), systems_features_json(&emu_data.systems, arch));
        }

        // Shared at emulator level
        if !emu_data.shared.is_empty() {
            emu_obj.insert("shared".into(), json!(&emu_data.shared));
        }

        // Cfeatures at emulator level
        if !emu_data.cfeatures.is_empty() {
            emu_obj.insert("cfeatures".into(), cfeatures_json(&emu_data.cfeatures, arch));
        }

        out.push(JsonValue::Object(emu_obj));
    }

    json!({ "features": out })
}

fn systems_features_json(
    systems: &IndexMap<String, SystemFeatures>,
    arch: &str,
) -> JsonValue {
    let mut out = Vec::new();
    for (sys_name, sys_data) in systems {
        let mut sys_obj = Map::new();
        sys_obj.insert("name".into(), json!(sys_name));
        if !sys_data.features.is_empty() {
            sys_obj.insert("features".into(), json!(&sys_data.features));
        }
        if !sys_data.shared.is_empty() {
            sys_obj.insert("shared".into(), json!(&sys_data.shared));
        }
        if !sys_data.cfeatures.is_empty() {
            sys_obj.insert("cfeatures".into(), cfeatures_json(&sys_data.cfeatures, arch));
        }
        out.push(JsonValue::Object(sys_obj));
    }
    json!(out)
}

fn cfeatures_json(
    cfeatures: &IndexMap<String, CustomFeature>,
    arch: &str,
) -> JsonValue {
    let mut out = Vec::new();
    for (cf_name, cf) in cfeatures {
        if !requirements::arch_valid(arch, cf) {
            continue;
        }
        let mut obj = Map::new();
        obj.insert("value".into(), json!(cf_name));
        obj.insert("name".into(), json!(&cf.prompt));
        if let Some(desc) = &cf.description {
            if !desc.is_empty() {
                obj.insert("description".into(), json!(desc));
            }
        }
        if let Some(group) = &cf.group {
            obj.insert("group".into(), json!(group));
        }
        if let Some(submenu) = &cf.submenu {
            obj.insert("submenu".into(), json!(submenu));
        }
        if let Some(order) = &cf.order {
            obj.insert("order".into(), json!(yaml_value_to_string(order)));
        }
        if let Some(preset) = &cf.preset {
            obj.insert("preset".into(), json!(preset));
        }
        if let Some(params) = &cf.preset_parameters {
            obj.insert("preset_parameters".into(), json!(params));
        }
        if !cf.choices.is_empty() && cf.preset.is_none() {
            let choices: Vec<JsonValue> = cf.choices.iter().map(|(k, v)| {
                json!({ "name": k, "value": yaml_value_to_string(v) })
            }).collect();
            obj.insert("choices".into(), json!(choices));
        }
        out.push(JsonValue::Object(obj));
    }
    json!(out)
}

/// Write JSON to a file.
pub fn write_json(value: &JsonValue, path: &Path) {
    let content = serde_json::to_string_pretty(value).expect("Failed to serialize JSON");
    fs::write(path, content).expect("Failed to write JSON file");
}
