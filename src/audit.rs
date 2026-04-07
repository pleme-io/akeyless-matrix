use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use serde::Serialize;

/// Structured audit event for the evolution log.
#[derive(Debug, Clone, Serialize)]
pub struct AuditEvent {
    pub timestamp: String,
    pub event: String,
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// Audit logger that appends JSON Lines to a file.
pub struct AuditLog {
    path: PathBuf,
}

impl AuditLog {
    pub fn new(path: PathBuf) -> Self {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        Self { path }
    }

    /// Default audit log location: `~/.local/share/tend/audit.jsonl`
    ///
    /// Shared with tend so both tools write to the same evolution log.
    #[must_use]
    pub fn default_path() -> Self {
        let dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from(".local/share"))
            .join("tend");
        Self::new(dir.join("audit.jsonl"))
    }

    /// Log an event. Appends a single JSON line.
    pub fn log(&self, event: &str, data: serde_json::Value) {
        let entry = AuditEvent {
            timestamp: chrono::Utc::now().to_rfc3339(),
            event: event.to_string(),
            data,
        };
        if let Ok(line) = serde_json::to_string(&entry)
            && let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
        {
            let _ = writeln!(file, "{line}");
        }
    }

    /// Log a certify completion event.
    pub fn certify_complete(&self, package: &str, version: &str, status: &str, duration_ms: u64) {
        self.log(
            "certify_complete",
            serde_json::json!({
                "package": package,
                "version": version,
                "status": status,
                "duration_ms": duration_ms,
            }),
        );
    }

    /// Log a generation complete event.
    pub fn generation_complete(&self, backend: &str, resources: usize, artifacts: usize, duration_ms: u64) {
        self.log(
            "generation_complete",
            serde_json::json!({
                "backend": backend,
                "resources": resources,
                "artifacts": artifacts,
                "duration_ms": duration_ms,
            }),
        );
    }

}

// Methods used by the watch module and tests but not yet wired into the CLI.
// TODO(scope): move these back to the main impl block when the `watch`
// subcommand is wired into main.rs.
#[cfg(test)]
impl AuditLog {
    /// Return the path of the audit log file.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Log a version detection event.
    pub fn version_detected(
        &self,
        org: &str,
        repo: &str,
        version: &str,
        rev: &str,
        tracking: &str,
    ) {
        self.log(
            "version_detected",
            serde_json::json!({
                "org": org,
                "repo": repo,
                "version": version,
                "rev": rev,
                "tracking": tracking,
            }),
        );
    }

    /// Log a matrix entry append event.
    pub fn matrix_entry_appended(&self, package: &str, version: &str, status: &str) {
        self.log(
            "matrix_entry_appended",
            serde_json::json!({
                "package": package,
                "version": version,
                "status": status,
            }),
        );
    }

    /// Log a validation result event.
    pub fn validation_result(&self, total: usize, passed: usize, failed: usize) {
        self.log(
            "validation_result",
            serde_json::json!({
                "total": total,
                "passed": passed,
                "failed": failed,
            }),
        );
    }

    /// Log a commit push event.
    pub fn commit_pushed(&self, repo: &str, commit: &str, message: &str) {
        self.log(
            "commit_pushed",
            serde_json::json!({
                "repo": repo,
                "commit": commit,
                "message": message,
            }),
        );
    }

    /// Log a hook execution event.
    pub fn hook_executed(&self, trigger: &str, command: &str, exit_code: i32, duration_ms: u64) {
        self.log(
            "hook_executed",
            serde_json::json!({
                "trigger": trigger,
                "command": command,
                "exit_code": exit_code,
                "duration_ms": duration_ms,
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_audit_path() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join("akeyless-matrix-audit-test");
        let _ = std::fs::create_dir_all(&dir);
        dir.join(format!("audit-{}-{n}.jsonl", std::process::id()))
    }

    #[test]
    fn test_audit_log_creates_file() {
        let path = temp_audit_path();
        let audit = AuditLog::new(path.clone());
        audit.log("test_event", serde_json::json!({"key": "value"}));
        assert!(path.exists());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_audit_log_appends_valid_json() {
        let path = temp_audit_path();
        let audit = AuditLog::new(path.clone());
        audit.log("test_event", serde_json::json!({"key": "value"}));

        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert!(parsed.get("timestamp").is_some());
        assert_eq!(parsed["event"], "test_event");
        assert_eq!(parsed["key"], "value");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_audit_log_multiple_events() {
        let path = temp_audit_path();
        let audit = AuditLog::new(path.clone());
        audit.log("event_1", serde_json::json!({"a": 1}));
        audit.log("event_2", serde_json::json!({"b": 2}));
        audit.log("event_3", serde_json::json!({"c": 3}));

        let file = std::fs::File::open(&path).unwrap();
        let reader = std::io::BufReader::new(file);
        let lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();
        assert_eq!(lines.len(), 3);

        for line in &lines {
            let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(parsed.get("timestamp").is_some());
            assert!(parsed.get("event").is_some());
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_audit_log_event_fields() {
        let path = temp_audit_path();
        let audit = AuditLog::new(path.clone());
        audit.version_detected("akeylesslabs", "akeyless-go", "5.0.23", "abc123", "tags");

        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed["event"], "version_detected");
        assert_eq!(parsed["org"], "akeylesslabs");
        assert_eq!(parsed["repo"], "akeyless-go");
        assert_eq!(parsed["version"], "5.0.23");
        assert_eq!(parsed["rev"], "abc123");
        assert_eq!(parsed["tracking"], "tags");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_audit_log_default_path() {
        let audit = AuditLog::default_path();
        let path = audit.path();
        assert!(path.ends_with("tend/audit.jsonl"));
    }

    #[test]
    fn test_audit_log_convenience_methods() {
        let path = temp_audit_path();
        let audit = AuditLog::new(path.clone());

        audit.matrix_entry_appended("akeyless-go-sdk", "5.0.23", "pending");
        audit.certify_complete("akeyless-go-sdk", "5.0.23", "verified", 25000);
        audit.generation_complete("nix", 121, 242, 1500);
        audit.validation_result(121, 121, 0);
        audit.commit_pushed("blackmatter-akeyless", "abc123", "chore: certify");
        audit.hook_executed("after_certify", "test-cmd", 0, 100);

        let file = std::fs::File::open(&path).unwrap();
        let reader = std::io::BufReader::new(file);
        let lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();
        assert_eq!(lines.len(), 6);

        let events: Vec<String> = lines
            .iter()
            .map(|l| {
                let v: serde_json::Value = serde_json::from_str(l).unwrap();
                v["event"].as_str().unwrap().to_string()
            })
            .collect();
        assert_eq!(
            events,
            vec![
                "matrix_entry_appended",
                "certify_complete",
                "generation_complete",
                "validation_result",
                "commit_pushed",
                "hook_executed",
            ]
        );
        let _ = std::fs::remove_file(&path);
    }
}
