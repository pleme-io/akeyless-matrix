use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use toml_edit::DocumentMut;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pending,
    Building,
    Verified,
    Broken,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Building => write!(f, "building"),
            Self::Verified => write!(f, "verified"),
            Self::Broken => write!(f, "broken"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Go,
    Rust,
    Python,
    #[serde(rename = "typescript")]
    TypeScript,
    Java,
    Ruby,
    Php,
    Csharp,
    Helm,
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Go => write!(f, "go"),
            Self::Rust => write!(f, "rust"),
            Self::Python => write!(f, "python"),
            Self::TypeScript => write!(f, "typescript"),
            Self::Java => write!(f, "java"),
            Self::Ruby => write!(f, "ruby"),
            Self::Php => write!(f, "php"),
            Self::Csharp => write!(f, "csharp"),
            Self::Helm => write!(f, "helm"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Builder {
    #[serde(rename = "mkGoTool")]
    MkGoTool,
    #[serde(rename = "mkGoLibraryCheck")]
    MkGoLibraryCheck,
    #[serde(rename = "buildRustPackage")]
    BuildRustPackage,
    #[serde(rename = "mkPythonPackage")]
    MkPythonPackage,
    #[serde(rename = "buildNpmPackage")]
    BuildNpmPackage,
    #[serde(rename = "fetchurl")]
    Fetchurl,
    #[serde(rename = "mkJavaMavenPackage")]
    MkJavaMavenPackage,
    #[serde(rename = "mkDotnetPackage")]
    MkDotnetPackage,
    #[serde(rename = "mkTerraformModuleCheck")]
    MkTerraformModuleCheck,
    #[serde(rename = "none")]
    None,
}

impl fmt::Display for Builder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MkGoTool => write!(f, "mkGoTool"),
            Self::MkGoLibraryCheck => write!(f, "mkGoLibraryCheck"),
            Self::BuildRustPackage => write!(f, "buildRustPackage"),
            Self::MkPythonPackage => write!(f, "mkPythonPackage"),
            Self::BuildNpmPackage => write!(f, "buildNpmPackage"),
            Self::Fetchurl => write!(f, "fetchurl"),
            Self::MkJavaMavenPackage => write!(f, "mkJavaMavenPackage"),
            Self::MkDotnetPackage => write!(f, "mkDotnetPackage"),
            Self::MkTerraformModuleCheck => write!(f, "mkTerraformModuleCheck"),
            Self::None => write!(f, "none"),
        }
    }
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionEntry {
    pub rev: String,
    pub source_hash: Option<String>,
    pub vendor_hash: Option<String>,
    pub cargo_hash: Option<String>,
    pub npm_deps_hash: Option<String>,
    /// Maven repository hash for Java/Maven packages
    #[serde(default)]
    pub maven_hash: Option<String>,
    /// .NET NuGet dependencies hash for C#/.NET packages
    #[serde(default)]
    pub nuget_deps_hash: Option<String>,
    pub status: Status,
    pub verified_at: Option<DateTime<Utc>>,
    /// Per-platform hashes for fetchurl packages
    #[serde(default)]
    pub hash_aarch64_darwin: Option<String>,
    #[serde(default)]
    pub hash_x86_64_darwin: Option<String>,
    #[serde(default)]
    pub hash_x86_64_linux: Option<String>,
    #[serde(default)]
    pub hash_aarch64_linux: Option<String>,
}

/// How the watch daemon detects new versions for this package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TrackMode {
    /// Track semver tags (e.g., v1.0.0 → version "1.0.0"). Default.
    #[default]
    Tags,
    /// Track HEAD commits. Version = "{unstable_base}-unstable.{date}".
    Commits,
    /// Track binary URL hash changes (closed-source prebuilt binaries).
    Binary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub owner: String,
    pub repo: String,
    pub language: Language,
    pub builder: Builder,
    pub tier: u8,
    /// How to detect new versions: "tags" (default) or "commits".
    #[serde(default)]
    pub track: TrackMode,
    /// Base version for commit-tracked unstable builds (e.g., "0.1.0").
    /// Used to generate "0.1.0-unstable.2026-03-14" versions.
    #[serde(default)]
    pub unstable_base: Option<String>,
    #[serde(default)]
    pub sub_packages: Option<Vec<String>>,
    #[serde(default)]
    pub proxy_vendor: Option<bool>,
    #[serde(default)]
    pub license: Option<String>,
    pub description: String,
    pub homepage: String,
    #[serde(default)]
    pub fork_of: Option<String>,
    #[serde(default)]
    pub fork_reason: Option<String>,
    #[serde(default)]
    pub native_build_inputs: Option<Vec<String>>,
    #[serde(default)]
    pub python_deps: Option<Vec<String>>,
    #[serde(default)]
    pub pname_override: Option<String>,
    #[serde(default)]
    pub dont_npm_build: Option<bool>,
    #[serde(default)]
    pub extra_post_install: Option<String>,
    /// Binary name for fetchurl packages (e.g., "akeyless")
    #[serde(default)]
    pub binary_name: Option<String>,
    /// Per-platform download URLs for fetchurl packages
    #[serde(default)]
    pub platform_urls: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub versions: BTreeMap<String, VersionEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Matrix {
    pub packages: BTreeMap<String, Package>,
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl Matrix {
    /// Parse a Matrix from a TOML string (useful for testing).
    pub fn from_str(content: &str) -> Result<Self> {
        toml::from_str(content).context("parsing matrix TOML")
    }

    /// Load and parse `matrix.toml` from the given path.
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        let matrix: Self =
            toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;
        Ok(matrix)
    }

    /// Write modifications back to the file using `toml_edit` to preserve
    /// comments and formatting as much as possible.
    pub fn save_to_path(path: &Path, matrix: &Self) -> Result<()> {
        // Read the existing document (or start fresh)
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        let mut doc: DocumentMut = existing
            .parse::<DocumentMut>()
            .with_context(|| format!("parsing {} as toml_edit document", path.display()))?;

        // Ensure [packages] table exists
        if doc.get("packages").is_none() {
            doc["packages"] = toml_edit::Item::Table(toml_edit::Table::new());
        }

        let packages_table = doc["packages"]
            .as_table_mut()
            .context("packages is not a table")?;

        for (pkg_name, pkg) in &matrix.packages {
            // Ensure the package table exists
            if packages_table.get(pkg_name).is_none() {
                packages_table[pkg_name] = toml_edit::Item::Table(toml_edit::Table::new());
            }
            let pkg_table = packages_table[pkg_name]
                .as_table_mut()
                .with_context(|| format!("{pkg_name} is not a table"))?;

            // Update scalar fields
            pkg_table["owner"] = toml_edit::value(&pkg.owner);
            pkg_table["repo"] = toml_edit::value(&pkg.repo);
            pkg_table["language"] = toml_edit::value(pkg.language.to_string());
            pkg_table["builder"] = toml_edit::value(pkg.builder.to_string());
            pkg_table["tier"] = toml_edit::value(i64::from(pkg.tier));
            pkg_table["description"] = toml_edit::value(&pkg.description);
            pkg_table["homepage"] = toml_edit::value(&pkg.homepage);

            // Optional fields
            set_optional_string(pkg_table, "license", &pkg.license);
            set_optional_string(pkg_table, "fork_of", &pkg.fork_of);
            set_optional_string(pkg_table, "fork_reason", &pkg.fork_reason);
            set_optional_string(pkg_table, "pname_override", &pkg.pname_override);
            set_optional_string(pkg_table, "extra_post_install", &pkg.extra_post_install);
            set_optional_bool(pkg_table, "proxy_vendor", pkg.proxy_vendor);
            set_optional_bool(pkg_table, "dont_npm_build", pkg.dont_npm_build);
            set_optional_string(pkg_table, "binary_name", &pkg.binary_name);
            set_optional_vec(pkg_table, "sub_packages", &pkg.sub_packages);
            set_optional_vec(pkg_table, "native_build_inputs", &pkg.native_build_inputs);
            set_optional_vec(pkg_table, "python_deps", &pkg.python_deps);

            // platform_urls sub-table
            if let Some(ref urls) = pkg.platform_urls {
                if pkg_table.get("platform_urls").is_none() {
                    pkg_table["platform_urls"] = toml_edit::Item::Table(toml_edit::Table::new());
                }
                let urls_table = pkg_table["platform_urls"]
                    .as_table_mut()
                    .unwrap();
                for (platform, url) in urls {
                    urls_table[platform.as_str()] = toml_edit::value(url.as_str());
                }
            }

            // Versions sub-table
            if pkg_table.get("versions").is_none() {
                pkg_table["versions"] = toml_edit::Item::Table(toml_edit::Table::new());
            }
            let versions_table = pkg_table["versions"]
                .as_table_mut()
                .with_context(|| format!("{pkg_name}.versions is not a table"))?;

            for (ver_key, ver) in &pkg.versions {
                if versions_table.get(ver_key).is_none() {
                    versions_table[ver_key] = toml_edit::Item::Table(toml_edit::Table::new());
                }
                let ver_table = versions_table[ver_key]
                    .as_table_mut()
                    .with_context(|| format!("{pkg_name}.versions.{ver_key} is not a table"))?;

                ver_table["rev"] = toml_edit::value(&ver.rev);
                ver_table["status"] = toml_edit::value(ver.status.to_string());

                if let Some(ref h) = ver.source_hash {
                    ver_table["source_hash"] = toml_edit::value(h.as_str());
                }
                if let Some(ref h) = ver.vendor_hash {
                    ver_table["vendor_hash"] = toml_edit::value(h.as_str());
                }
                if let Some(ref h) = ver.cargo_hash {
                    ver_table["cargo_hash"] = toml_edit::value(h.as_str());
                }
                if let Some(ref h) = ver.npm_deps_hash {
                    ver_table["npm_deps_hash"] = toml_edit::value(h.as_str());
                }
                if let Some(ref h) = ver.maven_hash {
                    ver_table["maven_hash"] = toml_edit::value(h.as_str());
                }
                if let Some(ref h) = ver.nuget_deps_hash {
                    ver_table["nuget_deps_hash"] = toml_edit::value(h.as_str());
                }
                if let Some(ref h) = ver.hash_aarch64_darwin {
                    ver_table["hash_aarch64_darwin"] = toml_edit::value(h.as_str());
                }
                if let Some(ref h) = ver.hash_x86_64_darwin {
                    ver_table["hash_x86_64_darwin"] = toml_edit::value(h.as_str());
                }
                if let Some(ref h) = ver.hash_x86_64_linux {
                    ver_table["hash_x86_64_linux"] = toml_edit::value(h.as_str());
                }
                if let Some(ref h) = ver.hash_aarch64_linux {
                    ver_table["hash_aarch64_linux"] = toml_edit::value(h.as_str());
                }
                if let Some(ts) = ver.verified_at {
                    ver_table["verified_at"] = toml_edit::value(ts.to_rfc3339());
                }
            }
        }

        std::fs::write(path, doc.to_string())
            .with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    /// Return the latest verified version entry for a package (highest version
    /// key by string ordering, filtered to `Verified` status).
    #[must_use]
    pub fn latest_verified(pkg: &Package) -> Option<(&str, &VersionEntry)> {
        pkg.versions
            .iter()
            .filter(|(_, v)| v.status == Status::Verified)
            .last()
            .map(|(k, v)| (k.as_str(), v))
    }

    /// Derive the source key used in `sources.nix` from a package name.
    /// Strips the `akeyless-` prefix.
    #[must_use]
    pub fn source_key(pkg_name: &str) -> String {
        pkg_name
            .strip_prefix("akeyless-")
            .unwrap_or(pkg_name)
            .to_string()
    }

    /// Sanitize a version string for use in Nix attribute names.
    /// "0.6.1" -> "0_6_1", "0.1.0-pleme.1" -> "0_1_0-pleme_1"
    #[must_use]
    pub fn sanitize_version(version: &str) -> String {
        version.replace('.', "_")
    }

    /// Return ALL verified version entries for a package, ordered by version key.
    #[must_use]
    pub fn all_verified(pkg: &Package) -> Vec<(&str, &VersionEntry)> {
        pkg.versions
            .iter()
            .filter(|(_, v)| v.status == Status::Verified)
            .map(|(k, v)| (k.as_str(), v))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Helpers for toml_edit optional fields
// ---------------------------------------------------------------------------

fn set_optional_string(
    table: &mut toml_edit::Table,
    key: &str,
    value: &Option<String>,
) {
    if let Some(v) = value {
        table[key] = toml_edit::value(v.as_str());
    }
}

fn set_optional_bool(
    table: &mut toml_edit::Table,
    key: &str,
    value: Option<bool>,
) {
    if let Some(v) = value {
        table[key] = toml_edit::value(v);
    }
}

fn set_optional_vec(
    table: &mut toml_edit::Table,
    key: &str,
    value: &Option<Vec<String>>,
) {
    if let Some(items) = value {
        let mut arr = toml_edit::Array::new();
        for item in items {
            arr.push(item.as_str());
        }
        table[key] = toml_edit::value(arr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_key() {
        assert_eq!(Matrix::source_key("akeyless-cli"), "cli");
        assert_eq!(Matrix::source_key("akeyless-go-sdk"), "go-sdk");
        assert_eq!(
            Matrix::source_key("akeyless-terraform-provider"),
            "terraform-provider"
        );
        assert_eq!(Matrix::source_key("unknown"), "unknown");
    }

    #[test]
    fn test_from_str_minimal() {
        let toml = r#"
[packages.akeyless-test]
owner = "test-org"
repo = "test-repo"
language = "go"
builder = "mkGoTool"
tier = 1
description = "Test package"
homepage = "https://example.com"

[packages.akeyless-test.versions."1.0.0"]
rev = "abc123"
source_hash = "sha256-test"
vendor_hash = "sha256-vendor"
status = "verified"
"#;
        let matrix = Matrix::from_str(toml).unwrap();
        assert_eq!(matrix.packages.len(), 1);
        let pkg = &matrix.packages["akeyless-test"];
        assert_eq!(pkg.owner, "test-org");
        assert_eq!(pkg.language, Language::Go);
        assert_eq!(pkg.versions.len(), 1);
        let ver = &pkg.versions["1.0.0"];
        assert_eq!(ver.status, Status::Verified);
    }

    #[test]
    fn test_latest_verified() {
        let mut versions = BTreeMap::new();
        versions.insert(
            "1.0.0".to_string(),
            VersionEntry {
                rev: "aaa".into(),
                source_hash: Some("sha256-a".into()),
                vendor_hash: None,
                cargo_hash: None,
                npm_deps_hash: None,
                maven_hash: None,
                nuget_deps_hash: None,
                status: Status::Verified,
                verified_at: None,
                hash_aarch64_darwin: None,
                hash_x86_64_darwin: None,
                hash_x86_64_linux: None,
                hash_aarch64_linux: None,
            },
        );
        versions.insert(
            "2.0.0".to_string(),
            VersionEntry {
                rev: "bbb".into(),
                source_hash: Some("sha256-b".into()),
                vendor_hash: None,
                cargo_hash: None,
                npm_deps_hash: None,
                maven_hash: None,
                nuget_deps_hash: None,
                status: Status::Pending,
                verified_at: None,
                hash_aarch64_darwin: None,
                hash_x86_64_darwin: None,
                hash_x86_64_linux: None,
                hash_aarch64_linux: None,
            },
        );
        versions.insert(
            "1.5.0".to_string(),
            VersionEntry {
                rev: "ccc".into(),
                source_hash: Some("sha256-c".into()),
                vendor_hash: None,
                cargo_hash: None,
                npm_deps_hash: None,
                maven_hash: None,
                nuget_deps_hash: None,
                status: Status::Verified,
                verified_at: None,
                hash_aarch64_darwin: None,
                hash_x86_64_darwin: None,
                hash_x86_64_linux: None,
                hash_aarch64_linux: None,
            },
        );

        let pkg = Package {
            owner: "t".into(),
            repo: "t".into(),
            language: Language::Go,
            builder: Builder::MkGoTool,
            tier: 1,
            sub_packages: None,
            proxy_vendor: None,
            license: None,
            description: "t".into(),
            homepage: "t".into(),
            fork_of: None,
            fork_reason: None,
            native_build_inputs: None,
            python_deps: None,
            pname_override: None,
            dont_npm_build: None,
            extra_post_install: None,
            binary_name: None,
            platform_urls: None,
            track: TrackMode::default(),
            unstable_base: None,
            versions,
        };

        let (ver, entry) = Matrix::latest_verified(&pkg).unwrap();
        // BTreeMap ordered: 1.0.0, 1.5.0, 2.0.0 -- last verified is 1.5.0
        assert_eq!(ver, "1.5.0");
        assert_eq!(entry.rev, "ccc");
    }

    #[test]
    fn test_sanitize_version() {
        assert_eq!(Matrix::sanitize_version("0.6.1"), "0_6_1");
        assert_eq!(Matrix::sanitize_version("0.1.0-pleme.1"), "0_1_0-pleme_1");
        assert_eq!(Matrix::sanitize_version("5.0.22"), "5_0_22");
    }

    #[test]
    fn test_all_verified() {
        let mut versions = BTreeMap::new();
        versions.insert(
            "1.0.0".to_string(),
            VersionEntry {
                rev: "aaa".into(),
                source_hash: Some("sha256-a".into()),
                vendor_hash: None,
                cargo_hash: None,
                npm_deps_hash: None,
                maven_hash: None,
                nuget_deps_hash: None,
                status: Status::Verified,
                verified_at: None,
                hash_aarch64_darwin: None,
                hash_x86_64_darwin: None,
                hash_x86_64_linux: None,
                hash_aarch64_linux: None,
            },
        );
        versions.insert(
            "2.0.0".to_string(),
            VersionEntry {
                rev: "bbb".into(),
                source_hash: Some("sha256-b".into()),
                vendor_hash: None,
                cargo_hash: None,
                npm_deps_hash: None,
                maven_hash: None,
                nuget_deps_hash: None,
                status: Status::Pending,
                verified_at: None,
                hash_aarch64_darwin: None,
                hash_x86_64_darwin: None,
                hash_x86_64_linux: None,
                hash_aarch64_linux: None,
            },
        );
        versions.insert(
            "1.5.0".to_string(),
            VersionEntry {
                rev: "ccc".into(),
                source_hash: Some("sha256-c".into()),
                vendor_hash: None,
                cargo_hash: None,
                npm_deps_hash: None,
                maven_hash: None,
                nuget_deps_hash: None,
                status: Status::Verified,
                verified_at: None,
                hash_aarch64_darwin: None,
                hash_x86_64_darwin: None,
                hash_x86_64_linux: None,
                hash_aarch64_linux: None,
            },
        );

        let pkg = Package {
            owner: "t".into(),
            repo: "t".into(),
            language: Language::Go,
            builder: Builder::MkGoTool,
            tier: 1,
            sub_packages: None,
            proxy_vendor: None,
            license: None,
            description: "t".into(),
            homepage: "t".into(),
            fork_of: None,
            fork_reason: None,
            native_build_inputs: None,
            python_deps: None,
            pname_override: None,
            dont_npm_build: None,
            extra_post_install: None,
            binary_name: None,
            platform_urls: None,
            track: TrackMode::default(),
            unstable_base: None,
            versions,
        };

        let all = Matrix::all_verified(&pkg);
        assert_eq!(all.len(), 2); // 1.0.0 and 1.5.0 (2.0.0 is pending)
        assert_eq!(all[0].0, "1.0.0");
        assert_eq!(all[1].0, "1.5.0");
    }
}

/// Test helpers available to all modules via `crate::matrix::test_helpers`.
#[cfg(test)]
pub mod test_helpers {
    use super::*;

    /// Create a minimal Package with sensible defaults. Override fields as needed.
    #[must_use]
    pub fn pkg(language: Language, builder: Builder) -> Package {
        Package {
            owner: "testorg".into(),
            repo: "testrepo".into(),
            language,
            builder,
            tier: 1,
            track: TrackMode::default(),
            unstable_base: None,
            sub_packages: None,
            proxy_vendor: None,
            license: None,
            description: "test package".into(),
            homepage: "https://example.com".into(),
            fork_of: None,
            fork_reason: None,
            native_build_inputs: None,
            python_deps: None,
            pname_override: None,
            dont_npm_build: None,
            extra_post_install: None,
            binary_name: None,
            platform_urls: None,
            versions: BTreeMap::new(),
        }
    }

    /// Create a pending VersionEntry with just a rev.
    #[must_use]
    pub fn pending_version(rev: &str) -> VersionEntry {
        VersionEntry {
            rev: rev.into(),
            source_hash: None,
            vendor_hash: None,
            cargo_hash: None,
            npm_deps_hash: None,
            maven_hash: None,
            nuget_deps_hash: None,
            status: Status::Pending,
            verified_at: None,
            hash_aarch64_darwin: None,
            hash_x86_64_darwin: None,
            hash_x86_64_linux: None,
            hash_aarch64_linux: None,
        }
    }

    /// Create a verified VersionEntry with source + vendor hashes.
    #[must_use]
    pub fn verified_version(rev: &str, source: &str, vendor: Option<&str>) -> VersionEntry {
        VersionEntry {
            rev: rev.into(),
            source_hash: Some(source.into()),
            vendor_hash: vendor.map(|s| s.into()),
            cargo_hash: None,
            npm_deps_hash: None,
            maven_hash: None,
            nuget_deps_hash: None,
            status: Status::Verified,
            verified_at: None,
            hash_aarch64_darwin: None,
            hash_x86_64_darwin: None,
            hash_x86_64_linux: None,
            hash_aarch64_linux: None,
        }
    }
}
