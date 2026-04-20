use std::collections::HashSet;
use std::fs;
use std::path::Path;

use indexmap::IndexMap;
use serde_json::{json, Map, Value as JsonValue};

use crate::models::*;
use crate::requirements;

// ═══════════════════════════════════════════════════════════════════════════════
// es_systems.json  (kept for possible future use; REG-Station currently only
// consumes rs_systems.cfg XML, so main.rs does not invoke this path.)
// ═══════════════════════════════════════════════════════════════════════════════

#[allow(dead_code)]
const DEFAULT_PARENTPATH: &str = "/userdata/roms";
#[allow(dead_code)]
const DEFAULT_COMMAND: &str = "emulatorlauncher %CONTROLLERSCONFIG% -system %SYSTEM% -rom %ROM% -gameinfoxml %GAMEINFOXML% -systemname %SYSTEMNAME%";

/// Generate es_systems as a JSON value.
#[allow(dead_code)]
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

#[allow(dead_code)]
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

    let mut sys = Map::new();
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

#[allow(dead_code)]
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

                let mut core_obj = Map::new();
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

#[allow(dead_code)]
fn system_path(name: &str, data: &System) -> String {
    match &data.path {
        Some(p) if p.starts_with('/') => p.clone(),
        Some(p) => format!("{}/{}", DEFAULT_PARENTPATH, p),
        None => format!("{}/{}", DEFAULT_PARENTPATH, name),
    }
}

#[allow(dead_code)]
fn system_platform(name: &str, data: &System) -> String {
    match &data.platform {
        Some(p) => p.clone(),
        None => name.to_string(),
    }
}

#[allow(dead_code)]
fn list_extensions_vec(data: &System) -> Vec<String> {
    data.extensions
        .iter()
        .map(|e| format!(".{}", e.to_lowercase()))
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════════════
// es_features.json — schema consumed by REG-Station (CustomFeatures.cpp)
// ═══════════════════════════════════════════════════════════════════════════════
//
// Matches the shape produced by REG-Station's legacy
// tools/convert_features_xml_to_json.py so the converter can be retired.
//
// Root:
//   {
//     "sharedFeatures": { "customFeatures": [<def>, ...] },      // defs only
//     "globalFeatures": {
//        "sharedFeatures": [<ref>, ...],                         // refs
//        "customFeatures": [<def>, ...],                         // defs
//     },
//     "emulators": [ <emulator> ],
//   }
//
// <emulator> / <core> / <system>:
//   { name, features?, sharedFeatures?, customFeatures?, cores?, systems? }
//   - `features`: comma-separated string, omitted when empty
//   - `cores` only for emulators; `systems` for emulators and cores
//
// <def>:
//   { name=prompt, description?, submenu?, preset?, preset-parameters?,
//     group?, order? (int if numeric, else string), value=key,
//     choices? [ { name, value? } ] }
//
// <ref>:
//   { value: shared_name }   — (other attributes unused by current emitter)

pub fn generate_features(
    features: &IndexMap<String, EmulatorFeatures>,
    arch: &str,
) -> JsonValue {
    let mut root = Map::new();

    // Top-level <sharedFeatures>: only definitions (from features["shared"].cfeatures).
    if let Some(shared_sec) = features.get("shared") {
        let defs = collect_cfeatures(&shared_sec.cfeatures, arch);
        if !defs.is_empty() {
            let mut obj = Map::new();
            obj.insert("customFeatures".into(), JsonValue::Array(defs));
            root.insert("sharedFeatures".into(), JsonValue::Object(obj));
        }
    }

    // Top-level <globalFeatures>: shared refs + definitions.
    if let Some(global_sec) = features.get("global") {
        let mut obj = Map::new();
        let refs = collect_shared_refs(&global_sec.shared, arch, features);
        if !refs.is_empty() {
            obj.insert("sharedFeatures".into(), JsonValue::Array(refs));
        }
        let defs = collect_cfeatures(&global_sec.cfeatures, arch);
        if !defs.is_empty() {
            obj.insert("customFeatures".into(), JsonValue::Array(defs));
        }
        if !obj.is_empty() {
            root.insert("globalFeatures".into(), JsonValue::Object(obj));
        }
    }

    // <emulators>: everything except the reserved "shared" / "global" keys.
    let mut emulators_out = Vec::new();
    for (emu_name, emu_data) in features {
        if emu_name == "global" || emu_name == "shared" {
            continue;
        }
        emulators_out.push(emulator_to_json(emu_name, emu_data, arch, features));
    }
    if !emulators_out.is_empty() {
        root.insert("emulators".into(), JsonValue::Array(emulators_out));
    }

    JsonValue::Object(root)
}

fn emulator_to_json(
    name: &str,
    data: &EmulatorFeatures,
    arch: &str,
    all_features: &IndexMap<String, EmulatorFeatures>,
) -> JsonValue {
    let mut obj = Map::new();
    obj.insert("name".into(), json!(name));

    if let Some(csv) = features_csv(&data.features) {
        obj.insert("features".into(), json!(csv));
    }

    let refs = collect_shared_refs(&data.shared, arch, all_features);
    if !refs.is_empty() {
        obj.insert("sharedFeatures".into(), JsonValue::Array(refs));
    }

    let defs = collect_cfeatures(&data.cfeatures, arch);
    if !defs.is_empty() {
        obj.insert("customFeatures".into(), JsonValue::Array(defs));
    }

    if !data.cores.is_empty() {
        let mut cores_out = Vec::new();
        for (core_name, core_data) in &data.cores {
            cores_out.push(core_to_json(core_name, core_data, arch, all_features));
        }
        obj.insert("cores".into(), JsonValue::Array(cores_out));
    }

    let sys_out = collect_systems(&data.systems, arch, all_features);
    if !sys_out.is_empty() {
        obj.insert("systems".into(), JsonValue::Array(sys_out));
    }

    JsonValue::Object(obj)
}

fn core_to_json(
    name: &str,
    data: &CoreFeatures,
    arch: &str,
    all_features: &IndexMap<String, EmulatorFeatures>,
) -> JsonValue {
    let mut obj = Map::new();
    obj.insert("name".into(), json!(name));

    if let Some(csv) = features_csv(&data.features) {
        obj.insert("features".into(), json!(csv));
    }

    let refs = collect_shared_refs(&data.shared, arch, all_features);
    if !refs.is_empty() {
        obj.insert("sharedFeatures".into(), JsonValue::Array(refs));
    }

    let defs = collect_cfeatures(&data.cfeatures, arch);
    if !defs.is_empty() {
        obj.insert("customFeatures".into(), JsonValue::Array(defs));
    }

    let sys_out = collect_systems(&data.systems, arch, all_features);
    if !sys_out.is_empty() {
        obj.insert("systems".into(), JsonValue::Array(sys_out));
    }

    JsonValue::Object(obj)
}

fn collect_systems(
    systems: &IndexMap<String, SystemFeatures>,
    arch: &str,
    all_features: &IndexMap<String, EmulatorFeatures>,
) -> Vec<JsonValue> {
    let mut out = Vec::new();
    for (sys_name, sys_data) in systems {
        let mut obj = Map::new();
        obj.insert("name".into(), json!(sys_name));

        if let Some(csv) = features_csv(&sys_data.features) {
            obj.insert("features".into(), json!(csv));
        }

        let refs = collect_shared_refs(&sys_data.shared, arch, all_features);
        if !refs.is_empty() {
            obj.insert("sharedFeatures".into(), JsonValue::Array(refs));
        }

        let defs = collect_cfeatures(&sys_data.cfeatures, arch);
        if !defs.is_empty() {
            obj.insert("customFeatures".into(), JsonValue::Array(defs));
        }

        out.push(JsonValue::Object(obj));
    }
    out
}

fn features_csv(list: &[String]) -> Option<String> {
    if list.is_empty() {
        None
    } else {
        Some(list.join(", "))
    }
}

fn collect_shared_refs(
    shared_list: &[String],
    arch: &str,
    all_features: &IndexMap<String, EmulatorFeatures>,
) -> Vec<JsonValue> {
    let mut out = Vec::new();
    let shared_section = match all_features.get("shared") {
        Some(s) => s,
        None => return out,
    };
    for shared_name in shared_list {
        if let Some(feature) = shared_section.cfeatures.get(shared_name) {
            if requirements::arch_valid(arch, feature) {
                out.push(json!({ "value": shared_name }));
            } else {
                eprintln!("skipping shared {}", shared_name);
            }
        }
    }
    out
}

fn collect_cfeatures(
    cfeatures: &IndexMap<String, CustomFeature>,
    arch: &str,
) -> Vec<JsonValue> {
    let mut out = Vec::new();
    for (cf_name, cf) in cfeatures {
        if !requirements::arch_valid(arch, cf) {
            continue;
        }
        out.push(feature_to_json(cf_name, cf));
    }
    out
}

fn feature_to_json(key: &str, cf: &CustomFeature) -> JsonValue {
    let mut obj = Map::new();
    // Match convert_features_xml_to_json.py parse_feature key order:
    // name first, then (description, submenu, preset, preset-parameters, group, order, value),
    // then choices.
    obj.insert("name".into(), json!(&cf.prompt));

    if let Some(v) = non_empty(&cf.description) {
        obj.insert("description".into(), json!(v));
    }
    if let Some(v) = non_empty(&cf.submenu) {
        obj.insert("submenu".into(), json!(v));
    }
    if let Some(v) = non_empty(&cf.preset) {
        obj.insert("preset".into(), json!(v));
    }
    if let Some(v) = non_empty(&cf.preset_parameters) {
        obj.insert("preset-parameters".into(), json!(v));
    }
    if let Some(v) = non_empty(&cf.group) {
        obj.insert("group".into(), json!(v));
    }
    if let Some(order) = &cf.order {
        let s = yaml_value_to_string(order);
        if !s.is_empty() {
            // Python converter: int(value) if possible, else keep the string.
            let order_val: JsonValue = match s.parse::<i64>() {
                Ok(n) => json!(n),
                Err(_) => json!(s),
            };
            obj.insert("order".into(), order_val);
        }
    }
    if !key.is_empty() {
        obj.insert("value".into(), json!(key));
    }

    // Choices are only emitted when there is no preset (matches the XML emitter).
    if cf.preset.is_none() && !cf.choices.is_empty() {
        let mut choices = Vec::new();
        for (choice_name, choice_value) in &cf.choices {
            let mut c = Map::new();
            c.insert("name".into(), json!(choice_name));
            let v = yaml_value_to_string(choice_value);
            if !v.is_empty() {
                c.insert("value".into(), json!(v));
            }
            choices.push(JsonValue::Object(c));
        }
        if !choices.is_empty() {
            obj.insert("choices".into(), JsonValue::Array(choices));
        }
    }

    JsonValue::Object(obj)
}

fn non_empty(opt: &Option<String>) -> Option<&str> {
    opt.as_deref().filter(|s| !s.is_empty())
}

/// Write JSON to a file.
pub fn write_json(value: &JsonValue, path: &Path) {
    let content = serde_json::to_string_pretty(value).expect("Failed to serialize JSON");
    fs::write(path, content).expect("Failed to write JSON file");
}
