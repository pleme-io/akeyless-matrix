use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use toml_edit::DocumentMut;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
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

impl FromStr for Status {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "pending" => Ok(Self::Pending),
            "building" => Ok(Self::Building),
            "verified" => Ok(Self::Verified),
            "broken" => Ok(Self::Broken),
            other => Err(anyhow::anyhow!("unknown status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
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

impl FromStr for Language {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "go" => Ok(Self::Go),
            "rust" => Ok(Self::Rust),
            "python" => Ok(Self::Python),
            "typescript" => Ok(Self::TypeScript),
            "java" => Ok(Self::Java),
            "ruby" => Ok(Self::Ruby),
            "php" => Ok(Self::Php),
            "csharp" => Ok(Self::Csharp),
            "helm" => Ok(Self::Helm),
            other => Err(anyhow::anyhow!("unknown language: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
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

impl FromStr for Builder {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "mkGoTool" => Ok(Self::MkGoTool),
            "mkGoLibraryCheck" => Ok(Self::MkGoLibraryCheck),
            "buildRustPackage" => Ok(Self::BuildRustPackage),
            "mkPythonPackage" => Ok(Self::MkPythonPackage),
            "buildNpmPackage" => Ok(Self::BuildNpmPackage),
            "fetchurl" => Ok(Self::Fetchurl),
            "mkJavaMavenPackage" => Ok(Self::MkJavaMavenPackage),
            "mkDotnetPackage" => Ok(Self::MkDotnetPackage),
            "mkTerraformModuleCheck" => Ok(Self::MkTerraformModuleCheck),
            "none" => Ok(Self::None),
            other => Err(anyhow::anyhow!("unknown builder: {other}")),
        }
    }
}

impl fmt::Display for TrackMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tags => write!(f, "tags"),
            Self::Commits => write!(f, "commits"),
            Self::Binary => write!(f, "binary"),
        }
    }
}

impl FromStr for TrackMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "tags" => Ok(Self::Tags),
            "commits" => Ok(Self::Commits),
            "binary" => Ok(Self::Binary),
            other => Err(anyhow::anyhow!("unknown track mode: {other}")),
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
    /// .NET `NuGet` dependencies hash for C#/.NET packages
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
#[non_exhaustive]
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

            set_optional_string(pkg_table, "license", pkg.license.as_ref());
            set_optional_string(pkg_table, "fork_of", pkg.fork_of.as_ref());
            set_optional_string(pkg_table, "fork_reason", pkg.fork_reason.as_ref());
            set_optional_string(pkg_table, "pname_override", pkg.pname_override.as_ref());
            set_optional_string(pkg_table, "extra_post_install", pkg.extra_post_install.as_ref());
            set_optional_bool(pkg_table, "proxy_vendor", pkg.proxy_vendor);
            set_optional_bool(pkg_table, "dont_npm_build", pkg.dont_npm_build);
            set_optional_string(pkg_table, "binary_name", pkg.binary_name.as_ref());
            set_optional_vec(pkg_table, "sub_packages", pkg.sub_packages.as_ref());
            set_optional_vec(pkg_table, "native_build_inputs", pkg.native_build_inputs.as_ref());
            set_optional_vec(pkg_table, "python_deps", pkg.python_deps.as_ref());

            // platform_urls sub-table
            if let Some(ref urls) = pkg.platform_urls {
                if pkg_table.get("platform_urls").is_none() {
                    pkg_table["platform_urls"] = toml_edit::Item::Table(toml_edit::Table::new());
                }
                let urls_table = pkg_table["platform_urls"]
                    .as_table_mut()
                    .with_context(|| format!("{pkg_name}.platform_urls is not a table"))?;
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
            .next_back()
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
    /// `"0.6.1"` -> `"0_6_1"`, `"0.1.0-pleme.1"` -> `"0_1_0-pleme_1"`
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
    value: Option<&String>,
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
    value: Option<&Vec<String>>,
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
        let matrix = test_helpers::from_str(toml).unwrap();
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

    #[test]
    fn test_status_display() {
        assert_eq!(Status::Pending.to_string(), "pending");
        assert_eq!(Status::Building.to_string(), "building");
        assert_eq!(Status::Verified.to_string(), "verified");
        assert_eq!(Status::Broken.to_string(), "broken");
    }

    #[test]
    fn test_language_display() {
        assert_eq!(Language::Go.to_string(), "go");
        assert_eq!(Language::Rust.to_string(), "rust");
        assert_eq!(Language::Python.to_string(), "python");
        assert_eq!(Language::TypeScript.to_string(), "typescript");
        assert_eq!(Language::Java.to_string(), "java");
        assert_eq!(Language::Ruby.to_string(), "ruby");
        assert_eq!(Language::Php.to_string(), "php");
        assert_eq!(Language::Csharp.to_string(), "csharp");
        assert_eq!(Language::Helm.to_string(), "helm");
    }

    #[test]
    fn test_builder_display() {
        assert_eq!(Builder::MkGoTool.to_string(), "mkGoTool");
        assert_eq!(Builder::MkGoLibraryCheck.to_string(), "mkGoLibraryCheck");
        assert_eq!(Builder::BuildRustPackage.to_string(), "buildRustPackage");
        assert_eq!(Builder::MkPythonPackage.to_string(), "mkPythonPackage");
        assert_eq!(Builder::BuildNpmPackage.to_string(), "buildNpmPackage");
        assert_eq!(Builder::Fetchurl.to_string(), "fetchurl");
        assert_eq!(Builder::MkJavaMavenPackage.to_string(), "mkJavaMavenPackage");
        assert_eq!(Builder::MkDotnetPackage.to_string(), "mkDotnetPackage");
        assert_eq!(Builder::MkTerraformModuleCheck.to_string(), "mkTerraformModuleCheck");
        assert_eq!(Builder::None.to_string(), "none");
    }

    #[test]
    fn test_status_fromstr_roundtrip() {
        for s in &[Status::Pending, Status::Building, Status::Verified, Status::Broken] {
            let parsed: Status = s.to_string().parse().unwrap();
            assert_eq!(*s, parsed);
        }
        assert!("invalid".parse::<Status>().is_err());
    }

    #[test]
    fn test_language_fromstr_roundtrip() {
        for l in &[
            Language::Go, Language::Rust, Language::Python, Language::TypeScript,
            Language::Java, Language::Ruby, Language::Php, Language::Csharp, Language::Helm,
        ] {
            let parsed: Language = l.to_string().parse().unwrap();
            assert_eq!(*l, parsed);
        }
        assert!("invalid".parse::<Language>().is_err());
    }

    #[test]
    fn test_builder_fromstr_roundtrip() {
        for b in &[
            Builder::MkGoTool, Builder::MkGoLibraryCheck, Builder::BuildRustPackage,
            Builder::MkPythonPackage, Builder::BuildNpmPackage, Builder::Fetchurl,
            Builder::MkJavaMavenPackage, Builder::MkDotnetPackage,
            Builder::MkTerraformModuleCheck, Builder::None,
        ] {
            let parsed: Builder = b.to_string().parse().unwrap();
            assert_eq!(*b, parsed);
        }
        assert!("invalid".parse::<Builder>().is_err());
    }

    #[test]
    fn test_track_mode_display_fromstr_roundtrip() {
        for t in &[TrackMode::Tags, TrackMode::Commits, TrackMode::Binary] {
            assert_eq!(*t, t.to_string().parse::<TrackMode>().unwrap());
        }
        assert!("invalid".parse::<TrackMode>().is_err());
    }

    #[test]
    fn test_status_serde_roundtrip() {
        let toml_str = r#"status = "building""#;
        #[derive(serde::Deserialize)]
        struct Wrap {
            status: Status,
        }
        let w: Wrap = toml::from_str(toml_str).unwrap();
        assert_eq!(w.status, Status::Building);
    }

    #[test]
    fn test_builder_serde_roundtrip() {
        let toml_str = r#"builder = "mkTerraformModuleCheck""#;
        #[derive(serde::Deserialize)]
        struct Wrap {
            builder: Builder,
        }
        let w: Wrap = toml::from_str(toml_str).unwrap();
        assert_eq!(w.builder, Builder::MkTerraformModuleCheck);
    }

    #[test]
    fn test_language_serde_roundtrip() {
        let toml_str = r#"language = "csharp""#;
        #[derive(serde::Deserialize)]
        struct Wrap {
            language: Language,
        }
        let w: Wrap = toml::from_str(toml_str).unwrap();
        assert_eq!(w.language, Language::Csharp);
    }

    #[test]
    fn test_track_mode_default() {
        assert_eq!(TrackMode::default(), TrackMode::Tags);
    }

    #[test]
    fn test_track_mode_serde() {
        let toml_str = r#"track = "commits""#;
        #[derive(serde::Deserialize)]
        struct Wrap {
            track: TrackMode,
        }
        let w: Wrap = toml::from_str(toml_str).unwrap();
        assert_eq!(w.track, TrackMode::Commits);
    }

    #[test]
    fn test_track_mode_binary() {
        let toml_str = r#"track = "binary""#;
        #[derive(serde::Deserialize)]
        struct Wrap {
            track: TrackMode,
        }
        let w: Wrap = toml::from_str(toml_str).unwrap();
        assert_eq!(w.track, TrackMode::Binary);
    }

    #[test]
    fn test_from_str_all_languages() {
        let toml = r#"
[packages.test-go]
owner = "o"
repo = "r"
language = "go"
builder = "mkGoTool"
tier = 1
description = "d"
homepage = "h"

[packages.test-rust]
owner = "o"
repo = "r"
language = "rust"
builder = "buildRustPackage"
tier = 2
description = "d"
homepage = "h"

[packages.test-py]
owner = "o"
repo = "r"
language = "python"
builder = "mkPythonPackage"
tier = 3
description = "d"
homepage = "h"

[packages.test-ts]
owner = "o"
repo = "r"
language = "typescript"
builder = "buildNpmPackage"
tier = 2
description = "d"
homepage = "h"

[packages.test-java]
owner = "o"
repo = "r"
language = "java"
builder = "mkJavaMavenPackage"
tier = 3
description = "d"
homepage = "h"

[packages.test-csharp]
owner = "o"
repo = "r"
language = "csharp"
builder = "mkDotnetPackage"
tier = 3
description = "d"
homepage = "h"

[packages.test-helm]
owner = "o"
repo = "r"
language = "helm"
builder = "none"
tier = 3
description = "d"
homepage = "h"
"#;
        let matrix = test_helpers::from_str(toml).unwrap();
        assert_eq!(matrix.packages.len(), 7);
        assert_eq!(matrix.packages["test-go"].language, Language::Go);
        assert_eq!(matrix.packages["test-rust"].language, Language::Rust);
        assert_eq!(matrix.packages["test-py"].language, Language::Python);
        assert_eq!(matrix.packages["test-ts"].language, Language::TypeScript);
        assert_eq!(matrix.packages["test-java"].language, Language::Java);
        assert_eq!(matrix.packages["test-csharp"].language, Language::Csharp);
        assert_eq!(matrix.packages["test-helm"].language, Language::Helm);
    }

    #[test]
    fn test_from_str_with_optional_fields() {
        let toml = r#"
[packages.akeyless-test]
owner = "org"
repo = "test"
language = "go"
builder = "mkGoTool"
tier = 1
description = "test"
homepage = "https://example.com"
license = "MIT"
fork_of = "upstream/repo"
fork_reason = "go mod fix"
proxy_vendor = true
sub_packages = ["cmd/main", "cmd/helper"]
extra_post_install = "echo done"
track = "commits"
unstable_base = "0.1.0"
"#;
        let matrix = test_helpers::from_str(toml).unwrap();
        let pkg = &matrix.packages["akeyless-test"];
        assert_eq!(pkg.license.as_deref(), Some("MIT"));
        assert_eq!(pkg.fork_of.as_deref(), Some("upstream/repo"));
        assert_eq!(pkg.fork_reason.as_deref(), Some("go mod fix"));
        assert_eq!(pkg.proxy_vendor, Some(true));
        assert_eq!(
            pkg.sub_packages.as_ref().unwrap(),
            &vec!["cmd/main".to_string(), "cmd/helper".to_string()]
        );
        assert_eq!(pkg.track, TrackMode::Commits);
        assert_eq!(pkg.unstable_base.as_deref(), Some("0.1.0"));
    }

    #[test]
    fn test_from_str_version_entry_all_hashes() {
        let toml = r#"
[packages.akeyless-test]
owner = "org"
repo = "test"
language = "go"
builder = "fetchurl"
tier = 1
description = "test"
homepage = "https://example.com"

[packages.akeyless-test.versions."1.0.0"]
rev = "abc123"
status = "verified"
source_hash = "sha256-src"
vendor_hash = "sha256-vendor"
cargo_hash = "sha256-cargo"
npm_deps_hash = "sha256-npm"
maven_hash = "sha256-maven"
nuget_deps_hash = "sha256-nuget"
hash_aarch64_darwin = "sha256-arm-darwin"
hash_x86_64_darwin = "sha256-x86-darwin"
hash_x86_64_linux = "sha256-x86-linux"
hash_aarch64_linux = "sha256-arm-linux"
"#;
        let matrix = test_helpers::from_str(toml).unwrap();
        let entry = &matrix.packages["akeyless-test"].versions["1.0.0"];
        assert_eq!(entry.source_hash.as_deref(), Some("sha256-src"));
        assert_eq!(entry.vendor_hash.as_deref(), Some("sha256-vendor"));
        assert_eq!(entry.cargo_hash.as_deref(), Some("sha256-cargo"));
        assert_eq!(entry.npm_deps_hash.as_deref(), Some("sha256-npm"));
        assert_eq!(entry.maven_hash.as_deref(), Some("sha256-maven"));
        assert_eq!(entry.nuget_deps_hash.as_deref(), Some("sha256-nuget"));
        assert_eq!(entry.hash_aarch64_darwin.as_deref(), Some("sha256-arm-darwin"));
        assert_eq!(entry.hash_x86_64_darwin.as_deref(), Some("sha256-x86-darwin"));
        assert_eq!(entry.hash_x86_64_linux.as_deref(), Some("sha256-x86-linux"));
        assert_eq!(entry.hash_aarch64_linux.as_deref(), Some("sha256-arm-linux"));
    }

    #[test]
    fn test_from_str_invalid_toml() {
        let bad_toml = "not valid [ toml }{";
        let result = test_helpers::from_str(bad_toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_latest_verified_no_verified() {
        let mut versions = BTreeMap::new();
        versions.insert(
            "1.0.0".to_string(),
            VersionEntry {
                rev: "aaa".into(),
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

        assert!(Matrix::latest_verified(&pkg).is_none());
    }

    #[test]
    fn test_all_verified_empty() {
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
            versions: BTreeMap::new(),
        };

        assert!(Matrix::all_verified(&pkg).is_empty());
    }

    #[test]
    fn test_source_key_empty_string() {
        assert_eq!(Matrix::source_key(""), "");
    }

    #[test]
    fn test_sanitize_version_no_dots() {
        assert_eq!(Matrix::sanitize_version("100"), "100");
    }

    #[test]
    fn test_sanitize_version_unstable() {
        assert_eq!(
            Matrix::sanitize_version("0.1.0-unstable.2026-03-14.d240017e"),
            "0_1_0-unstable_2026-03-14_d240017e"
        );
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = std::env::temp_dir().join("matrix-save-load-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("matrix.toml");

        let mut versions = BTreeMap::new();
        versions.insert(
            "1.0.0".to_string(),
            VersionEntry {
                rev: "abc123".into(),
                source_hash: Some("sha256-src".into()),
                vendor_hash: Some("sha256-vendor".into()),
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
        let mut packages = BTreeMap::new();
        packages.insert(
            "akeyless-test".to_string(),
            Package {
                owner: "testorg".into(),
                repo: "test".into(),
                language: Language::Go,
                builder: Builder::MkGoTool,
                tier: 1,
                sub_packages: None,
                proxy_vendor: None,
                license: None,
                description: "test pkg".into(),
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
                track: TrackMode::default(),
                unstable_base: None,
                versions,
            },
        );
        let matrix = Matrix { packages };

        Matrix::save_to_path(&path, &matrix).unwrap();
        let loaded = Matrix::load_from_path(&path).unwrap();

        assert_eq!(loaded.packages.len(), 1);
        let pkg = &loaded.packages["akeyless-test"];
        assert_eq!(pkg.owner, "testorg");
        assert_eq!(pkg.language, Language::Go);
        assert_eq!(pkg.builder, Builder::MkGoTool);
        let ver = &pkg.versions["1.0.0"];
        assert_eq!(ver.status, Status::Verified);
        assert_eq!(ver.source_hash.as_deref(), Some("sha256-src"));
        assert_eq!(ver.vendor_hash.as_deref(), Some("sha256-vendor"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_load_from_path_nonexistent() {
        let result = Matrix::load_from_path(std::path::Path::new("/tmp/nonexistent-matrix-xyz.toml"));
        assert!(result.is_err());
    }
}

/// Test helpers available to all modules via `crate::matrix::test_helpers`.
#[cfg(test)]
pub mod test_helpers {
    use super::*;
    use crate::runner::CommandOutput;
    use crate::storage::MatrixStore;
    use std::sync::Mutex;

    /// Mock command runner that returns predetermined responses in FIFO order.
    pub struct MockRunner {
        pub responses: Mutex<Vec<CommandOutput>>,
    }

    #[async_trait::async_trait]
    impl crate::runner::CommandRunner for MockRunner {
        async fn run(&self, _program: &str, _args: &[&str]) -> anyhow::Result<CommandOutput> {
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                anyhow::bail!("no more mock responses");
            }
            Ok(responses.remove(0))
        }
    }

    /// In-memory matrix store for testing, backed by a `Mutex<Matrix>`.
    pub struct InMemoryStore {
        pub matrix: Mutex<Matrix>,
    }

    impl MatrixStore for InMemoryStore {
        fn load(&self, _path: &std::path::Path) -> anyhow::Result<Matrix> {
            Ok(self.matrix.lock().unwrap().clone())
        }
        fn save(&self, _path: &std::path::Path, matrix: &Matrix) -> anyhow::Result<()> {
            *self.matrix.lock().unwrap() = matrix.clone();
            Ok(())
        }
    }

    /// Parse a [`Matrix`] from a TOML string.
    pub fn from_str(content: &str) -> anyhow::Result<Matrix> {
        toml::from_str(content).context("parsing matrix TOML")
    }

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
