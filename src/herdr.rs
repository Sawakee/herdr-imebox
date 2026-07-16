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

/// The herdr binary: plugin actions receive it as $HERDR_BIN_PATH.
fn herdr_bin() -> String {
    std::env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".to_owned())
}

/// Run a herdr CLI command and return its stdout.
pub fn run(args: &[&str]) -> Result<String> {
    let out = Command::new(herdr_bin())
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

/// Lock-file-safe form of a pane id (also used for draft file names).
pub fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// What `launch` should do for `target`, given the box pane id locked for
/// this target (if any), the box pane ids of every open box, and the
/// current pane list.
#[derive(Debug, PartialEq, Eq)]
pub enum LaunchPlan {
    /// The focused pane is itself a box: do nothing.
    Noop,
    /// This target already has a live box: focus it.
    Focus(String),
    /// Open a new box (any lock for this target is stale).
    Split,
}

pub fn plan_launch(
    target: &str,
    own_lock: Option<&str>,
    all_locks: &[String],
    pane_list_json: &str,
) -> LaunchPlan {
    if all_locks.iter().any(|b| b == target) {
        return LaunchPlan::Noop;
    }
    match own_lock {
        Some(b) if pane_exists(pane_list_json, b) => LaunchPlan::Focus(b.to_owned()),
        _ => LaunchPlan::Split,
    }
}

/// Keybinding entry point: open the text box below the focused pane.
pub fn launch() -> Result<()> {
    let list = run(&["pane", "list"])?;
    let Some(target) = focused_pane_id(&list) else {
        return Ok(());
    };

    let dir = cache_dir();
    fs::create_dir_all(&dir)?;
    let _ = fs::remove_file(dir.join("lock")); // pre-0.1.2 global lock

    // one lock per target pane: lock-<target> holds that target's box pane id
    let own_path = dir.join(format!("lock-{}", sanitize_id(&target)));
    let own_lock = fs::read_to_string(&own_path)
        .ok()
        .map(|s| s.trim().to_owned());
    let all_locks: Vec<String> = fs::read_dir(&dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().starts_with("lock-"))
                .filter_map(|e| fs::read_to_string(e.path()).ok())
                .map(|s| s.trim().to_owned())
                .collect()
        })
        .unwrap_or_default();
    match plan_launch(&target, own_lock.as_deref(), &all_locks, &list) {
        LaunchPlan::Noop => return Ok(()),
        LaunchPlan::Focus(box_pane) => {
            let _ = run(&["agent", "focus", &box_pane]);
            return Ok(());
        }
        LaunchPlan::Split => {
            if own_lock.is_some() {
                let _ = fs::remove_file(&own_path); // stale lock
            }
        }
    }

    // herdr's --ratio is the share kept by the ORIGINAL pane, so the box
    // gets 1 - ratio; config `ratio` is the box's share.
    let box_ratio = crate::config::Config::load().ratio.clamp(0.05, 0.9);
    let split = run(&[
        "pane",
        "split",
        &target,
        "--direction",
        "down",
        "--ratio",
        &format!("{}", 1.0 - box_ratio),
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

    // PANE_LIST panes: w1A:p2 (focused) and w1A:p4.
    fn locks(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn launch_with_no_locks_splits() {
        assert_eq!(
            plan_launch("w1A:p2", None, &[], PANE_LIST),
            LaunchPlan::Split
        );
    }

    #[test]
    fn launch_focuses_this_targets_live_box() {
        assert_eq!(
            plan_launch("w1A:p2", Some("w1A:p4"), &locks(&["w1A:p4"]), PANE_LIST),
            LaunchPlan::Focus("w1A:p4".to_owned())
        );
    }

    #[test]
    fn launch_from_inside_a_box_is_a_noop() {
        // the focused pane w1A:p2 is registered as some target's box
        assert_eq!(
            plan_launch("w1A:p2", None, &locks(&["w1A:p2"]), PANE_LIST),
            LaunchPlan::Noop
        );
    }

    #[test]
    fn another_targets_box_does_not_capture_the_launch() {
        // w1A:p4 is a box serving some other pane; w1A:p2 still gets its own
        assert_eq!(
            plan_launch("w1A:p2", None, &locks(&["w1A:p4"]), PANE_LIST),
            LaunchPlan::Split
        );
    }

    #[test]
    fn stale_lock_splits_again() {
        // this target's box pane w1A:p9 no longer exists
        assert_eq!(
            plan_launch("w1A:p2", Some("w1A:p9"), &locks(&["w1A:p9"]), PANE_LIST),
            LaunchPlan::Split
        );
    }
}
