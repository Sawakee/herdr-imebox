//! The `edit` subcommand: the pop-up text box TUI.

use crate::editor::{Editor, visible_slice};
use crate::herdr;
use anyhow::Result;
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind, KeyModifiers,
};
use ratatui::layout::{Constraint, Layout, Position};
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
    let dir = herdr::cache_dir();
    fs::create_dir_all(&dir)?;
    let draft_path = dir.join("draft.txt");
    let lock_path = dir.join("lock");
    if let Ok(pane_id) = std::env::var("HERDR_PANE_ID") {
        fs::write(&lock_path, pane_id)?;
    }
    let _lock = LockGuard(lock_path);

    let draft = fs::read_to_string(&draft_path).unwrap_or_default();
    let mut editor = Editor::from_text(&draft);

    let mut terminal = ratatui::init(); // raw mode + alternate screen + panic hook
    let _ = crossterm::execute!(std::io::stdout(), EnableBracketedPaste);
    let result = event_loop(&mut terminal, &mut editor, target, &draft_path);
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

fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    editor: &mut Editor,
    target: &str,
    draft_path: &Path,
) -> Result<()> {
    let hint = format!(
        "→ {target}   Ctrl+D: send   Ctrl+C / Esc Esc: save draft & close   Enter: newline"
    );
    let mut status = hint.clone();
    let mut top = 0usize; // first visible row
    let mut left = 0usize; // horizontal scroll in display cells
    let mut esc_armed = false;

    loop {
        terminal.draw(|f| {
            let [body, bar] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(f.area());
            let (w, h) = (body.width as usize, body.height as usize);
            if w > 0 && h > 0 {
                // scroll to keep the cursor visible
                if editor.row < top {
                    top = editor.row;
                }
                if editor.row >= top + h {
                    top = editor.row + 1 - h;
                }
                let cx = editor.cursor_x();
                if cx < left {
                    left = cx;
                }
                if cx >= left + w {
                    left = cx + 1 - w;
                }
                let lines: Vec<Line> = editor
                    .lines
                    .iter()
                    .skip(top)
                    .take(h)
                    .map(|l| Line::raw(visible_slice(l, left, w)))
                    .collect();
                f.render_widget(Paragraph::new(lines), body);
                // Keep the hardware cursor at the edit position: macOS IMEs
                // anchor their candidate window to it.
                f.set_cursor_position(Position::new(
                    body.x + (cx - left) as u16,
                    body.y + (editor.row - top) as u16,
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
                match (ctrl, key.code) {
                    (true, KeyCode::Char('d')) => {
                        if editor.is_blank() {
                            let _ = fs::remove_file(draft_path);
                            return Ok(());
                        }
                        let text = editor.text();
                        fs::write(draft_path, &text)?; // keep the text even if sending crashes
                        match herdr::send_to_pane(target, &text) {
                            Ok(()) => {
                                let _ = fs::remove_file(draft_path);
                                return Ok(());
                            }
                            Err(e) => status = format!("send failed: {e} (draft saved)"),
                        }
                    }
                    (true, KeyCode::Char('c')) => return save_draft(editor, draft_path),
                    (_, KeyCode::Esc) => {
                        if was_armed {
                            return save_draft(editor, draft_path);
                        }
                        esc_armed = true; // single Esc is ignored: IMEs use it to cancel conversion
                    }
                    (true, KeyCode::Char('a')) => editor.move_home(),
                    (true, KeyCode::Char('e')) => editor.move_end(),
                    // terminals send a raw \n as Ctrl+J
                    (_, KeyCode::Enter) | (true, KeyCode::Char('j')) => editor.insert_newline(),
                    (_, KeyCode::Backspace) => editor.backspace(),
                    (_, KeyCode::Delete) => editor.delete(),
                    (_, KeyCode::Left) => editor.move_left(),
                    (_, KeyCode::Right) => editor.move_right(),
                    (_, KeyCode::Up) => editor.move_up(),
                    (_, KeyCode::Down) => editor.move_down(),
                    (_, KeyCode::Home) => editor.move_home(),
                    (_, KeyCode::End) => editor.move_end(),
                    (false, KeyCode::Char(c)) => {
                        editor.insert_char(c);
                        status.clone_from(&hint);
                    }
                    _ => {}
                }
            }
            Event::Paste(s) => {
                let s = s.replace("\r\n", "\n").replace('\r', "\n");
                editor.insert_str(&s);
            }
            _ => {}
        }
    }
}
