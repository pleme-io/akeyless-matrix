//! Certification fingerprinting and audit trail.
//!
//! After each `certify` run, computes a SHA-256 fingerprint of the entire
//! verified matrix state (all packages × versions × hashes). This fingerprint
//! uniquely identifies the exact set of certified software. A log of
//! certification events with deltas is appended to `certifications.toml`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::matrix::{Matrix, Status};

/// A snapshot of one verified package+version for fingerprinting.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct VerifiedEntry {
    package: String,
    version: String,
    source_hash: String,
    build_hash: String, // vendor_hash | cargo_hash | npm_deps_hash | ""
}

/// A single certification event in the log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificationEntry {
    pub id: String,
    pub parent_id: Option<String>,
    pub at: DateTime<Utc>,
    pub added: Vec<String>,
    pub updated: Vec<String>,
    pub total_verified: usize,
}

/// The certifications log file format.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CertificationLog {
    #[serde(default)]
    pub entries: Vec<CertificationEntry>,
}

/// Compute a SHA-256 fingerprint of all verified entries in the matrix.
///
/// The fingerprint is deterministic: same verified state always produces the
/// same hash, regardless of pending/broken entries or matrix ordering.
#[must_use]
pub fn compute_fingerprint(matrix: &Matrix) -> String {
    let mut entries: Vec<VerifiedEntry> = matrix.packages.iter()
        .flat_map(|(pkg_name, pkg)| {
            pkg.versions.iter()
                .filter(|(_, entry)| entry.status == Status::Verified)
                .map(move |(ver, entry)| {
                    let build_hash = entry.build_hash().unwrap_or("").to_string();

                    VerifiedEntry {
                        package: pkg_name.clone(),
                        version: ver.clone(),
                        source_hash: entry.source_hash.clone().unwrap_or_default(),
                        build_hash,
                    }
                })
        })
        .collect();

    // Sort for determinism (BTreeMap is already sorted, but versions within
    // a package are also in BTreeMap order)
    entries.sort();

    // Build canonical representation
    let mut canonical = String::new();
    for e in &entries {
        let _ = writeln!(
            canonical,
            "{}@{}:{}:{}",
            e.package, e.version, e.source_hash, e.build_hash
        );
    }

    // SHA-256
    sha256_hex(&canonical)
}

/// Compute the delta between the previous and current verified states.
///
/// Returns (added, updated) where:
/// - added: newly verified package@version pairs (including historical versions)
/// - updated: packages whose latest version changed
#[must_use]
pub fn compute_delta(
    prev: &Matrix,
    current: &Matrix,
) -> (Vec<String>, Vec<String>) {
    // Track ALL verified (package, version) pairs for detecting new certifications
    let prev_all = all_verified_set(prev);
    let curr_all = all_verified_set(current);

    // Track latest-per-package for detecting version bumps
    let prev_latest = latest_verified_map(prev);
    let curr_latest = latest_verified_map(current);

    let mut added: Vec<String> = curr_all.difference(&prev_all).cloned().collect();
    let mut updated: Vec<String> = curr_latest.iter()
        .filter_map(|(key, curr_ver)| {
            prev_latest.get(key)
                .filter(|prev_ver| *prev_ver != curr_ver)
                .map(|prev_ver| format!("{key}: {prev_ver} -> {curr_ver}"))
        })
        .collect();

    added.sort();
    updated.sort();
    (added, updated)
}

/// Build set of all "package@version" strings for verified entries.
fn all_verified_set(matrix: &Matrix) -> std::collections::BTreeSet<String> {
    matrix.packages.iter()
        .flat_map(|(name, pkg)| {
            pkg.versions.iter()
                .filter(|(_, entry)| entry.status == Status::Verified)
                .map(move |(ver, _)| format!("{name}@{ver}"))
        })
        .collect()
}

/// Build a map of package -> latest verified version.
fn latest_verified_map(matrix: &Matrix) -> BTreeMap<String, String> {
    matrix.packages.iter()
        .filter_map(|(name, pkg)| {
            Matrix::latest_verified(pkg).map(|(ver, _)| (name.clone(), ver.to_string()))
        })
        .collect()
}

/// Load the certification log from disk (or return empty if missing).
pub fn load_log(matrix_dir: &Path) -> Result<CertificationLog> {
    let path = matrix_dir.join("certifications.toml");
    if !path.exists() {
        return Ok(CertificationLog::default());
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    let log: CertificationLog = toml::from_str(&content)
        .with_context(|| format!("parsing {}", path.display()))?;
    Ok(log)
}

/// Save the certification log to disk.
pub fn save_log(matrix_dir: &Path, log: &CertificationLog) -> Result<()> {
    let path = matrix_dir.join("certifications.toml");
    let content = toml::to_string_pretty(log)
        .context("serializing certification log")?;
    std::fs::write(&path, content)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Record a certification event: compute fingerprint, delta, and append to log.
///
/// `prev_matrix` is the state before `certify` ran (before build changed statuses).
/// `current_matrix` is the state after `certify` (newly verified entries).
pub fn record(
    matrix_dir: &Path,
    prev_matrix: &Matrix,
    current_matrix: &Matrix,
) -> Result<CertificationEntry> {
    let fingerprint = compute_fingerprint(current_matrix);
    let (added, updated) = compute_delta(prev_matrix, current_matrix);

    let total_verified = current_matrix
        .packages
        .values()
        .flat_map(|p| p.versions.values())
        .filter(|v| v.status == Status::Verified)
        .count();

    let mut log = load_log(matrix_dir)?;

    let parent_id = log.entries.last().map(|e| e.id.clone());

    // Skip if fingerprint matches the latest entry (no-op certification)
    if parent_id.as_deref() == Some(&fingerprint) {
        return log
            .entries
            .last()
            .cloned()
            .context("certification log has parent_id but no entries");
    }

    let entry = CertificationEntry {
        id: fingerprint,
        parent_id,
        at: Utc::now(),
        added,
        updated,
        total_verified,
    };

    log.entries.push(entry.clone());
    save_log(matrix_dir, &log)?;

    Ok(entry)
}

/// Simple SHA-256 using manual computation (no external crate needed).
/// Uses the standard FIPS 180-4 algorithm.
#[allow(clippy::many_single_char_names)]
fn sha256_hex(input: &str) -> String {
    // Initial hash values (first 32 bits of fractional parts of square roots of first 8 primes)
    let mut h: [u32; 8] = [
        0x6a09_e667, 0xbb67_ae85, 0x3c6e_f372, 0xa54f_f53a,
        0x510e_527f, 0x9b05_688c, 0x1f83_d9ab, 0x5be0_cd19,
    ];

    // Round constants
    let k: [u32; 64] = [
        0x428a_2f98, 0x7137_4491, 0xb5c0_fbcf, 0xe9b5_dba5,
        0x3956_c25b, 0x59f1_11f1, 0x923f_82a4, 0xab1c_5ed5,
        0xd807_aa98, 0x1283_5b01, 0x2431_85be, 0x550c_7dc3,
        0x72be_5d74, 0x80de_b1fe, 0x9bdc_06a7, 0xc19b_f174,
        0xe49b_69c1, 0xefbe_4786, 0x0fc1_9dc6, 0x240c_a1cc,
        0x2de9_2c6f, 0x4a74_84aa, 0x5cb0_a9dc, 0x76f9_88da,
        0x983e_5152, 0xa831_c66d, 0xb003_27c8, 0xbf59_7fc7,
        0xc6e0_0bf3, 0xd5a7_9147, 0x06ca_6351, 0x1429_2967,
        0x27b7_0a85, 0x2e1b_2138, 0x4d2c_6dfc, 0x5338_0d13,
        0x650a_7354, 0x766a_0abb, 0x81c2_c92e, 0x9272_2c85,
        0xa2bf_e8a1, 0xa81a_664b, 0xc24b_8b70, 0xc76c_51a3,
        0xd192_e819, 0xd699_0624, 0xf40e_3585, 0x106a_a070,
        0x19a4_c116, 0x1e37_6c08, 0x2748_774c, 0x34b0_bcb5,
        0x391c_0cb3, 0x4ed8_aa4a, 0x5b9c_ca4f, 0x682e_6ff3,
        0x748f_82ee, 0x78a5_636f, 0x84c8_7814, 0x8cc7_0208,
        0x90be_fffa, 0xa450_6ceb, 0xbef9_a3f7, 0xc671_78f2,
    ];

    let bytes = input.as_bytes();
    let bit_len = (bytes.len() as u64) * 8;

    // Pre-processing: pad message
    let mut msg = bytes.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    // Process each 512-bit (64-byte) block
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[4 * i],
                chunk[4 * i + 1],
                chunk[4 * i + 2],
                chunk[4 * i + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(k[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    format!(
        "{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix::{Builder, Language, Package, VersionEntry};

    #[test]
    fn test_sha256_known_vector() {
        // SHA-256 of empty string
        assert_eq!(
            sha256_hex(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        // SHA-256 of "hello"
        assert_eq!(
            sha256_hex("hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    fn make_pkg(versions: Vec<(&str, &str, &str, Status)>) -> Package {
        let mut vers = BTreeMap::new();
        for (ver, src, vendor, status) in versions {
            vers.insert(
                ver.to_string(),
                VersionEntry {
                    rev: "abc".into(),
                    source_hash: Some(src.into()),
                    vendor_hash: if vendor.is_empty() {
                        None
                    } else {
                        Some(vendor.into())
                    },
                    cargo_hash: None,
                    npm_deps_hash: None,
                    maven_hash: None,
                    nuget_deps_hash: None,
                    status,
                    verified_at: None,
                    hash_aarch64_darwin: None,
                    hash_x86_64_darwin: None,
                    hash_x86_64_linux: None,
                    hash_aarch64_linux: None,
                },
            );
        }
        Package {
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
            track: crate::matrix::TrackMode::default(),
            unstable_base: None,
            versions: vers,
        }
    }

    #[test]
    fn test_fingerprint_deterministic() {
        let mut pkgs = BTreeMap::new();
        pkgs.insert(
            "akeyless-cli".to_string(),
            make_pkg(vec![("0.6.1", "sha256-a", "sha256-v", Status::Verified)]),
        );
        let m1 = Matrix { packages: pkgs.clone() };
        let m2 = Matrix { packages: pkgs };
        assert_eq!(compute_fingerprint(&m1), compute_fingerprint(&m2));
    }

    #[test]
    fn test_fingerprint_changes_on_new_version() {
        let mut pkgs1 = BTreeMap::new();
        pkgs1.insert(
            "akeyless-cli".to_string(),
            make_pkg(vec![("0.6.1", "sha256-a", "sha256-v", Status::Verified)]),
        );

        let mut pkgs2 = BTreeMap::new();
        pkgs2.insert(
            "akeyless-cli".to_string(),
            make_pkg(vec![
                ("0.6.1", "sha256-a", "sha256-v", Status::Verified),
                ("0.7.0", "sha256-b", "sha256-w", Status::Verified),
            ]),
        );

        let f1 = compute_fingerprint(&Matrix { packages: pkgs1 });
        let f2 = compute_fingerprint(&Matrix { packages: pkgs2 });
        assert_ne!(f1, f2);
    }

    #[test]
    fn test_fingerprint_ignores_pending() {
        let mut pkgs1 = BTreeMap::new();
        pkgs1.insert(
            "akeyless-cli".to_string(),
            make_pkg(vec![("0.6.1", "sha256-a", "sha256-v", Status::Verified)]),
        );

        let mut pkgs2 = BTreeMap::new();
        pkgs2.insert(
            "akeyless-cli".to_string(),
            make_pkg(vec![
                ("0.6.1", "sha256-a", "sha256-v", Status::Verified),
                ("0.7.0", "sha256-b", "sha256-w", Status::Pending),
            ]),
        );

        let f1 = compute_fingerprint(&Matrix { packages: pkgs1 });
        let f2 = compute_fingerprint(&Matrix { packages: pkgs2 });
        assert_eq!(f1, f2); // pending doesn't affect fingerprint
    }

    #[test]
    fn test_log_roundtrip() {
        let log = CertificationLog {
            entries: vec![CertificationEntry {
                id: "abc123".into(),
                parent_id: None,
                at: Utc::now(),
                added: vec!["pkg@1.0".into()],
                updated: vec![],
                total_verified: 1,
            }],
        };
        let serialized = toml::to_string_pretty(&log).unwrap();
        let deserialized: CertificationLog = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.entries.len(), 1);
        assert_eq!(deserialized.entries[0].id, "abc123");
        assert_eq!(deserialized.entries[0].added, vec!["pkg@1.0"]);
    }

    #[test]
    fn test_delta_detects_historical_versions() {
        // Certifying a backport (0.6.2) while 0.7.0 is already latest
        let mut prev_pkgs = BTreeMap::new();
        prev_pkgs.insert(
            "akeyless-cli".to_string(),
            make_pkg(vec![
                ("0.6.1", "sha256-a", "sha256-v", Status::Verified),
                ("0.7.0", "sha256-b", "sha256-w", Status::Verified),
            ]),
        );
        let prev = Matrix { packages: prev_pkgs };

        let mut curr_pkgs = BTreeMap::new();
        curr_pkgs.insert(
            "akeyless-cli".to_string(),
            make_pkg(vec![
                ("0.6.1", "sha256-a", "sha256-v", Status::Verified),
                ("0.6.2", "sha256-c", "sha256-x", Status::Verified), // backport
                ("0.7.0", "sha256-b", "sha256-w", Status::Verified),
            ]),
        );
        let curr = Matrix { packages: curr_pkgs };

        let (added, updated) = compute_delta(&prev, &curr);

        // Should detect the new historical version
        assert_eq!(added, vec!["akeyless-cli@0.6.2"]);
        // Latest didn't change (still 0.7.0)
        assert!(updated.is_empty());
    }

    #[test]
    fn test_delta_empty_when_no_changes() {
        let mut pkgs = BTreeMap::new();
        pkgs.insert(
            "akeyless-cli".to_string(),
            make_pkg(vec![("0.6.1", "sha256-a", "sha256-v", Status::Verified)]),
        );
        let m = Matrix { packages: pkgs };

        let (added, updated) = compute_delta(&m, &m);
        assert!(added.is_empty());
        assert!(updated.is_empty());
    }

    #[test]
    fn test_delta_detects_additions_and_updates() {
        let mut prev_pkgs = BTreeMap::new();
        prev_pkgs.insert(
            "akeyless-cli".to_string(),
            make_pkg(vec![("0.6.1", "sha256-a", "sha256-v", Status::Verified)]),
        );
        let prev = Matrix { packages: prev_pkgs };

        let mut curr_pkgs = BTreeMap::new();
        curr_pkgs.insert(
            "akeyless-cli".to_string(),
            make_pkg(vec![
                ("0.6.1", "sha256-a", "sha256-v", Status::Verified),
                ("0.7.0", "sha256-b", "sha256-w", Status::Verified),
            ]),
        );
        curr_pkgs.insert(
            "akeyless-new".to_string(),
            make_pkg(vec![("1.0.0", "sha256-n", "sha256-nv", Status::Verified)]),
        );
        let curr = Matrix { packages: curr_pkgs };

        let (added, updated) = compute_delta(&prev, &curr);

        // akeyless-cli@0.7.0 is a new verified entry, akeyless-new@1.0.0 is a new package
        assert_eq!(added, vec!["akeyless-cli@0.7.0", "akeyless-new@1.0.0"]);
        // akeyless-cli latest changed from 0.6.1 to 0.7.0
        assert_eq!(updated, vec!["akeyless-cli: 0.6.1 -> 0.7.0"]);
    }

    #[test]
    fn test_fingerprint_empty_matrix() {
        let matrix = Matrix {
            packages: BTreeMap::new(),
        };
        let fp = compute_fingerprint(&matrix);
        assert!(!fp.is_empty());
        assert_eq!(fp.len(), 64);
    }

    #[test]
    fn test_fingerprint_all_pending_same_as_empty() {
        let mut pkgs = BTreeMap::new();
        pkgs.insert(
            "akeyless-cli".to_string(),
            make_pkg(vec![("0.6.1", "sha256-a", "sha256-v", Status::Pending)]),
        );
        let with_pending = Matrix { packages: pkgs };
        let empty = Matrix {
            packages: BTreeMap::new(),
        };
        assert_eq!(
            compute_fingerprint(&with_pending),
            compute_fingerprint(&empty)
        );
    }

    #[test]
    fn test_fingerprint_broken_entries_excluded() {
        let mut pkgs = BTreeMap::new();
        pkgs.insert(
            "akeyless-cli".to_string(),
            make_pkg(vec![
                ("0.6.1", "sha256-a", "sha256-v", Status::Verified),
                ("0.7.0", "sha256-b", "sha256-w", Status::Broken),
            ]),
        );
        let with_broken = Matrix {
            packages: pkgs.clone(),
        };

        let mut pkgs2 = BTreeMap::new();
        pkgs2.insert(
            "akeyless-cli".to_string(),
            make_pkg(vec![("0.6.1", "sha256-a", "sha256-v", Status::Verified)]),
        );
        let without_broken = Matrix { packages: pkgs2 };

        assert_eq!(
            compute_fingerprint(&with_broken),
            compute_fingerprint(&without_broken)
        );
    }

    #[test]
    fn test_fingerprint_changes_on_hash_change() {
        let mut pkgs1 = BTreeMap::new();
        pkgs1.insert(
            "akeyless-cli".to_string(),
            make_pkg(vec![("0.6.1", "sha256-a", "sha256-v", Status::Verified)]),
        );
        let m1 = Matrix { packages: pkgs1 };

        let mut pkgs2 = BTreeMap::new();
        pkgs2.insert(
            "akeyless-cli".to_string(),
            make_pkg(vec![("0.6.1", "sha256-DIFFERENT", "sha256-v", Status::Verified)]),
        );
        let m2 = Matrix { packages: pkgs2 };

        assert_ne!(compute_fingerprint(&m1), compute_fingerprint(&m2));
    }

    #[test]
    fn test_delta_new_package() {
        let prev = Matrix {
            packages: BTreeMap::new(),
        };

        let mut curr_pkgs = BTreeMap::new();
        curr_pkgs.insert(
            "akeyless-cli".to_string(),
            make_pkg(vec![("1.0.0", "sha256-a", "sha256-v", Status::Verified)]),
        );
        let curr = Matrix { packages: curr_pkgs };

        let (added, updated) = compute_delta(&prev, &curr);
        assert_eq!(added, vec!["akeyless-cli@1.0.0"]);
        assert!(updated.is_empty());
    }

    #[test]
    fn test_load_save_log_roundtrip() {
        let dir = std::env::temp_dir().join("cert-test-roundtrip");
        std::fs::create_dir_all(&dir).unwrap();

        let log = CertificationLog {
            entries: vec![
                CertificationEntry {
                    id: "first-fingerprint".into(),
                    parent_id: None,
                    at: Utc::now(),
                    added: vec!["pkg@1.0".into()],
                    updated: vec![],
                    total_verified: 1,
                },
                CertificationEntry {
                    id: "second-fingerprint".into(),
                    parent_id: Some("first-fingerprint".into()),
                    at: Utc::now(),
                    added: vec!["pkg@2.0".into()],
                    updated: vec!["pkg: 1.0 -> 2.0".into()],
                    total_verified: 2,
                },
            ],
        };

        save_log(&dir, &log).unwrap();
        let loaded = load_log(&dir).unwrap();

        assert_eq!(loaded.entries.len(), 2);
        assert_eq!(loaded.entries[0].id, "first-fingerprint");
        assert!(loaded.entries[0].parent_id.is_none());
        assert_eq!(loaded.entries[1].id, "second-fingerprint");
        assert_eq!(
            loaded.entries[1].parent_id.as_deref(),
            Some("first-fingerprint")
        );
        assert_eq!(loaded.entries[1].added, vec!["pkg@2.0"]);
        assert_eq!(loaded.entries[1].updated, vec!["pkg: 1.0 -> 2.0"]);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_load_log_missing_dir() {
        let dir = std::path::Path::new("/tmp/cert-test-nonexistent-dir-xyz");
        let log = load_log(dir).unwrap();
        assert!(log.entries.is_empty());
    }

    #[test]
    fn test_record_creates_entry() {
        let dir = std::env::temp_dir().join("cert-test-record");
        std::fs::create_dir_all(&dir).unwrap();
        let cert_path = dir.join("certifications.toml");
        if cert_path.exists() {
            std::fs::remove_file(&cert_path).unwrap();
        }

        let prev = Matrix {
            packages: BTreeMap::new(),
        };
        let mut curr_pkgs = BTreeMap::new();
        curr_pkgs.insert(
            "akeyless-cli".to_string(),
            make_pkg(vec![("1.0.0", "sha256-a", "sha256-v", Status::Verified)]),
        );
        let curr = Matrix { packages: curr_pkgs };

        let entry = record(&dir, &prev, &curr).unwrap();
        assert!(!entry.id.is_empty());
        assert!(entry.parent_id.is_none());
        assert_eq!(entry.total_verified, 1);
        assert_eq!(entry.added, vec!["akeyless-cli@1.0.0"]);

        let log = load_log(&dir).unwrap();
        assert_eq!(log.entries.len(), 1);
        assert_eq!(log.entries[0].id, entry.id);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_record_skips_noop() {
        let dir = std::env::temp_dir().join("cert-test-noop");
        std::fs::create_dir_all(&dir).unwrap();
        let cert_path = dir.join("certifications.toml");
        if cert_path.exists() {
            std::fs::remove_file(&cert_path).unwrap();
        }

        let mut pkgs = BTreeMap::new();
        pkgs.insert(
            "akeyless-cli".to_string(),
            make_pkg(vec![("1.0.0", "sha256-a", "sha256-v", Status::Verified)]),
        );
        let matrix = Matrix {
            packages: pkgs.clone(),
        };

        let entry1 = record(&dir, &Matrix { packages: BTreeMap::new() }, &matrix).unwrap();

        let entry2 = record(&dir, &matrix, &matrix).unwrap();
        assert_eq!(entry1.id, entry2.id);

        let log = load_log(&dir).unwrap();
        assert_eq!(log.entries.len(), 1);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_record_chains_parent_id() {
        let dir = std::env::temp_dir().join("cert-test-chain");
        std::fs::create_dir_all(&dir).unwrap();
        let cert_path = dir.join("certifications.toml");
        if cert_path.exists() {
            std::fs::remove_file(&cert_path).unwrap();
        }

        let mut pkgs1 = BTreeMap::new();
        pkgs1.insert(
            "akeyless-cli".to_string(),
            make_pkg(vec![("1.0.0", "sha256-a", "sha256-v", Status::Verified)]),
        );
        let m1 = Matrix { packages: pkgs1 };

        let entry1 = record(
            &dir,
            &Matrix { packages: BTreeMap::new() },
            &m1,
        )
        .unwrap();

        let mut pkgs2 = BTreeMap::new();
        pkgs2.insert(
            "akeyless-cli".to_string(),
            make_pkg(vec![
                ("1.0.0", "sha256-a", "sha256-v", Status::Verified),
                ("2.0.0", "sha256-b", "sha256-w", Status::Verified),
            ]),
        );
        let m2 = Matrix { packages: pkgs2 };

        let entry2 = record(&dir, &m1, &m2).unwrap();
        assert_eq!(entry2.parent_id.as_deref(), Some(entry1.id.as_str()));
        assert_eq!(entry2.total_verified, 2);

        let log = load_log(&dir).unwrap();
        assert_eq!(log.entries.len(), 2);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_fingerprint_with_multiple_hash_types() {
        let mut vers = BTreeMap::new();
        vers.insert(
            "1.0.0".to_string(),
            VersionEntry {
                rev: "abc".into(),
                source_hash: Some("sha256-src".into()),
                vendor_hash: None,
                cargo_hash: Some("sha256-cargo".into()),
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
            language: Language::Rust,
            builder: Builder::BuildRustPackage,
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
            track: crate::matrix::TrackMode::default(),
            unstable_base: None,
            versions: vers,
        };
        let mut pkgs = BTreeMap::new();
        pkgs.insert("akeyless-rust".to_string(), pkg);
        let matrix = Matrix { packages: pkgs };

        let fp = compute_fingerprint(&matrix);
        assert_eq!(fp.len(), 64);
        assert_ne!(
            fp,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
