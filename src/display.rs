use colored::Colorize;

use crate::matrix::{Matrix, Status};

/// Print the status table showing all packages, their latest version, status,
/// and verification date.
pub fn print_status_table(matrix: &Matrix) {
    println!(
        "  {:<32} {:<20} {:<12} {}",
        "Package".bold(),
        "Version".bold(),
        "Status".bold(),
        "Date".bold(),
    );
    println!("  {}", "-".repeat(80));

    for (name, pkg) in &matrix.packages {
        let (version, status, date) = match Matrix::latest_verified(pkg) {
            Some((ver, entry)) => {
                let date_str = entry
                    .verified_at
                    .map_or_else(|| "-".to_string(), |d| d.format("%Y-%m-%d").to_string());
                (ver.to_string(), entry.status, date_str)
            }
            None => {
                // Fall back to the latest version entry regardless of status
                if let Some((ver, entry)) = pkg.versions.iter().last() {
                    let date_str = entry
                        .verified_at
                        .map_or_else(|| "-".to_string(), |d| d.format("%Y-%m-%d").to_string());
                    (ver.clone(), entry.status, date_str)
                } else {
                    ("(none)".to_string(), Status::Pending, "-".to_string())
                }
            }
        };

        let status_str = format_status(status);

        println!("  {name:<32} {version:<20} {status_str:<12} {date}");
    }

    // Summary counts
    let total = matrix.packages.len();
    let verified = matrix
        .packages
        .values()
        .filter(|p| Matrix::latest_verified(p).is_some())
        .count();
    let pending = total - verified;

    println!();
    println!(
        "  {} packages, {} verified, {} pending",
        total.to_string().bold(),
        verified.to_string().green(),
        if pending > 0 {
            pending.to_string().yellow().to_string()
        } else {
            pending.to_string()
        },
    );
}

fn format_status(status: Status) -> String {
    match status {
        Status::Verified => "verified".green().to_string(),
        Status::Building => "building".yellow().to_string(),
        Status::Pending => "pending".cyan().to_string(),
        Status::Broken => "broken".red().to_string(),
    }
}

pub fn print_build_start(pkg: &str, version: &str) {
    println!(
        "  [{}] building {} {}",
        ">>".cyan(),
        pkg.bold(),
        version
    );
}

pub fn print_build_success(pkg: &str, version: &str) {
    println!(
        "  [{}] {} {} verified",
        "ok".green(),
        pkg.bold(),
        version
    );
}

pub fn print_build_failure(pkg: &str, version: &str, err: &str) {
    println!(
        "  [{}] {} {} broken: {}",
        "!!".red(),
        pkg.bold(),
        version,
        err
    );
}

pub fn print_generate_file(path: &str) {
    println!("  [{}] generated {}", "ok".green(), path);
}

pub fn print_add_success(pkg: &str, version: &str) {
    println!(
        "  [{}] added {} {} ({})",
        "ok".green(),
        pkg.bold(),
        version,
        "pending".cyan()
    );
}

pub fn print_prefetch_start(owner: &str, repo: &str, rev: &str) {
    println!(
        "  [{}] prefetching {}/{} @ {}",
        ">>".cyan(),
        owner,
        repo,
        &rev[..12.min(rev.len())]
    );
}

pub fn print_hash_extraction(hash_type: &str) {
    println!(
        "  [{}] extracting {} hash via nix build",
        ">>".cyan(),
        hash_type
    );
}

pub fn print_header(title: &str) {
    println!();
    println!("{}", title.bold());
    println!();
}

pub fn print_certification(cert: &crate::certification::CertificationEntry) {
    println!();
    println!("{}", "Certification".bold());
    println!();
    println!("  {} {}", "fingerprint:".bold(), cert.id.green());

    if let Some(ref parent) = cert.parent_id {
        let short = &parent[..parent.len().min(16)];
        println!("  {} {}", "parent:     ".bold(), short);
    } else {
        println!("  {} {}", "parent:     ".bold(), "(genesis)".cyan());
    }

    println!(
        "  {} {}",
        "verified:   ".bold(),
        cert.total_verified.to_string().green()
    );

    if cert.added.is_empty() && cert.updated.is_empty() {
        println!("  {} {}", "delta:      ".bold(), "(no changes)".cyan());
    } else {
        if !cert.added.is_empty() {
            println!("  {} {}", "added:      ".bold(), cert.added.join(", ").green());
        }
        if !cert.updated.is_empty() {
            println!(
                "  {} {}",
                "updated:    ".bold(),
                cert.updated.join(", ").yellow()
            );
        }
    }
}
