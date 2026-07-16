//! The `edit` subcommand: the pop-up text box TUI.

use crate::config::Config;
use crate::editor::Editor;
use crate::herdr;
use crate::history;
use crate::picker::{self, Picker};
use crate::wrap;
use anyhow::Result;
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind, KeyModifiers,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use std::fs;
use std::path::{Path, PathBuf};

/// Removes the lock file on every exit path, including panics.
struct LockGuard(PathBuf);

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub fn run(target: &str) -> Result<()> {
    let cfg = Config::load();
    let dir = herdr::cache_dir();
    fs::create_dir_all(&dir)?;

    // one draft per target pane, migrating the old single draft file
    let draft_path = dir.join(format!("draft-{}.txt", herdr::sanitize_id(target)));
    let legacy_draft = dir.join("draft.txt");
    if !draft_path.exists() && legacy_draft.exists() {
        let _ = fs::rename(&legacy_draft, &draft_path);
    }
    let history_path = dir.join("history.jsonl");

    let lock_path = dir.join(format!("lock-{}", herdr::sanitize_id(target)));
    if let Ok(pane_id) = std::env::var("HERDR_PANE_ID") {
        fs::write(&lock_path, pane_id)?;
    }
    let _lock = LockGuard(lock_path);

    let draft = fs::read_to_string(&draft_path).unwrap_or_default();
    let mut editor = Editor::from_text(&draft);

    let mut terminal = ratatui::init(); // raw mode + alternate screen + panic hook
    let _ = crossterm::execute!(std::io::stdout(), EnableBracketedPaste);
    // With the kitty keyboard protocol, modified keys stop leaking in as
    // escape-sequence text (Ctrl+Enter, etc. arrive as proper key events).
    let enhanced = crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false);
    if enhanced {
        let _ = crossterm::execute!(
            std::io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        );
    }
    let result = event_loop(
        &mut terminal,
        &mut editor,
        target,
        &draft_path,
        &history_path,
        &cfg,
    );
    if enhanced {
        let _ = crossterm::execute!(std::io::stdout(), PopKeyboardEnhancementFlags);
    }
    let _ = crossterm::execute!(std::io::stdout(), DisableBracketedPaste);
    ratatui::restore();
    result
}

fn save_draft(editor: &Editor, draft_path: &Path) -> Result<()> {
    if editor.is_blank() {
        let _ = fs::remove_file(draft_path);
    } else {
        fs::write(draft_path, editor.text())?;
    }
    Ok(())
}

/// Send the buffer to the target pane. `Ok` means the app should exit
/// (sent, or nothing to send); `Err` carries a status-bar message.
fn attempt_send(
    editor: &Editor,
    target: &str,
    draft_path: &Path,
    history_path: &Path,
    history_size: usize,
) -> Result<(), String> {
    if editor.is_blank() {
        let _ = fs::remove_file(draft_path);
        return Ok(());
    }
    let text = editor.text();
    // keep the text even if sending crashes
    fs::write(draft_path, &text).map_err(|e| format!("cannot save draft: {e}"))?;
    match herdr::send_to_pane(target, &text) {
        Ok(()) => {
            let _ = fs::remove_file(draft_path);
            history::append(history_path, &text, history_size);
            Ok(())
        }
        Err(e) => Err(format!("send failed: {e} (draft saved)")),
    }
}

/// Move the cursor one visual row up or down, keeping the x position.
fn move_vertical(editor: &mut Editor, delta: isize, width: usize) {
    if width == 0 {
        return;
    }
    let segments = wrap::layout(&editor.lines, width);
    let (vi, x) = wrap::find_cursor(&segments, &editor.lines, editor.row, editor.col);
    let Some(ti) = vi.checked_add_signed(delta).filter(|t| *t < segments.len()) else {
        return;
    };
    let s = segments[ti];
    editor.row = s.row;
    editor.col = wrap::col_at_x(&editor.lines[s.row], s.start, s.end, x);
}

/// The Ctrl+R history view: a query line on top, matches below (newest
/// first, selection reversed), key hints in the bar.
fn draw_picker(f: &mut ratatui::Frame, p: &mut Picker, hist: &[String], body: Rect, bar: Rect) {
    let [query_row, list] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(body);
    f.render_widget(Paragraph::new(format!("> {}", p.query)), query_row);
    let h = list.height as usize;
    if h > 0 {
        // scroll to keep the selected row visible
        if p.selected < p.top {
            p.top = p.selected;
        }
        if p.selected >= p.top + h {
            p.top = p.selected + 1 - h;
        }
        let lines: Vec<Line> = if p.matches.is_empty() {
            let msg = if hist.is_empty() {
                "(no history)"
            } else {
                "(no match)"
            };
            vec![Line::styled(
                msg,
                Style::default().add_modifier(Modifier::DIM),
            )]
        } else {
            p.matches
                .iter()
                .enumerate()
                .skip(p.top)
                .take(h)
                .map(|(i, &hi)| {
                    let line = Line::raw(picker::flatten(&hist[hi]));
                    if i == p.selected {
                        line.style(Style::default().add_modifier(Modifier::REVERSED))
                    } else {
                        line
                    }
                })
                .collect()
        };
        f.render_widget(Paragraph::new(lines), list);
    }
    // Keep the hardware cursor at the end of the query: macOS IMEs anchor
    // their candidate window to it.
    let x = 2 + wrap::width_range(&p.query, 0, p.query.chars().count());
    f.set_cursor_position(Position::new(
        query_row.x + x.min(query_row.width.saturating_sub(1) as usize) as u16,
        query_row.y,
    ));
    f.render_widget(
        Paragraph::new("Enter: use   ↑↓ ^P ^N: move   Esc / ^R: cancel")
            .style(Style::default().add_modifier(Modifier::REVERSED)),
        bar,
    );
}

fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    editor: &mut Editor,
    target: &str,
    draft_path: &Path,
    history_path: &Path,
    cfg: &Config,
) -> Result<()> {
    let send_hint = if cfg.triple_enter_send {
        "^D / Enter×3: send"
    } else {
        "^D: send"
    };
    let hint =
        format!("→ {target}   {send_hint}   ^P ^N ^R: history   ^C / Esc Esc: close (draft saved)");
    let mut status = hint.clone();
    let mut top = 0usize; // first visible visual row
    let mut body_width = 0usize; // last rendered text width, for Up/Down
    let mut esc_armed = false;
    let mut enter_streak = 0u8; // consecutive Enter presses; the 3rd sends

    let hist = history::load(history_path);
    let mut hist_pos: Option<usize> = None; // index into hist while browsing
    let mut stash = String::new(); // buffer text before browsing started
    let mut picker: Option<Picker> = None; // Some while the Ctrl+R view is open

    loop {
        terminal.draw(|f| {
            let [body, bar] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(f.area());
            if let Some(p) = picker.as_mut() {
                draw_picker(f, p, &hist, body, bar);
                return;
            }
            let (w, h) = (body.width as usize, body.height as usize);
            body_width = w;
            if w > 0 && h > 0 {
                let segments = wrap::layout(&editor.lines, w);
                let (vi, x) = wrap::find_cursor(&segments, &editor.lines, editor.row, editor.col);
                // scroll to keep the cursor's visual row visible
                if vi < top {
                    top = vi;
                }
                if vi >= top + h {
                    top = vi + 1 - h;
                }
                let lines: Vec<Line> = segments
                    .iter()
                    .skip(top)
                    .take(h)
                    .map(|s| Line::raw(wrap::slice_range(&editor.lines[s.row], s.start, s.end)))
                    .collect();
                f.render_widget(Paragraph::new(lines), body);
                // Keep the hardware cursor at the edit position: macOS IMEs
                // anchor their candidate window to it.
                f.set_cursor_position(Position::new(
                    body.x + x.min(w.saturating_sub(1)) as u16,
                    body.y + (vi - top) as u16,
                ));
            }
            f.render_widget(
                Paragraph::new(status.as_str())
                    .style(Style::default().add_modifier(Modifier::REVERSED)),
                bar,
            );
        })?;

        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                let was_armed = esc_armed;
                esc_armed = false;
                let streak = enter_streak;
                enter_streak = 0;
                if let Some(p) = picker.as_mut() {
                    match (ctrl, key.code) {
                        (true, KeyCode::Char('r' | 'c')) | (_, KeyCode::Esc) => picker = None,
                        (_, KeyCode::Enter) | (true, KeyCode::Char('j')) => {
                            if let Some(idx) = p.current() {
                                if hist_pos.is_none() {
                                    stash = editor.text();
                                }
                                *editor = Editor::from_text(&hist[idx]);
                                hist_pos = Some(idx);
                                picker = None;
                            }
                        }
                        (_, KeyCode::Up) | (true, KeyCode::Char('p')) => p.move_up(),
                        (_, KeyCode::Down) | (true, KeyCode::Char('n')) => p.move_down(),
                        (_, KeyCode::Backspace) => p.pop_char(&hist),
                        (false, KeyCode::Char(c)) => p.push_char(c, &hist),
                        _ => {}
                    }
                    continue;
                }
                match (ctrl, key.code) {
                    (true, KeyCode::Char('d')) => {
                        match attempt_send(
                            editor,
                            target,
                            draft_path,
                            history_path,
                            cfg.history_size,
                        ) {
                            Ok(()) => return Ok(()),
                            Err(msg) => status = msg,
                        }
                    }
                    (true, KeyCode::Char('c')) => return save_draft(editor, draft_path),
                    (_, KeyCode::Esc) => {
                        if was_armed {
                            return save_draft(editor, draft_path);
                        }
                        esc_armed = true; // single Esc is ignored: IMEs use it to cancel conversion
                    }
                    (true, KeyCode::Char('p')) => {
                        let pos = hist_pos.unwrap_or(hist.len());
                        if pos > 0 {
                            if hist_pos.is_none() {
                                stash = editor.text();
                            }
                            *editor = Editor::from_text(&hist[pos - 1]);
                            hist_pos = Some(pos - 1);
                        }
                    }
                    (true, KeyCode::Char('n')) => {
                        if let Some(pos) = hist_pos {
                            if pos + 1 < hist.len() {
                                *editor = Editor::from_text(&hist[pos + 1]);
                                hist_pos = Some(pos + 1);
                            } else {
                                *editor = Editor::from_text(&stash);
                                hist_pos = None;
                            }
                        }
                    }
                    (true, KeyCode::Char('r')) => picker = Some(Picker::new(&hist)),
                    (true, KeyCode::Char('a')) => editor.move_home(),
                    (true, KeyCode::Char('e')) => editor.move_end(),
                    // terminals send a raw \n as Ctrl+J
                    (_, KeyCode::Enter) | (true, KeyCode::Char('j')) => {
                        hist_pos = None;
                        if cfg.triple_enter_send && streak >= 2 {
                            // 3rd consecutive Enter sends; drop the two
                            // newlines the first two presses inserted
                            editor.backspace();
                            editor.backspace();
                            match attempt_send(
                                editor,
                                target,
                                draft_path,
                                history_path,
                                cfg.history_size,
                            ) {
                                Ok(()) => return Ok(()),
                                Err(msg) => status = msg,
                            }
                        } else {
                            editor.insert_newline();
                            enter_streak = streak + 1;
                        }
                    }
                    (_, KeyCode::Backspace) => {
                        hist_pos = None;
                        editor.backspace();
                    }
                    (_, KeyCode::Delete) => {
                        hist_pos = None;
                        editor.delete();
                    }
                    (_, KeyCode::Left) => editor.move_left(),
                    (_, KeyCode::Right) => editor.move_right(),
                    (_, KeyCode::Up) => move_vertical(editor, -1, body_width),
                    (_, KeyCode::Down) => move_vertical(editor, 1, body_width),
                    (_, KeyCode::Home) => editor.move_home(),
                    (_, KeyCode::End) => editor.move_end(),
                    (false, KeyCode::Char(c)) => {
                        hist_pos = None;
                        editor.insert_char(c);
                        status.clone_from(&hint);
                    }
                    _ => {}
                }
            }
            Event::Paste(s) => {
                enter_streak = 0;
                if let Some(p) = picker.as_mut() {
                    // paste into the query; newlines become plain spaces
                    for c in s
                        .chars()
                        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
                    {
                        p.push_char(c, &hist);
                    }
                    continue;
                }
                hist_pos = None;
                let s = s.replace("\r\n", "\n").replace('\r', "\n");
                editor.insert_str(&s);
            }
            _ => {}
        }
    }
}
