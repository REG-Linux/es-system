use std::fs;
use std::path::Path;

use indexmap::IndexMap;

use crate::models::{CustomFeature, EmulatorFeatures, SystemFeatures, yaml_value_to_string};
use crate::requirements;
use crate::xml;

/// Translation comment entry: which emulator/core uses a translatable string.
#[derive(Debug)]
pub struct TranslationComment {
    pub emulator: String,
    pub core: Option<String>,
}

fn add_translation(
    to_translate: &mut IndexMap<String, Vec<TranslationComment>>,
    text: &str,
    emulator: &str,
    core: Option<&str>,
) {
    let entry = to_translate.entry(text.to_string()).or_default();
    entry.push(TranslationComment {
        emulator: emulator.to_string(),
        core: core.map(|s| s.to_string()),
    });
}

fn array2vallist(arr: &[String]) -> String {
    arr.join(", ")
}

/// Generate a single <feature> XML element.
fn xml_feature(
    indent: usize,
    key: &str,
    info: &CustomFeature,
    to_translate: &mut IndexMap<String, Vec<TranslationComment>>,
    emulator: &str,
    core: Option<&str>,
) -> String {
    let spaces = " ".repeat(indent);
    let mut txt = String::new();

    let description = info.description.as_deref().unwrap_or("");
    let mut submenu_str = String::new();
    if let Some(submenu) = &info.submenu {
        submenu_str = format!(" submenu=\"{}\"", xml::escape(submenu));
    }
    let mut group_str = String::new();
    if let Some(group) = &info.group {
        group_str = format!(" group=\"{}\"", xml::escape(group));
    }
    let mut order_str = String::new();
    if let Some(order) = &info.order {
        order_str = format!(" order=\"{}\"", xml::escape(&yaml_value_to_string(order)));
    }
    let mut preset_str = String::new();
    if let Some(preset) = &info.preset {
        preset_str = format!(" preset=\"{}\"", xml::escape(preset));
    }
    if let Some(params) = &info.preset_parameters {
        preset_str.push_str(&format!(" preset-parameters=\"{}\"", xml::escape(params)));
    }

    txt.push_str(&format!(
        "{}<feature name=\"{}\"{}{}{} value=\"{}\" description=\"{}\"{}>\n",
        spaces,
        xml::escape(&info.prompt),
        submenu_str,
        group_str,
        order_str,
        xml::escape(key),
        xml::escape(description),
        preset_str,
    ));

    add_translation(to_translate, &info.prompt, emulator, core);
    add_translation(to_translate, description, emulator, core);

    if info.preset.is_none() {
        for (choice_name, choice_value) in &info.choices {
            txt.push_str(&format!(
                "{}  <choice name=\"{}\" value=\"{}\" />\n",
                spaces,
                xml::escape(choice_name),
                xml::escape(&yaml_value_to_string(choice_value)),
            ));
            add_translation(to_translate, choice_name, emulator, core);
        }
    }

    txt.push_str(&format!("{}</feature>\n", spaces));
    txt
}

/// Generate shared feature references for a given system/core context.
fn gen_shared_features(
    shared_list: &[String],
    arch: &str,
    all_features: &IndexMap<String, EmulatorFeatures>,
    indent: usize,
) -> String {
    let spaces = " ".repeat(indent);
    let mut txt = String::new();
    let shared_section = match all_features.get("shared") {
        Some(s) => s,
        None => return txt,
    };
    for shared_name in shared_list {
        if let Some(feature) = shared_section.cfeatures.get(shared_name) {
            if requirements::arch_valid(arch, feature) {
                txt.push_str(&format!(
                    "{}<sharedFeature value=\"{}\" />\n",
                    spaces,
                    xml::escape(shared_name),
                ));
            } else {
                eprintln!("skipping shared {}", shared_name);
            }
        }
    }
    txt
}

/// Generate cfeatures XML for a given context.
fn gen_cfeatures(
    cfeatures: &IndexMap<String, CustomFeature>,
    arch: &str,
    indent: usize,
    to_translate: &mut IndexMap<String, Vec<TranslationComment>>,
    emulator: &str,
    core: Option<&str>,
    skip_label: &str,
) -> String {
    let mut txt = String::new();
    for (cf_name, cf) in cfeatures {
        if requirements::arch_valid(arch, cf) {
            txt.push_str(&xml_feature(indent, cf_name, cf, to_translate, emulator, core));
        } else {
            eprintln!("skipping {} cfeature {}", skip_label, cf_name);
        }
    }
    txt
}

/// Generate the <systems> block within a core or emulator context.
fn gen_systems_block(
    systems: &IndexMap<String, SystemFeatures>,
    arch: &str,
    all_features: &IndexMap<String, EmulatorFeatures>,
    to_translate: &mut IndexMap<String, Vec<TranslationComment>>,
    emulator: &str,
    core: Option<&str>,
    indent_base: usize,
) -> String {
    let spaces = " ".repeat(indent_base);
    let mut txt = String::new();
    txt.push_str(&format!("{}<systems>\n", spaces));

    for (sys_name, sys) in systems {
        let sys_features_txt = array2vallist(&sys.features);
        txt.push_str(&format!(
            "{}  <system name=\"{}\" features=\"{}\" >\n",
            spaces,
            xml::escape(sys_name),
            xml::escape(&sys_features_txt),
        ));

        // shared
        txt.push_str(&gen_shared_features(
            &sys.shared,
            arch,
            all_features,
            indent_base + 4,
        ));

        // cfeatures
        let label = format!("system {}/{}", emulator, sys_name);
        txt.push_str(&gen_cfeatures(
            &sys.cfeatures,
            arch,
            indent_base + 4,
            to_translate,
            emulator,
            core,
            &label,
        ));

        txt.push_str(&format!("{}  </system>\n", spaces));
    }

    txt.push_str(&format!("{}</systems>\n", spaces));
    txt
}

/// Generate the complete es_features.cfg XML content.
/// This is the most complex function, porting createEsFeatures() from Python.
pub fn generate(
    features: &IndexMap<String, EmulatorFeatures>,
    arch: &str,
    to_translate: &mut IndexMap<String, Vec<TranslationComment>>,
) -> String {
    let mut txt = String::new();
    txt.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\n");
    txt.push_str("<features>\n");

    for (emulator, emu_data) in features {
        let emu_features_txt = array2vallist(&emu_data.features);

        let has_content = !emu_data.cores.is_empty()
            || !emu_data.systems.is_empty()
            || !emu_data.cfeatures.is_empty()
            || !emu_data.shared.is_empty();

        // Opening tag
        if emulator == "global" {
            txt.push_str("  <globalFeatures");
        } else if emulator == "shared" {
            txt.push_str("  <sharedFeatures");
        } else {
            txt.push_str(&format!(
                "  <emulator name=\"{}\" features=\"{}\"",
                xml::escape(emulator),
                xml::escape(&emu_features_txt),
            ));
        }

        if !has_content {
            txt.push_str(" />\n");
            continue;
        }

        txt.push_str(">\n");

        // Cores
        if !emu_data.cores.is_empty() {
            txt.push_str("    <cores>\n");
            for (core_name, core_data) in &emu_data.cores {
                let core_features_txt = array2vallist(&core_data.features);

                let core_has_content = !core_data.cfeatures.is_empty()
                    || !core_data.shared.is_empty()
                    || !core_data.systems.is_empty();

                if core_has_content {
                    txt.push_str(&format!(
                        "      <core name=\"{}\" features=\"{}\">\n",
                        xml::escape(core_name),
                        xml::escape(&core_features_txt),
                    ));

                    // Shared in core
                    txt.push_str(&gen_shared_features(
                        &core_data.shared,
                        arch,
                        features,
                        8,
                    ));

                    // Core cfeatures
                    let label = format!("core {}/{}", emulator, core_name);
                    txt.push_str(&gen_cfeatures(
                        &core_data.cfeatures,
                        arch,
                        8,
                        to_translate,
                        emulator,
                        Some(core_name),
                        &label,
                    ));

                    // Systems within core
                    if !core_data.systems.is_empty() {
                        txt.push_str(&gen_systems_block(
                            &core_data.systems,
                            arch,
                            features,
                            to_translate,
                            emulator,
                            Some(core_name),
                            8,
                        ));
                    }

                    txt.push_str("      </core>\n");
                } else {
                    txt.push_str(&format!(
                        "      <core name=\"{}\" features=\"{}\" />\n",
                        xml::escape(core_name),
                        xml::escape(&core_features_txt),
                    ));
                }
            }
            txt.push_str("    </cores>\n");
        }

        // Systems at emulator level
        if !emu_data.systems.is_empty() {
            txt.push_str("    <systems>\n");
            for (sys_name, sys_data) in &emu_data.systems {
                let sys_features_txt = array2vallist(&sys_data.features);
                txt.push_str(&format!(
                    "      <system name=\"{}\" features=\"{}\" >\n",
                    xml::escape(sys_name),
                    xml::escape(&sys_features_txt),
                ));

                // shared
                txt.push_str(&gen_shared_features(
                    &sys_data.shared,
                    arch,
                    features,
                    4,
                ));

                // cfeatures
                let label = format!("system {}/{}", emulator, sys_name);
                txt.push_str(&gen_cfeatures(
                    &sys_data.cfeatures,
                    arch,
                    8,
                    to_translate,
                    emulator,
                    None,
                    &label,
                ));

                txt.push_str("      </system>\n");
            }
            txt.push_str("    </systems>\n");
        }

        // Shared at emulator level
        txt.push_str(&gen_shared_features(
            &emu_data.shared,
            arch,
            features,
            4,
        ));

        // Cfeatures at emulator level
        let label = format!("emulator {}", emulator);
        txt.push_str(&gen_cfeatures(
            &emu_data.cfeatures,
            arch,
            4,
            to_translate,
            emulator,
            None,
            &label,
        ));

        // Closing tag
        if emulator == "global" {
            txt.push_str("  </globalFeatures>\n");
        } else if emulator == "shared" {
            txt.push_str("  </sharedFeatures>\n");
        } else {
            txt.push_str("  </emulator>\n");
        }
    }

    txt.push_str("</features>\n");
    txt
}

/// Write the es_features.cfg file.
pub fn write_file(content: &str, path: &Path) {
    fs::write(path, content).expect("Failed to write es_features.cfg");
}
