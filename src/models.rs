use indexmap::IndexMap;
use serde::Deserialize;

// ═══════════════════════════════════════════════════════════════════════════════
// es_systems.yml
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct System {
    pub name: String,
    pub manufacturer: String,
    #[serde(default)]
    pub release: serde_yaml::Value,
    pub hardware: String,
    #[serde(default)]
    pub extensions: Vec<String>,
    pub platform: Option<String>,
    pub theme: Option<String>,
    pub group: Option<String>,
    pub path: Option<String>,
    pub command: Option<String>,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub emulators: IndexMap<String, IndexMap<String, CoreReq>>,
    pub comment_en: Option<String>,
    pub comment_fr: Option<String>,
    pub comment_br: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CoreReq {
    #[serde(rename = "requireAnyOf", default)]
    pub require_any_of: Vec<RequirementItem>,
    #[serde(default)]
    pub incompatible_extensions: Vec<String>,
}

/// requireAnyOf items can be either a plain string or a nested list of strings.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum RequirementItem {
    Single(String),
    Group(Vec<String>),
}

// ═══════════════════════════════════════════════════════════════════════════════
// es_features.yml
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize, Default)]
pub struct EmulatorFeatures {
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub shared: Vec<String>,
    #[serde(default)]
    pub cfeatures: IndexMap<String, CustomFeature>,
    #[serde(default)]
    pub cores: IndexMap<String, CoreFeatures>,
    #[serde(default)]
    pub systems: IndexMap<String, SystemFeatures>,
}

#[derive(Debug, Deserialize, Default)]
pub struct CoreFeatures {
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub shared: Vec<String>,
    #[serde(default)]
    pub cfeatures: IndexMap<String, CustomFeature>,
    #[serde(default)]
    pub systems: IndexMap<String, SystemFeatures>,
}

#[derive(Debug, Deserialize, Default)]
pub struct SystemFeatures {
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub shared: Vec<String>,
    #[serde(default)]
    pub cfeatures: IndexMap<String, CustomFeature>,
}

#[derive(Debug, Deserialize)]
pub struct CustomFeature {
    pub prompt: String,
    pub description: Option<String>,
    pub group: Option<String>,
    pub submenu: Option<String>,
    pub order: Option<serde_yaml::Value>,
    pub preset: Option<String>,
    pub preset_parameters: Option<String>,
    #[serde(default)]
    pub choices: IndexMap<String, serde_yaml::Value>,
    #[serde(default)]
    pub archs_include: Vec<String>,
    #[serde(default)]
    pub archs_exclude: Vec<String>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// configgen-defaults.yml
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize, Default)]
pub struct SystemDefault {
    pub emulator: Option<String>,
    pub core: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Convert a serde_yaml::Value to a display string matching Python's str() behavior.
///
/// IMPORTANT: serde_yaml (libyaml) follows YAML 1.1 spec which treats
/// yes/no/on/off/true/false/True/False as booleans. Python's PyYAML does the same,
/// so str(True) = "True", str(False) = "False". Similarly, YAML 1.1 sexagesimal
/// notation means 4:3 → 243 (4*60+3). We match Python/PyYAML output exactly.
pub fn yaml_value_to_string(v: &serde_yaml::Value) -> String {
    match v {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Number(n) => {
            let s = format!("{}", n);
            if s.contains('.') || (n.as_f64().is_some() && n.as_i64().is_none()) {
                if let Some(f) = n.as_f64() {
                    if f.fract() == 0.0 {
                        format!("{:.1}", f)
                    } else {
                        format!("{}", f)
                    }
                } else {
                    s
                }
            } else if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(f) = n.as_f64() {
                format!("{}", f)
            } else {
                s
            }
        }
        serde_yaml::Value::Bool(b) => {
            // Python str(True) = "True", str(False) = "False"
            if *b { "True".to_string() } else { "False".to_string() }
        }
        serde_yaml::Value::Null => "None".to_string(),
        _ => format!("{:?}", v),
    }
}
