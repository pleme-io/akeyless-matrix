# akeyless-matrix

> **★★★ CSE / Knowable Construction.** This repo operates under **Constructive Substrate Engineering** — canonical specification at [`pleme-io/theory/CONSTRUCTIVE-SUBSTRATE-ENGINEERING.md`](https://github.com/pleme-io/theory/blob/main/CONSTRUCTIVE-SUBSTRATE-ENGINEERING.md). The Compounding Directive (operational rules: solve once, load-bearing fixes only, idiom-first, models stay current, direction beats velocity) is in the org-level pleme-io/CLAUDE.md ★★★ section. Read both before non-trivial changes.


Version matrix manager for Akeyless Nix packages. Reads/writes `matrix.toml`,
extracts Nix build hashes via trial builds, generates Nix source files, and
tracks certification fingerprints.

## Commands

| Command | Purpose |
|---------|---------|
| `status` | Print all packages with version, status, date |
| `add --package KEY --version VER --rev SHA` | Add a pending version entry |
| `build [--package KEY]` | Prefetch + hash extraction for pending entries |
| `generate [--dir PATH]` | Emit Nix files from verified entries |
| `certify [--package KEY]` | Build + generate + certification fingerprint |
| `verify` | Rebuild ALL entries to validate full matrix |

All commands default to `./matrix.toml`. Override with `--matrix PATH`.

## Architecture

```
src/
├── main.rs          # clap CLI dispatch, wires concrete trait implementations
├── matrix.rs        # Data types (Matrix, Package, VersionEntry, Status, Language, Builder, TrackMode)
├── runner.rs        # CommandRunner trait (abstracts process execution)
├── storage.rs       # MatrixStore + FileWriter traits (abstracts file I/O)
├── hash.rs          # nix-prefetch-github wrapper, hash regex extraction
├── nixexpr.rs       # Shared Nix expression templates (used by build + verify)
├── build.rs         # Build pending entries: prefetch → hash extract → mark verified/broken
├── verify.rs        # Verify ALL entries by rebuilding with stored hashes
├── generate.rs      # Orchestrate writing 12 Nix files from matrix
├── nix.rs           # Nix code generators (sources, go/rust/python/ts/java/csharp/ruby/php/helm builds, metadata)
├── certification.rs # SHA-256 fingerprinting, delta tracking, audit log
├── add.rs           # Add pending version entry
├── status.rs        # Print status table
└── display.rs       # Colored terminal output
```

## Trait Abstractions

All I/O boundaries use traits for testability:

| Trait | Purpose | Production impl |
|-------|---------|-----------------|
| `CommandRunner` | Process execution (nix, nix-prefetch-github) | `SystemRunner` |
| `MatrixStore` | matrix.toml read/write | `FsMatrixStore` |
| `FileWriter` | Generated file output | `FsFileWriter` |

## Key Design Decisions

- **BTreeMap** throughout for deterministic output ordering
- **toml_edit** for writes (preserves comments/formatting), **toml** for reads (clean serde)
- **Multi-version**: all verified versions emitted as independent Nix derivations
- **Hash extraction**: exploit Nix build errors (`got: sha256-...`) as an oracle
- **Certification**: SHA-256 fingerprint of all verified (package, version, source_hash, build_hash) tuples
- **Nix escaping**: descriptions/homepages sanitized via `nix_escape()` to prevent injection

## Version Naming

| Track mode | Format | Example |
|-----------|--------|---------|
| `tags` (default) | `<tag without v>` | `1.0.0` |
| `commits` | `<base>-unstable.<date>.<sha>` | `0.1.0-unstable.2026-03-14.d240017e` |
| pleme fork | `<upstream>-pleme.<N>` | `0.1.0-pleme.1` |

## Testing

149 tests across 12 modules. Run: `cargo test`

Key test patterns:
- `MockRunner` for process execution
- `InMemoryStore` for matrix persistence
- `RecordingWriter` for file output verification
- SHA-256 known vectors for certification fingerprints
