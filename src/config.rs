use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Load a buildroot .config file, collecting all keys where value is "y".
/// Matches Python: `re.search("^([^ ]+)=y$", line)`
pub fn load_config(path: &Path) -> HashSet<String> {
    let mut config = HashSet::new();
    if let Ok(content) = fs::read_to_string(path) {
        for line in content.lines() {
            let line = line.trim();
            if !line.contains(' ') && line.ends_with("=y") {
                if let Some(key) = line.strip_suffix("=y") {
                    if !key.is_empty() {
                        config.insert(key.to_string());
                    }
                }
            }
        }
    }
    config
}
