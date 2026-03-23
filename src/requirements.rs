use std::collections::HashSet;

use crate::models::{CustomFeature, RequirementItem};

/// Check if the requirements are met by the config.
/// Python: isValidRequirements()
///
/// Logic: empty requirements → true.
/// Each top-level item is OR'd. For RequirementItem::Group, any item in the
/// sublist matching config makes that group true, and returns true overall.
pub fn is_valid(config: &HashSet<String>, requirements: &[RequirementItem]) -> bool {
    if requirements.is_empty() {
        return true;
    }

    for req in requirements {
        match req {
            RequirementItem::Single(s) => {
                if config.contains(s) {
                    return true;
                }
            }
            RequirementItem::Group(group) => {
                for item in group {
                    if config.contains(item) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Check if a feature is valid for the given architecture.
/// Python: archValid()
pub fn arch_valid(arch: &str, feature: &CustomFeature) -> bool {
    if !feature.archs_exclude.is_empty() && feature.archs_exclude.contains(&arch.to_string()) {
        return false;
    }
    feature.archs_include.is_empty() || feature.archs_include.contains(&arch.to_string())
}
