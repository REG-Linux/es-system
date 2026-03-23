use std::collections::HashSet;
use std::fs;
use std::path::Path;

use indexmap::IndexMap;
use regex::Regex;

use crate::generate_features::TranslationComment;
use crate::models::EmulatorFeatures;

/// Find all translatable strings from the features YAML, regardless of arch.
/// Python: findTranslations()
pub fn find_all(features: &IndexMap<String, EmulatorFeatures>) -> IndexMap<String, Vec<TranslationComment>> {
    let mut to_translate: IndexMap<String, Vec<TranslationComment>> = IndexMap::new();

    for (emulator, emu_data) in features {
        // Cores
        for (core_name, core_data) in &emu_data.cores {
            // Core cfeatures
            extract_cfeatures(&core_data.cfeatures, emulator, Some(core_name), &mut to_translate);

            // Systems within cores
            for (_sys_name, sys_data) in &core_data.systems {
                extract_cfeatures(&sys_data.cfeatures, emulator, Some(core_name), &mut to_translate);
            }
        }

        // Emulator-level systems
        for (_sys_name, sys_data) in &emu_data.systems {
            // Note: Python uses `core` variable from the outer loop which is stale here.
            // We pass None for core at emulator-level systems.
            extract_cfeatures(&sys_data.cfeatures, emulator, None, &mut to_translate);
        }

        // Emulator-level cfeatures
        extract_cfeatures_no_core(&emu_data.cfeatures, emulator, &mut to_translate);
    }

    to_translate
}

fn extract_cfeatures(
    cfeatures: &IndexMap<String, crate::models::CustomFeature>,
    emulator: &str,
    core: Option<&str>,
    to_translate: &mut IndexMap<String, Vec<TranslationComment>>,
) {
    for (_cf_name, cf) in cfeatures {
        for tag in &["description", "submenu", "group", "prompt"] {
            let val = match *tag {
                "description" => cf.description.as_deref(),
                "submenu" => cf.submenu.as_deref(),
                "group" => cf.group.as_deref(),
                "prompt" => Some(cf.prompt.as_str()),
                _ => None,
            };
            if let Some(v) = val {
                add_comment(to_translate, v, emulator, core);
            }
        }
        for choice_name in cf.choices.keys() {
            add_comment(to_translate, choice_name, emulator, core);
        }
    }
}

fn extract_cfeatures_no_core(
    cfeatures: &IndexMap<String, crate::models::CustomFeature>,
    emulator: &str,
    to_translate: &mut IndexMap<String, Vec<TranslationComment>>,
) {
    for (_cf_name, cf) in cfeatures {
        for tag in &["description", "submenu", "group", "prompt"] {
            let val = match *tag {
                "description" => cf.description.as_deref(),
                "submenu" => cf.submenu.as_deref(),
                "group" => cf.group.as_deref(),
                "prompt" => Some(cf.prompt.as_str()),
                _ => None,
            };
            if let Some(v) = val {
                let entry = to_translate.entry(v.to_string()).or_default();
                entry.push(TranslationComment {
                    emulator: emulator.to_string(),
                    core: None,
                });
            }
        }
        for choice_name in cf.choices.keys() {
            let entry = to_translate.entry(choice_name.to_string()).or_default();
            entry.push(TranslationComment {
                emulator: emulator.to_string(),
                core: None,
            });
        }
    }
}

fn add_comment(
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

/// Generate the es_external_translations.h C header file.
/// Python: createEsTranslations()
pub fn write_header(
    path: &Path,
    to_translate: &IndexMap<String, Vec<TranslationComment>>,
    blacklisted_words: &HashSet<String>,
) {
    let skip_patterns = build_skip_patterns();
    let mut out = String::new();
    out.push_str("// file generated automatically by es-system, don't modify it\n\n");

    let mut n = 1;
    for (text, comments) in to_translate {
        // skip None/empty
        if text.is_empty() {
            continue;
        }
        // skip blacklisted
        if blacklisted_words.contains(text) {
            continue;
        }
        // skip numeric patterns
        if should_skip(text, &skip_patterns) {
            continue;
        }

        let vcomment = format_comment(comments);
        out.push_str(&format!("/* TRANSLATION: {} */\n", vcomment));
        out.push_str(&format!(
            "#define fake_gettext_external_{} pgettext(\"game_options\", \"{}\")\n",
            n,
            text.replace('"', "\\\""),
        ));
        n += 1;
    }

    fs::write(path, out).expect("Failed to write translations header");
}

/// Generate the es_keys_translations.h C header file from .keys JSON files.
/// Python: createEsKeysTranslations()
pub fn write_keys_header(path: &Path, keys_parent_folder: &Path) {
    eprintln!("generating {}...", path.display());

    let pattern = format!("{}/**/*.keys", keys_parent_folder.display());
    let mut vals: IndexMap<String, IndexMap<String, ()>> = IndexMap::new();

    for entry in glob::glob(&pattern).expect("Failed to glob .keys files") {
        if let Ok(file_path) = entry {
            eprintln!("... {}", file_path.display());
            let content = fs::read_to_string(&file_path).unwrap_or_default();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(obj) = json.as_object() {
                    for (_device, actions) in obj {
                        if let Some(arr) = actions.as_array() {
                            for action in arr {
                                if let Some(desc) = action.get("description").and_then(|d| d.as_str()) {
                                    vals.entry(desc.to_string())
                                        .or_default()
                                        .insert(
                                            file_path.file_name().unwrap().to_string_lossy().to_string(),
                                            (),
                                        );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut out = String::new();
    out.push_str("// file generated automatically by es-system, don't modify it\n\n");

    let mut n = 0;
    for (text, sources) in &vals {
        let mut vcomment = String::new();
        let mut vn = 0;
        for source_file in sources.keys() {
            if vn < 5 {
                if !vcomment.is_empty() {
                    vcomment.push_str(", ");
                }
                vcomment.push_str(source_file);
            } else if vn == 5 {
                vcomment.push_str(", ...");
            }
            vn += 1;
        }
        out.push_str(&format!("/* TRANSLATION: {} */\n", vcomment));
        out.push_str(&format!(
            "#define fake_gettext_external_{} pgettext(\"keys_files\", \"{}\")\n",
            n,
            text.replace('"', "\\\""),
        ));
        n += 1;
    }

    fs::write(path, out).expect("Failed to write keys translations header");
}

/// Load blacklisted words from a text file.
pub fn load_blacklist(path: &Path) -> HashSet<String> {
    let mut set = HashSet::new();
    if let Ok(content) = fs::read_to_string(path) {
        for line in content.lines() {
            let line = line.trim_end_matches('\n').trim_end_matches('\r');
            if !line.is_empty() {
                set.insert(line.to_string());
            }
        }
    }
    set
}

/// Format comment string for translation header (up to 5 sources).
fn format_comment(comments: &[TranslationComment]) -> String {
    let mut vcomment = String::new();
    for (i, c) in comments.iter().enumerate() {
        if i < 5 {
            if !vcomment.is_empty() {
                vcomment.push_str(", ");
            }
            if c.core.is_none() || c.core.as_deref() == Some(&c.emulator) {
                vcomment.push_str(&c.emulator);
            } else {
                vcomment.push_str(&format!("{}/{}", c.emulator, c.core.as_deref().unwrap()));
            }
        } else if i == 5 {
            vcomment.push_str(", ...");
        }
    }
    vcomment
}

struct SkipPatterns {
    integer: Regex,
    float: Regex,
    ratio_colon: Regex,
    ratio_slash: Regex,
    number_suffix: Regex,
    resolution: Regex,
    resolution_complex: Regex,
}

fn build_skip_patterns() -> SkipPatterns {
    SkipPatterns {
        integer: Regex::new(r"^[0-9]+[+]?$").unwrap(),
        float: Regex::new(r"^[0-9]+\.[0-9]+[+]?$").unwrap(),
        ratio_colon: Regex::new(r"^[0-9]+:[0-9]+$").unwrap(),
        ratio_slash: Regex::new(r"^[0-9]+/[0-9]+$").unwrap(),
        number_suffix: Regex::new(r"^[+-]?[0-9]+[%x]?$").unwrap(),
        resolution: Regex::new(r"^[0-9]+x[0-9]+$").unwrap(),
        resolution_complex: Regex::new(
            r"^[xX]?[0-9]*[xX]?[ ]*\(?[0-9]+[x]?[0-9]+[pK]?\)?[ ]*\(?[0-9]+[x]?[0-9]+[pK]?\)?$"
        ).unwrap(),
    }
}

fn should_skip(text: &str, patterns: &SkipPatterns) -> bool {
    patterns.integer.is_match(text)
        || patterns.float.is_match(text)
        || patterns.ratio_colon.is_match(text)
        || patterns.ratio_slash.is_match(text)
        || patterns.number_suffix.is_match(text)
        || patterns.resolution.is_match(text)
        || patterns.resolution_complex.is_match(text)
}
