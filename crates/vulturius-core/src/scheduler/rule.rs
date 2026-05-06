//! Auto-cleanup rules for scheduled cleanup.

use crate::types::FileCategory;
use serde::{Deserialize, Serialize};

/// A single auto-cleanup rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoCleanupRule {
    /// Rule name for identification.
    pub name: String,
    /// File category to match.
    pub category: FileCategory,
    /// Minimum file age in days before cleanup.
    pub min_age_days: u64,
    /// Maximum file size (0 = unlimited).
    pub max_size_bytes: u64,
    /// Whether to enable this rule.
    pub enabled: bool,
}

/// A set of cleanup rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupRuleSet {
    pub rules: Vec<AutoCleanupRule>,
}

impl Default for CleanupRuleSet {
    fn default() -> Self {
        Self {
            rules: vec![
                AutoCleanupRule {
                    name: "Temp files".to_string(),
                    category: FileCategory::Temp,
                    min_age_days: 7,
                    max_size_bytes: 0,
                    enabled: true,
                },
                AutoCleanupRule {
                    name: "Cache files".to_string(),
                    category: FileCategory::Cache,
                    min_age_days: 14,
                    max_size_bytes: 0,
                    enabled: true,
                },
                AutoCleanupRule {
                    name: "Log files".to_string(),
                    category: FileCategory::Log,
                    min_age_days: 30,
                    max_size_bytes: 100_000_000, // 100MB
                    enabled: true,
                },
                AutoCleanupRule {
                    name: "Crash dumps".to_string(),
                    category: FileCategory::CrashDump,
                    min_age_days: 7,
                    max_size_bytes: 0,
                    enabled: true,
                },
                AutoCleanupRule {
                    name: "Thumbnails".to_string(),
                    category: FileCategory::Thumbnail,
                    min_age_days: 3,
                    max_size_bytes: 0,
                    enabled: true,
                },
                AutoCleanupRule {
                    name: "Old backups".to_string(),
                    category: FileCategory::OldBackup,
                    min_age_days: 60,
                    max_size_bytes: 0,
                    enabled: false,
                },
            ],
        }
    }
}

impl CleanupRuleSet {
    /// Check if a file matches any enabled rule.
    pub fn matches(&self, category: &FileCategory, age_days: u64, size: u64) -> bool {
        self.rules.iter().any(|rule| {
            rule.enabled
                && rule.category == *category
                && age_days >= rule.min_age_days
                && (rule.max_size_bytes == 0 || size <= rule.max_size_bytes)
        })
    }

    /// Get all enabled rules.
    pub fn enabled_rules(&self) -> Vec<&AutoCleanupRule> {
        self.rules.iter().filter(|r| r.enabled).collect()
    }

    /// Enable a rule by name.
    pub fn enable_rule(&mut self, name: &str) {
        for rule in &mut self.rules {
            if rule.name == name {
                rule.enabled = true;
            }
        }
    }

    /// Disable a rule by name.
    pub fn disable_rule(&mut self, name: &str) {
        for rule in &mut self.rules {
            if rule.name == name {
                rule.enabled = false;
            }
        }
    }

    /// Add a new rule.
    pub fn add_rule(&mut self, rule: AutoCleanupRule) {
        self.rules.push(rule);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_rules() {
        let rules = CleanupRuleSet::default();
        assert_eq!(rules.rules.len(), 6);
        assert_eq!(rules.enabled_rules().len(), 5); // old backups disabled
    }

    #[test]
    fn test_matches_temp_file() {
        let rules = CleanupRuleSet::default();
        assert!(rules.matches(&FileCategory::Temp, 10, 1000));
        assert!(!rules.matches(&FileCategory::Temp, 3, 1000)); // too new
    }

    #[test]
    fn test_matches_log_with_size_limit() {
        let rules = CleanupRuleSet::default();
        assert!(rules.matches(&FileCategory::Log, 31, 50_000_000)); // under limit
        assert!(!rules.matches(&FileCategory::Log, 31, 200_000_000)); // over limit
    }

    #[test]
    fn test_enable_disable_rule() {
        let mut rules = CleanupRuleSet::default();
        rules.disable_rule("Temp files");
        assert!(!rules.matches(&FileCategory::Temp, 10, 1000));
        rules.enable_rule("Temp files");
        assert!(rules.matches(&FileCategory::Temp, 10, 1000));
    }

    #[test]
    fn test_add_rule() {
        let mut rules = CleanupRuleSet::default();
        rules.add_rule(AutoCleanupRule {
            name: "Custom".to_string(),
            category: FileCategory::Unknown,
            min_age_days: 90,
            max_size_bytes: 0,
            enabled: true,
        });
        assert_eq!(rules.rules.len(), 7);
    }
}
