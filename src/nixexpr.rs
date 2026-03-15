//! Nix expression templates for trial builds and verification.
//!
//! Used by both `build` (with dummy hashes to extract real hashes) and
//! `verify` (with real hashes to validate builds).

use crate::matrix::Package;

/// Generate a Go build expression.
pub fn go_expr(
    pkg: &Package,
    rev: &str,
    source_hash: &str,
    vendor_hash: &str,
    for_verify: bool,
) -> String {
    let proxy_vendor = pkg.proxy_vendor.unwrap_or(false);
    let proxy_str = if proxy_vendor { "true" } else { "false" };

    let sub_packages = pkg.sub_packages.as_ref().map_or_else(
        || r#"["."]"#.to_string(),
        |sp| {
            let items: Vec<String> = sp.iter().map(|s| format!("\"{s}\"")).collect();
            format!("[{}]", items.join(" "))
        },
    );

    let pname = if for_verify {
        "verify"
    } else {
        "extract-hash"
    };

    format!(
        r#"let pkgs = import <nixpkgs> {{}}; in pkgs.buildGoModule {{
  pname = "{pname}";
  version = "0.0.0";
  src = pkgs.fetchFromGitHub {{
    owner = "{owner}";
    repo = "{repo}";
    rev = "{rev}";
    hash = "{source_hash}";
  }};
  vendorHash = {vendor_hash};
  proxyVendor = {proxy_str};
  subPackages = {sub_packages};
  doCheck = false;
}}"#,
        owner = pkg.owner,
        repo = pkg.repo,
    )
}

/// Generate a Rust build expression.
pub fn rust_expr(
    pkg: &Package,
    rev: &str,
    source_hash: &str,
    cargo_hash: &str,
    for_verify: bool,
) -> String {
    let native_build_inputs = pkg
        .native_build_inputs
        .as_ref()
        .map_or_else(String::new, |inputs| {
            let items: Vec<String> = inputs.iter().map(|s| format!("pkgs.{s}")).collect();
            format!("nativeBuildInputs = [{}];", items.join(" "))
        });

    let pname = if for_verify {
        "verify"
    } else {
        "extract-hash"
    };

    format!(
        r#"let pkgs = import <nixpkgs> {{}}; in pkgs.rustPlatform.buildRustPackage {{
  pname = "{pname}";
  version = "0.0.0";
  src = pkgs.fetchFromGitHub {{
    owner = "{owner}";
    repo = "{repo}";
    rev = "{rev}";
    hash = "{source_hash}";
  }};
  cargoHash = "{cargo_hash}";
  doCheck = false;
  {native_build_inputs}
}}"#,
        owner = pkg.owner,
        repo = pkg.repo,
    )
}

/// Generate a TypeScript/npm build expression.
pub fn typescript_expr(
    pkg: &Package,
    rev: &str,
    source_hash: &str,
    npm_deps_hash: &str,
    for_verify: bool,
) -> String {
    let dont_npm_build = pkg.dont_npm_build.unwrap_or(false);
    let dont_build_str = if dont_npm_build { "true" } else { "false" };

    let pname = if for_verify {
        "verify"
    } else {
        "extract-hash"
    };

    format!(
        r#"let pkgs = import <nixpkgs> {{}}; in pkgs.buildNpmPackage {{
  pname = "{pname}";
  version = "0.0.0";
  src = pkgs.fetchFromGitHub {{
    owner = "{owner}";
    repo = "{repo}";
    rev = "{rev}";
    hash = "{source_hash}";
  }};
  npmDepsHash = "{npm_deps_hash}";
  dontNpmBuild = {dont_build_str};
}}"#,
        owner = pkg.owner,
        repo = pkg.repo,
    )
}

/// Generate a Python build expression.
pub fn python_expr(
    pkg: &Package,
    rev: &str,
    source_hash: &str,
    for_verify: bool,
) -> String {
    let pname = pkg.pname_override.as_deref().unwrap_or(if for_verify {
        "verify"
    } else {
        "extract-hash"
    });
    let deps = pkg
        .python_deps
        .as_ref()
        .map_or_else(String::new, |deps| {
            let items: Vec<String> = deps.iter().map(|d| format!("      {d}")).collect();
            format!(
                "propagatedBuildInputs = with pkgs.python3Packages; [\n{}\n    ];",
                items.join("\n")
            )
        });

    format!(
        r#"let pkgs = import <nixpkgs> {{}}; in pkgs.python3Packages.buildPythonPackage {{
  pname = "{pname}";
  version = "0.0.0";
  src = pkgs.fetchFromGitHub {{
    owner = "{owner}";
    repo = "{repo}";
    rev = "{rev}";
    hash = "{source_hash}";
  }};
  doCheck = false;
  {deps}
}}"#,
        owner = pkg.owner,
        repo = pkg.repo,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix::{Builder, Language};
    use std::collections::BTreeMap;

    fn test_go_pkg() -> Package {
        Package {
            owner: "testorg".into(),
            repo: "testrepo".into(),
            language: Language::Go,
            builder: Builder::MkGoTool,
            tier: 1,
            sub_packages: Some(vec!["cmd/main".into()]),
            proxy_vendor: Some(true),
            license: None,
            description: "test".into(),
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
            track: crate::matrix::TrackMode::default(),
            unstable_base: None,
            versions: BTreeMap::new(),
        }
    }

    #[test]
    fn test_go_expr_includes_proxy_vendor() {
        let pkg = test_go_pkg();
        let expr = go_expr(&pkg, "abc123", "sha256-src", "\"sha256-vendor\"", false);
        assert!(expr.contains("proxyVendor = true"));
        assert!(expr.contains(r#"subPackages = ["cmd/main"]"#));
        assert!(expr.contains(r#"owner = "testorg""#));
        assert!(expr.contains(r#"rev = "abc123""#));
        assert!(expr.contains(r#"pname = "extract-hash""#));
    }

    #[test]
    fn test_go_expr_verify_mode() {
        let pkg = test_go_pkg();
        let expr = go_expr(&pkg, "abc", "sha256-s", "null", true);
        assert!(expr.contains(r#"pname = "verify""#));
        assert!(expr.contains("vendorHash = null"));
    }

    #[test]
    fn test_rust_expr_with_native_inputs() {
        let mut pkg = test_go_pkg();
        pkg.language = Language::Rust;
        pkg.builder = Builder::BuildRustPackage;
        pkg.native_build_inputs = Some(vec!["protobuf".into()]);
        let expr = rust_expr(&pkg, "def", "sha256-s", "sha256-c", false);
        assert!(expr.contains("nativeBuildInputs = [pkgs.protobuf]"));
        assert!(expr.contains("cargoHash"));
    }

    #[test]
    fn test_typescript_expr_dont_npm_build() {
        let mut pkg = test_go_pkg();
        pkg.language = Language::TypeScript;
        pkg.builder = Builder::BuildNpmPackage;
        pkg.dont_npm_build = Some(true);
        let expr = typescript_expr(&pkg, "ghi", "sha256-s", "sha256-n", false);
        assert!(expr.contains("dontNpmBuild = true"));
        assert!(expr.contains("npmDepsHash"));
    }

    #[test]
    fn test_python_expr_with_deps() {
        let mut pkg = test_go_pkg();
        pkg.language = Language::Python;
        pkg.builder = Builder::MkPythonPackage;
        pkg.python_deps = Some(vec!["requests".into(), "urllib3".into()]);
        pkg.pname_override = Some("mypkg".into());
        let expr = python_expr(&pkg, "jkl", "sha256-s", false);
        assert!(expr.contains(r#"pname = "mypkg""#));
        assert!(expr.contains("propagatedBuildInputs"));
        assert!(expr.contains("requests"));
        assert!(expr.contains("urllib3"));
    }
}
