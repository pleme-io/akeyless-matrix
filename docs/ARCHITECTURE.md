# akeyless-matrix Architecture

## Problem

18 buildable Akeyless packages across Go, Rust, Python, and TypeScript. Each version
update requires: finding the new upstream tag, prefetching the source hash, computing
the vendor/cargo/npm hash via trial nix build, updating Nix files, and propagating
to the nix repo. Tedious and error-prone for 18 packages across 82 repos.

## Solution

A TOML matrix (`matrix.toml`) as the single source of truth, with a Rust CLI that
automates hash extraction and Nix file generation. Combined with tend's watch daemon,
the entire lifecycle is fully automated.

## Data Flow

```
matrix.toml (source of truth)
  │
  │  akeyless-matrix certify
  │    1. nix-prefetch-github → source_hash
  │    2. nix build (dummy hash) → stderr → vendor/cargo/npm hash
  │    3. mark verified, record certification fingerprint
  │
  │  akeyless-matrix generate
  │    4. emit lib/sources.nix (all verified versions)
  │    5. emit builds/{go,rust,python,typescript}/default.nix
  │    6. emit lib/matrix-metadata.nix (packageSourceMap + tier lists)
  │
  ▼
blackmatter-akeyless flake.nix
  │  imports matrix-metadata.nix
  │  generates unversioned + versioned Nix attributes
  │
  ▼
nix repo → rebuild → tools in PATH
```

## Matrix Format

```toml
[packages.akeyless-cli]
owner = "akeylesslabs"
repo = "cli"
language = "go"
builder = "mkGoTool"
tier = 1
track = "tags"                    # or "commits"
sub_packages = ["cmd/clint"]
description = "Akeyless CLI"
homepage = "https://github.com/akeylesslabs/cli"

  [packages.akeyless-cli.versions."0.6.1"]
  rev = "731e5bd..."
  source_hash = "sha256-..."
  vendor_hash = "sha256-..."
  status = "verified"             # pending | building | verified | broken
  verified_at = "2026-03-14T19:00:00Z"
```

## Hash Extraction Strategy

The key insight: Nix itself tells you the correct hash when you provide a wrong one.

1. Build with `vendorHash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="`
2. Build fails: `error: hash mismatch ... got: sha256-RealHash=`
3. Regex captures the real hash from stderr
4. Rebuild with real hash — succeeds

This eliminates manual hash computation entirely. Works for Go (vendorHash),
Rust (cargoHash), and TypeScript (npmDepsHash).

## Multi-Version Support

All verified versions are emitted as independent Nix derivations:

```
sources.nix:
  cli       = { version = "0.7.0"; ... }   # latest
  cli-0_6_1 = { version = "0.6.1"; ... }   # historical

builds/go/default.nix:
  akeyless-cli       = mkAkeylessTool "cli" { vendorHash = "new"; }
  akeyless-cli-0_6_1 = mkAkeylessTool "cli-0_6_1" { vendorHash = "old"; }
```

Each version has its own source hash and build hash. Consumers pin via
`pkgs.akeyless-cli-0_6_1`. Old versions are only removed when explicitly
deleted from matrix.toml.

## Certification Tracking

Each `certify` run produces:
- **Fingerprint**: SHA-256 of all `(package, version, source_hash, build_hash)` tuples
- **Parent chain**: each entry links to its parent fingerprint
- **Delta**: newly certified versions + latest-version bumps
- **Log**: append-only `certifications.toml` (duplicate fingerprints skipped)

## Trait Architecture

```
main.rs
  ├── SystemRunner    ← CommandRunner trait (process execution)
  ├── FsMatrixStore   ← MatrixStore trait (matrix.toml I/O)
  └── FsFileWriter    ← FileWriter trait (generated file output)
        │
        ├── build.rs    uses CommandRunner + MatrixStore
        ├── verify.rs   uses CommandRunner + MatrixStore
        ├── add.rs      uses MatrixStore
        ├── status.rs   uses MatrixStore
        └── generate.rs uses MatrixStore + FileWriter
```

Tests substitute MockRunner, InMemoryStore, RecordingWriter.

## Self-Healing

```
Cycle 1: v1.2.0 → pending → certify → BROKEN (missing go.sum)
Cycle N: upstream fixes, tags v1.2.1
         → new pending entry → certify → VERIFIED
         → generates Nix with 1.2.1 (broken 1.2.0 excluded)
```

Broken entries never block the pipeline. New tags create fresh pending entries.
