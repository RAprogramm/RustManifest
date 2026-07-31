// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Configuration loader and resolver for `rustmanifest`.
//!
//! Parses `rustmanifest.toml` into the canonical
//! [`rustmanifest_schema::Config`] shape, resolves the selected [`Profile`],
//! and exposes the combined result as [`Effective`], which answers the two
//! questions the engine needs per rule: does it run, and with what severity.

use std::{
    io,
    path::{Path, PathBuf},
    str::FromStr
};

pub use rustmanifest_schema::{Config, RuleOverride};
use rustmanifest_schema::{Rule, Severity};
use thiserror::Error;

/// File name probed in the current directory when no explicit path is given.
pub const DEFAULT_FILE_NAME: &str = "rustmanifest.toml";

/// Failures produced while loading or resolving configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The configuration file exists but could not be read from disk.
    #[error("cannot read config file {path}: {source}")]
    Read {
        /// Path of the file that failed to read.
        path:   PathBuf,
        /// Underlying IO error.
        #[source]
        source: io::Error
    },
    /// The configuration file was read but is not valid `rustmanifest.toml`.
    #[error("cannot parse config file {path}: {message}")]
    Parse {
        /// Path of the file that failed to parse.
        path:    PathBuf,
        /// Message reported by the TOML parser.
        message: String
    },
    /// The `profile` key names a profile that does not exist.
    #[error("unknown profile {name:?}; valid profiles: strict, default, minimal, ci")]
    UnknownProfile {
        /// The unrecognized profile name.
        name: String
    }
}

/// Loads configuration from `path`, or from `rustmanifest.toml` in the
/// current directory when `path` is `None`.
///
/// A missing default file yields [`Config::default`]; an explicitly given
/// path must exist and parse.
///
/// # Errors
///
/// Returns [`ConfigError::Read`] when the file cannot be read and
/// [`ConfigError::Parse`] when its contents are not valid configuration.
pub fn load(path: Option<&Path>) -> Result<Config, ConfigError> {
    path.map_or_else(
        || {
            let default_path = Path::new(DEFAULT_FILE_NAME);
            if default_path.exists() {
                parse_file(default_path)
            } else {
                Ok(Config::default())
            }
        },
        parse_file
    )
}

fn parse_file(path: &Path) -> Result<Config, ConfigError> {
    let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source
    })?;
    toml::from_str(&text).map_err(|error| ConfigError::Parse {
        path:    path.to_path_buf(),
        message: error.to_string()
    })
}

/// Built-in analysis profile selecting which rules run and which severities
/// affect the exit code.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Profile {
    /// Every rule runs with its authored severity.
    Strict,
    /// Currently identical to [`Profile::Strict`].
    #[default]
    Default,
    /// Only `error`-severity rules run.
    Minimal,
    /// Every rule runs; only `warning` and above affect the exit code.
    Ci
}

impl Profile {
    /// Names accepted by [`Profile::from_str`], in declaration order.
    pub const NAMES: [&'static str; 4] = ["strict", "default", "minimal", "ci"];

    /// Whether a rule of the given effective severity runs under this
    /// profile.
    #[must_use]
    pub const fn admits(self, severity: Severity) -> bool {
        match self {
            Self::Strict | Self::Default | Self::Ci => true,
            Self::Minimal => matches!(severity, Severity::Error)
        }
    }

    /// Minimum severity a finding must reach to influence the exit code.
    #[must_use]
    pub const fn exit_threshold(self) -> Severity {
        match self {
            Self::Strict | Self::Default | Self::Minimal => Severity::Hint,
            Self::Ci => Severity::Warning
        }
    }
}

impl FromStr for Profile {
    type Err = ConfigError;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "strict" => Ok(Self::Strict),
            "default" => Ok(Self::Default),
            "minimal" => Ok(Self::Minimal),
            "ci" => Ok(Self::Ci),
            other => Err(ConfigError::UnknownProfile {
                name: other.to_owned()
            })
        }
    }
}

/// A [`Config`] combined with its resolved [`Profile`].
///
/// This is the read-side view the engine consumes: per-rule enablement,
/// per-rule effective severity, and file globs.
#[derive(Debug, Clone)]
pub struct Effective {
    config:  Config,
    profile: Profile
}

impl Effective {
    /// Resolves the profile named by `config.profile` and pairs it with the
    /// configuration.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::UnknownProfile`] when `config.profile` names
    /// no built-in profile.
    pub fn from_config(config: Config) -> Result<Self, ConfigError> {
        let profile = config
            .profile
            .as_deref()
            .map_or_else(|| Ok(Profile::default()), Profile::from_str)?;
        Ok(Self {
            config,
            profile
        })
    }

    /// The resolved profile.
    #[must_use]
    pub const fn profile(&self) -> Profile {
        self.profile
    }

    /// Effective severity for `rule`: a matching entry in
    /// [`Config::overrides`] wins over the authored severity.
    #[must_use]
    pub fn severity_for(&self, rule: &Rule) -> Severity {
        self.config
            .overrides
            .iter()
            .find(|entry| entry.rule_id == rule.id)
            .map_or(rule.severity, |entry| entry.severity)
    }

    /// Whether `rule` runs under the resolved profile, judged by its
    /// effective severity.
    #[must_use]
    pub fn rule_enabled(&self, rule: &Rule) -> bool {
        self.profile.admits(self.severity_for(rule))
    }

    /// Glob patterns of files to analyze; empty means analyze everything.
    #[must_use]
    pub fn include(&self) -> &[String] {
        &self.config.include
    }

    /// Glob patterns of files to skip.
    #[must_use]
    pub fn exclude(&self) -> &[String] {
        &self.config.exclude
    }

    /// User-declared behavioral invariants to enforce.
    #[must_use]
    pub fn invariants(&self) -> &[rustmanifest_schema::Invariant] {
        &self.config.invariants
    }

    /// Minimum severity a finding must reach to influence the exit code.
    #[must_use]
    pub const fn exit_threshold(&self) -> Severity {
        self.profile.exit_threshold()
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::io::Write;

    use rustmanifest_schema::RuleDefinition;

    use super::*;

    fn rule(id: &str, severity: Severity) -> Rule {
        Rule {
            id: id.to_owned(),
            severity,
            title: format!("{id} title"),
            rationale_uri: "rustmanifest://methodology/structure".to_owned(),
            definition: RuleDefinition::Pattern {
                regex:         "x".to_owned(),
                exclude_globs: Vec::new()
            }
        }
    }

    fn effective(config: Config) -> Effective {
        Effective::from_config(config).unwrap()
    }

    #[test]
    fn load_without_path_and_without_default_file_yields_default() {
        let config = load(None).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn load_missing_explicit_path_reports_read_error_with_path() {
        let error = load(Some(Path::new("/nonexistent/rustmanifest.toml"))).unwrap_err();
        assert!(matches!(error, ConfigError::Read { .. }));
        assert!(error.to_string().contains("/nonexistent/rustmanifest.toml"));
    }

    #[test]
    fn load_parses_profile_overrides_and_globs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rustmanifest.toml");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(
            concat!(
                "profile = \"ci\"\n",
                "include = [\"src/**\"]\n",
                "exclude = [\"**/generated.rs\"]\n",
                "[[overrides]]\n",
                "rule-id = \"RM-SEC-001\"\n",
                "severity = \"warning\"\n"
            )
            .as_bytes()
        )
        .unwrap();
        let config = load(Some(&path)).unwrap();
        assert_eq!(config.profile.as_deref(), Some("ci"));
        assert_eq!(config.include, vec!["src/**".to_owned()]);
        assert_eq!(config.exclude, vec!["**/generated.rs".to_owned()]);
        assert_eq!(config.overrides.len(), 1);
        assert_eq!(config.overrides[0].severity, Severity::Warning);
    }

    #[test]
    fn load_invalid_toml_reports_parse_error_with_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rustmanifest.toml");
        std::fs::write(&path, "profile = [broken").unwrap();
        let error = load(Some(&path)).unwrap_err();
        assert!(matches!(error, ConfigError::Parse { .. }));
        assert!(error.to_string().contains("rustmanifest.toml"));
    }

    #[test]
    fn profile_parses_every_known_name() {
        for name in Profile::NAMES {
            assert!(Profile::from_str(name).is_ok(), "profile {name} must parse");
        }
        assert_eq!(Profile::from_str("strict").unwrap(), Profile::Strict);
        assert_eq!(Profile::from_str("ci").unwrap(), Profile::Ci);
    }

    #[test]
    fn unknown_profile_is_rejected_with_valid_names_listed() {
        let error = Profile::from_str("paranoid").unwrap_err();
        let message = error.to_string();
        assert!(message.contains("paranoid"));
        for name in Profile::NAMES {
            assert!(message.contains(name), "message must list {name}");
        }
    }

    #[test]
    fn unknown_profile_in_config_fails_resolution() {
        let config = Config {
            profile: Some("paranoid".to_owned()),
            ..Config::default()
        };
        assert!(matches!(
            Effective::from_config(config),
            Err(ConfigError::UnknownProfile { .. })
        ));
    }

    #[test]
    fn override_changes_effective_severity() {
        let config = Config {
            overrides: vec![RuleOverride {
                rule_id:  "RM-SEC-001".to_owned(),
                severity: Severity::Hint
            }],
            ..Config::default()
        };
        let effective = effective(config);
        assert_eq!(
            effective.severity_for(&rule("RM-SEC-001", Severity::Error)),
            Severity::Hint
        );
        assert_eq!(
            effective.severity_for(&rule("RM-SEC-002", Severity::Error)),
            Severity::Error
        );
    }

    #[test]
    fn minimal_profile_runs_only_error_severity_rules() {
        let config = Config {
            profile: Some("minimal".to_owned()),
            ..Config::default()
        };
        let effective = effective(config);
        assert!(effective.rule_enabled(&rule("RM-SEC-001", Severity::Error)));
        assert!(!effective.rule_enabled(&rule("RM-STRUCT-001", Severity::Warning)));
        assert!(!effective.rule_enabled(&rule("RM-STRUCT-002", Severity::Info)));
        assert!(!effective.rule_enabled(&rule("RM-STRUCT-003", Severity::Hint)));
    }

    #[test]
    fn minimal_profile_honors_override_when_deciding_enablement() {
        let config = Config {
            profile: Some("minimal".to_owned()),
            overrides: vec![RuleOverride {
                rule_id:  "RM-STRUCT-001".to_owned(),
                severity: Severity::Error
            }],
            ..Config::default()
        };
        let effective = effective(config);
        assert!(effective.rule_enabled(&rule("RM-STRUCT-001", Severity::Warning)));
    }

    #[test]
    fn ci_profile_runs_everything_with_warning_exit_threshold() {
        let config = Config {
            profile: Some("ci".to_owned()),
            ..Config::default()
        };
        let effective = effective(config);
        assert!(effective.rule_enabled(&rule("RM-STRUCT-003", Severity::Hint)));
        assert_eq!(effective.exit_threshold(), Severity::Warning);
    }

    #[test]
    fn strict_and_default_profiles_behave_identically() {
        for name in ["strict", "default"] {
            let config = Config {
                profile: Some(name.to_owned()),
                ..Config::default()
            };
            let effective = effective(config);
            assert!(effective.rule_enabled(&rule("RM-STRUCT-003", Severity::Hint)));
            assert_eq!(effective.exit_threshold(), Severity::Hint);
        }
    }
}
