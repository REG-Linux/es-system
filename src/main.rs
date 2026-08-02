mod config;
mod generate_features;
mod generate_json;
mod generate_systems;
mod models;
mod requirements;
mod roms;
mod translations;
mod xml;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use indexmap::IndexMap;

use models::{EmulatorFeatures, System, SystemDefault};

#[derive(Clone, Copy, PartialEq)]
enum OutputFormat {
    Xml,
    Json,
}

/// Returns `None` when `--format` was not given, so each mode can pick its own
/// default: XML for the legacy build-time invocation, JSON for `regenerate`
/// (the only features format REG-Station reads).
fn parse_format(args: &[String]) -> (Option<OutputFormat>, Vec<String>) {
    let mut format = None;
    let mut filtered = Vec::new();
    let mut skip_next = false;

    for (i, arg) in args.iter().enumerate() {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "--format" {
            if let Some(val) = args.get(i + 1) {
                match val.as_str() {
                    "json" => format = Some(OutputFormat::Json),
                    "xml" => format = Some(OutputFormat::Xml),
                    _ => {
                        eprintln!("Unknown format: {} (expected: xml, json)", val);
                        process::exit(1);
                    }
                }
                skip_next = true;
            }
        } else {
            filtered.push(arg.clone());
        }
    }

    (format, filtered)
}

fn main() {
    let raw_args: Vec<String> = env::args().collect();
    let (format, args) = parse_format(&raw_args);

    if args.len() > 1 && args[1] == "regenerate" {
        regenerate_mode(&args[2..], format.unwrap_or(OutputFormat::Json));
        return;
    }

    // Legacy positional args mode (compatible with Python es-system.py invocation)
    if args.len() < 15 {
        eprintln!("Usage: {} [--format xml|json] <yml> <features> <es_translations> <es_keys_translations> \\", args[0]);
        eprintln!("         <keys_parent_folder> <blacklisted_words> <config> \\");
        eprintln!("         <es_systems> <es_features> <gen_defaults_global> \\");
        eprintln!("         <gen_defaults_arch> <romsdirsource> <romsdirtarget> <arch>");
        eprintln!();
        eprintln!("Or: {} regenerate [--user-dir DIR] [--data-dir DIR] [--output-dir DIR] \\", args[0]);
        eprintln!("         [--launcher-dir DIR] [--config FILE] [--arch ARCH] [--format xml|json]");
        process::exit(1);
    }

    let yml_path = PathBuf::from(&args[1]);
    let features_path = PathBuf::from(&args[2]);
    let es_translations_path = PathBuf::from(&args[3]);
    let es_keys_translations_path = PathBuf::from(&args[4]);
    let keys_parent_folder = PathBuf::from(&args[5]);
    let blacklisted_words_path = PathBuf::from(&args[6]);
    let config_path = PathBuf::from(&args[7]);
    let es_systems_path = PathBuf::from(&args[8]);
    let es_features_path = PathBuf::from(&args[9]);
    let gen_defaults_global_path = PathBuf::from(&args[10]);
    let gen_defaults_arch_path = PathBuf::from(&args[11]);
    let roms_dir_source = PathBuf::from(&args[12]);
    let roms_dir_target = PathBuf::from(&args[13]);
    let arch = &args[14];

    generate_all(
        &yml_path,
        &features_path,
        &es_translations_path,
        &es_keys_translations_path,
        &keys_parent_folder,
        &blacklisted_words_path,
        &config_path,
        &es_systems_path,
        &es_features_path,
        &gen_defaults_global_path,
        &gen_defaults_arch_path,
        &roms_dir_source,
        &roms_dir_target,
        arch,
        format.unwrap_or(OutputFormat::Xml),
    );
}

fn generate_all(
    yml_path: &Path,
    features_path: &Path,
    es_translations_path: &Path,
    es_keys_translations_path: &Path,
    keys_parent_folder: &Path,
    blacklisted_words_path: &Path,
    config_path: &Path,
    es_systems_path: &Path,
    es_features_path: &Path,
    gen_defaults_global_path: &Path,
    gen_defaults_arch_path: &Path,
    roms_dir_source: &Path,
    roms_dir_target: &Path,
    arch: &str,
    format: OutputFormat,
) {
    // Load inputs
    let systems: IndexMap<String, System> = load_yaml(yml_path);
    let config = config::load_config(config_path);

    let systems_config: IndexMap<String, SystemDefault> = load_yaml(gen_defaults_global_path);
    let arch_systems_config: IndexMap<String, SystemDefault> =
        load_yaml_or_empty(gen_defaults_arch_path);

    let features: IndexMap<String, EmulatorFeatures> = load_yaml_ordered(features_path);

    // Systems: always XML (rs_systems.cfg) — REG-Station consumes XML only.
    eprintln!("generating the {} file...", es_systems_path.display());
    let systems_xml = generate_systems::generate(&systems, &config, &systems_config, &arch_systems_config);
    generate_systems::write_file(&systems_xml, es_systems_path);

    // Features: JSON (new default for REG-Station) or XML (legacy).
    match format {
        OutputFormat::Xml => {
            let mut to_translate_on_arch: IndexMap<String, Vec<generate_features::TranslationComment>> =
                IndexMap::new();
            let features_xml = generate_features::generate(&features, arch, &mut to_translate_on_arch);
            generate_features::write_file(&features_xml, es_features_path);
        }
        OutputFormat::Json => {
            let features_json = generate_json::generate_features(&features, arch);
            generate_json::write_json(&features_json, es_features_path);
        }
    }

    // Find all translations (arch-independent) — always generate regardless of format
    let mut to_translate = translations::find_all(&features);

    // Remove blacklisted words
    let blacklist = translations::load_blacklist(blacklisted_words_path);
    to_translate.retain(|k, _| !blacklist.contains(k));

    // Generate translation headers
    translations::write_header(es_translations_path, &to_translate, &blacklist);
    translations::write_keys_header(es_keys_translations_path, keys_parent_folder);

    // Generate ROM directories
    eprintln!("removing the {} folder...", roms_dir_target.display());
    if roms_dir_target.is_dir() {
        fs::remove_dir_all(roms_dir_target).ok();
    }
    eprintln!("generating the {} folder...", roms_dir_target.display());

    let mut sorted_names: Vec<&String> = systems.keys().collect();
    sorted_names.sort();
    for name in sorted_names {
        let data = &systems[name];
        if generate_systems::need_folder(data, &config) {
            roms::create_folders(name, data, roms_dir_source, roms_dir_target);
            roms::write_info(name, data, roms_dir_target);
        } else {
            eprintln!("skipping directory for system {}", name);
        }
    }
}

/// Resolve one input file: the user's copy wins, the image's is the fallback.
fn resolve_input(user_dir: &Path, data_dir: &Path, name: &str) -> Option<PathBuf> {
    let user = user_dir.join(name);
    if user.is_file() {
        return Some(user);
    }
    let system = data_dir.join(name);
    if system.is_file() {
        return Some(system);
    }
    None
}

/// On-device regeneration mode. Manual only — nothing on the device invokes
/// this, at boot or otherwise.
///
/// Every input resolves `--user-dir` first, `--data-dir` second, so a stock
/// device regenerates byte-identically to its own build and the user overrides
/// only the file they actually edited. `--data-dir` holds the image's copies of
/// `es_systems.yml` / `es_features.yml` plus the two build facts a device cannot
/// reconstruct: `installed-packages.conf` (which BR2_PACKAGE_* symbols this
/// image was built with, i.e. the `requireAnyOf` gating) and `arch.conf`.
///
/// Output goes to /userdata, which every consumer already prefers over the
/// image copy: REG-Station's `SystemData::getConfigPath()` and
/// `CustomFeatures::loadFeatures()`, and regmsgd's `library::systems`.
fn regenerate_mode(args: &[String], format: OutputFormat) {
    let mut user_dir = PathBuf::from("/userdata/system/configs/es-system");
    let mut data_dir = PathBuf::from("/usr/share/es-system");
    let mut output_dir = PathBuf::from("/userdata/system/configs/regstation");
    let mut launcher_dir = PathBuf::from("/usr/share/regmsg/launcher");
    let mut arch = String::new();
    let mut config_override: Option<PathBuf> = None;

    let need_value = |i: usize| {
        if i >= args.len() {
            eprintln!("Missing value for {}", args[i - 1]);
            process::exit(1);
        }
    };

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--user-dir" => { i += 1; need_value(i); user_dir = PathBuf::from(&args[i]); }
            "--data-dir" => { i += 1; need_value(i); data_dir = PathBuf::from(&args[i]); }
            "--output-dir" => { i += 1; need_value(i); output_dir = PathBuf::from(&args[i]); }
            "--launcher-dir" => { i += 1; need_value(i); launcher_dir = PathBuf::from(&args[i]); }
            "--arch" => { i += 1; need_value(i); arch = args[i].clone(); }
            "--config" => { i += 1; need_value(i); config_override = Some(PathBuf::from(&args[i])); }
            _ => { eprintln!("Unknown option: {}", args[i]); process::exit(1); }
        }
        i += 1;
    }

    // Report a missing input plainly rather than letting load_yaml panic on it.
    let mut missing = Vec::new();
    let yml_path = resolve_input(&user_dir, &data_dir, "es_systems.yml");
    let features_path = resolve_input(&user_dir, &data_dir, "es_features.yml");
    if yml_path.is_none() {
        missing.push("es_systems.yml");
    }
    if features_path.is_none() {
        missing.push("es_features.yml");
    }
    if !missing.is_empty() {
        eprintln!("Missing input: {}", missing.join(", "));
        eprintln!("Looked in {} then {}", user_dir.display(), data_dir.display());
        process::exit(1);
    }
    let yml_path = yml_path.unwrap();
    let features_path = features_path.unwrap();

    // Which BR2_PACKAGE_* symbols this image was built with. Without it every
    // `requireAnyOf` gate fails closed and the result is an empty system list,
    // so refuse rather than write that out.
    let config_path = match config_override
        .or_else(|| resolve_input(&user_dir, &data_dir, "installed-packages.conf"))
    {
        Some(p) => p,
        None => {
            eprintln!("Missing installed-packages.conf in {} or {}", user_dir.display(), data_dir.display());
            eprintln!("Without it no emulator passes its requireAnyOf gate.");
            process::exit(1);
        }
    };

    // Auto-detect arch from arch.conf if not specified
    if arch.is_empty() {
        arch = resolve_input(&user_dir, &data_dir, "arch.conf")
            .and_then(|p| fs::read_to_string(p).ok())
            .unwrap_or_default()
            .trim()
            .to_string();
        if arch.is_empty() {
            eprintln!("No --arch given and no usable arch.conf in {} or {}", user_dir.display(), data_dir.display());
            process::exit(1);
        }
    }

    // Shared with regmsgd, which reads them from this same path
    // (`launcher::system_map`) — so they are read from the image, not shadowed
    // per-consumer, or the frontend and the launcher would disagree.
    let defaults_global = launcher_dir.join("launcher-defaults.yml");
    let defaults_arch = launcher_dir.join("launcher-defaults-arch.yml");

    // Systems file is always XML; only the features extension tracks the format.
    let feat_name = match format {
        OutputFormat::Xml => "rs_features.cfg",
        OutputFormat::Json => "rs_features.json",
    };
    let es_systems_out = output_dir.join("rs_systems.cfg");
    let es_features_out = output_dir.join(feat_name);

    if let Err(e) = fs::create_dir_all(&output_dir) {
        eprintln!("Cannot create {}: {}", output_dir.display(), e);
        process::exit(1);
    }

    eprintln!("systems:   {}", yml_path.display());
    eprintln!("features:  {}", features_path.display());
    eprintln!("packages:  {}", config_path.display());
    eprintln!("defaults:  {}", defaults_global.display());
    eprintln!("arch:      {}", arch);

    let systems: IndexMap<String, System> = load_yaml(&yml_path);
    let config = config::load_config(&config_path);
    let systems_config: IndexMap<String, SystemDefault> = load_yaml(&defaults_global);
    let arch_systems_config: IndexMap<String, SystemDefault> = load_yaml_or_empty(&defaults_arch);

    let features: IndexMap<String, EmulatorFeatures> = load_yaml_ordered(&features_path);

    // Systems: always XML (rs_systems.cfg) — REG-Station consumes XML only.
    eprintln!("regenerating {} ...", es_systems_out.display());
    let systems_xml = generate_systems::generate(&systems, &config, &systems_config, &arch_systems_config);
    generate_systems::write_file(&systems_xml, &es_systems_out);

    // Features: JSON (new default for REG-Station) or XML (legacy).
    eprintln!("regenerating {} ...", es_features_out.display());
    match format {
        OutputFormat::Xml => {
            let mut to_translate: IndexMap<String, Vec<generate_features::TranslationComment>> =
                IndexMap::new();
            let features_xml = generate_features::generate(&features, &arch, &mut to_translate);
            generate_features::write_file(&features_xml, &es_features_out);
        }
        OutputFormat::Json => {
            let features_json = generate_json::generate_features(&features, &arch);
            generate_json::write_json(&features_json, &es_features_out);
        }
    }

    eprintln!("done. Restart the frontend to pick them up.");
}

// ═══════════════════════════════════════════════════════════════════════════════
// YAML loading helpers
// ═══════════════════════════════════════════════════════════════════════════════

fn load_yaml<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
    serde_yaml::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e))
}

fn load_yaml_or_empty(path: &Path) -> IndexMap<String, SystemDefault> {
    if let Ok(content) = fs::read_to_string(path) {
        serde_yaml::from_str(&content).unwrap_or_default()
    } else {
        IndexMap::new()
    }
}

/// Load YAML preserving insertion order (using IndexMap via serde).
fn load_yaml_ordered(path: &Path) -> IndexMap<String, EmulatorFeatures> {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
    serde_yaml::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e))
}
