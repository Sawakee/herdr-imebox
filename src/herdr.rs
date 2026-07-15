//! Helpers for driving the herdr CLI and parsing its JSON output.

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Cache directory for the draft and lock files
/// (`$XDG_CACHE_HOME/herdr-imebox` or `~/.cache/herdr-imebox`).
pub fn cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache")
        });
    base.join("herdr-imebox")
}

/// Run a herdr CLI command and return its stdout.
pub fn run(args: &[&str]) -> Result<String> {
    let out = Command::new("herdr")
        .args(args)
        .output()
        .context("failed to run herdr; is it on PATH?")?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let msg = if stderr.trim().is_empty() {
            stdout.trim().to_owned()
        } else {
            stderr.trim().to_owned()
        };
        bail!("herdr {} failed: {msg}", args.join(" "));
    }
    Ok(stdout)
}

/// Pane id of the focused pane in `herdr pane list` output.
pub fn focused_pane_id(pane_list_json: &str) -> Option<String> {
    let v: Value = serde_json::from_str(pane_list_json).ok()?;
    v["result"]["panes"]
        .as_array()?
        .iter()
        .find(|p| p["focused"].as_bool() == Some(true))
        .and_then(|p| p["pane_id"].as_str().map(str::to_owned))
}

/// Whether a pane id appears in `herdr pane list` output.
pub fn pane_exists(pane_list_json: &str, pane_id: &str) -> bool {
    serde_json::from_str::<Value>(pane_list_json)
        .ok()
        .and_then(|v| {
            v["result"]["panes"]
                .as_array()
                .map(|panes| panes.iter().any(|p| p["pane_id"].as_str() == Some(pane_id)))
        })
        .unwrap_or(false)
}

/// New pane id from `herdr pane split` output.
pub fn split_pane_id(split_json: &str) -> Result<String> {
    let v: Value = serde_json::from_str(split_json).context("pane split: invalid JSON")?;
    v["result"]["pane"]["pane_id"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("pane split: no pane_id in output: {}", split_json.trim()))
}

/// Raw newlines submit immediately in most agent TUIs, so multi-line text is
/// wrapped in bracketed-paste markers to arrive as a single message.
pub fn wrap_payload(text: &str) -> String {
    if text.contains('\n') {
        format!("\x1b[200~{text}\x1b[201~")
    } else {
        text.to_owned()
    }
}

/// Send text followed by Enter to the target pane.
pub fn send_to_pane(target: &str, text: &str) -> Result<()> {
    run(&["pane", "send-text", target, &wrap_payload(text)])?;
    run(&["pane", "send-keys", target, "enter"])?;
    Ok(())
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Keybinding entry point: open the text box below the focused pane.
pub fn launch() -> Result<()> {
    let list = run(&["pane", "list"])?;
    let Some(target) = focused_pane_id(&list) else {
        return Ok(());
    };

    let dir = cache_dir();
    fs::create_dir_all(&dir)?;
    let lock = dir.join("lock");
    if let Ok(existing) = fs::read_to_string(&lock) {
        let existing = existing.trim();
        if existing == target {
            return Ok(()); // the box itself is focused
        }
        if pane_exists(&list, existing) {
            let _ = run(&["agent", "focus", existing]);
            return Ok(());
        }
        let _ = fs::remove_file(&lock); // stale lock
    }

    let split = run(&[
        "pane",
        "split",
        &target,
        "--direction",
        "down",
        "--ratio",
        "0.25",
        "--focus",
    ])?;
    let new_pane = split_pane_id(&split)?;
    let exe = std::env::current_exe()?;
    let cmd = format!(
        "exec {} edit {}",
        sh_quote(&exe.display().to_string()),
        sh_quote(&target)
    );
    run(&["pane", "run", &new_pane, &cmd])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PANE_LIST: &str = r#"{"id":"cli:pane:list","result":{"panes":[
        {"agent":"claude","focused":true,"pane_id":"w1A:p2","tab_id":"w1A:t1","workspace_id":"w1A"},
        {"focused":false,"pane_id":"w1A:p4","tab_id":"w1A:t1","workspace_id":"w1A"}
    ],"type":"pane_list"}}"#;

    #[test]
    fn finds_focused_pane() {
        assert_eq!(focused_pane_id(PANE_LIST).as_deref(), Some("w1A:p2"));
    }

    #[test]
    fn no_focused_pane() {
        let json = r#"{"result":{"panes":[{"focused":false,"pane_id":"w1A:p1"}]}}"#;
        assert_eq!(focused_pane_id(json), None);
        assert_eq!(focused_pane_id("not json"), None);
    }

    #[test]
    fn pane_existence() {
        assert!(pane_exists(PANE_LIST, "w1A:p4"));
        assert!(!pane_exists(PANE_LIST, "w1A:p9"));
        assert!(!pane_exists("not json", "w1A:p2"));
    }

    #[test]
    fn parses_split_output() {
        let json =
            r#"{"id":"cli:pane:split","result":{"pane":{"pane_id":"w1A:p5"},"type":"pane_split"}}"#;
        assert_eq!(split_pane_id(json).unwrap(), "w1A:p5");
        assert!(split_pane_id(r#"{"result":{}}"#).is_err());
    }

    #[test]
    fn wraps_multiline_only() {
        assert_eq!(wrap_payload("hello"), "hello");
        assert_eq!(wrap_payload("a\nb"), "\x1b[200~a\nb\x1b[201~");
    }

    #[test]
    fn quotes_shell_args() {
        assert_eq!(sh_quote("plain"), "'plain'");
        assert_eq!(sh_quote("it's"), "'it'\\''s'");
    }
}
