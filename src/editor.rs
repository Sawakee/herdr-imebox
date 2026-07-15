//! Minimal multi-line text editor state.
//!
//! Kept free of terminal I/O so it can be unit tested. Cursor columns are
//! character offsets; display widths (CJK double-width) are computed
//! separately for rendering.

use unicode_width::UnicodeWidthChar;

pub struct Editor {
    pub lines: Vec<String>,
    pub row: usize,
    /// Character offset within the current line.
    pub col: usize,
}

impl Editor {
    pub fn from_text(text: &str) -> Self {
        let lines: Vec<String> = if text.is_empty() {
            vec![String::new()]
        } else {
            text.split('\n').map(str::to_owned).collect()
        };
        let row = lines.len() - 1;
        let col = lines[row].chars().count();
        Self { lines, row, col }
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn is_blank(&self) -> bool {
        self.lines.iter().all(|l| l.trim().is_empty())
    }

    fn byte_idx(line: &str, col: usize) -> usize {
        line.char_indices()
            .nth(col)
            .map(|(i, _)| i)
            .unwrap_or(line.len())
    }

    fn line_len(&self, row: usize) -> usize {
        self.lines[row].chars().count()
    }

    pub fn insert_char(&mut self, c: char) {
        let i = Self::byte_idx(&self.lines[self.row], self.col);
        self.lines[self.row].insert(i, c);
        self.col += 1;
    }

    /// Insert text; newlines split lines. Callers normalize \r\n / \r first.
    pub fn insert_str(&mut self, s: &str) {
        for c in s.chars() {
            if c == '\n' {
                self.insert_newline();
            } else {
                self.insert_char(c);
            }
        }
    }

    pub fn insert_newline(&mut self) {
        let i = Self::byte_idx(&self.lines[self.row], self.col);
        let rest = self.lines[self.row].split_off(i);
        self.lines.insert(self.row + 1, rest);
        self.row += 1;
        self.col = 0;
    }

    pub fn backspace(&mut self) {
        if self.col > 0 {
            self.col -= 1;
            let i = Self::byte_idx(&self.lines[self.row], self.col);
            self.lines[self.row].remove(i);
        } else if self.row > 0 {
            let cur = self.lines.remove(self.row);
            self.row -= 1;
            self.col = self.line_len(self.row);
            self.lines[self.row].push_str(&cur);
        }
    }

    pub fn delete(&mut self) {
        if self.col < self.line_len(self.row) {
            let i = Self::byte_idx(&self.lines[self.row], self.col);
            self.lines[self.row].remove(i);
        } else if self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].push_str(&next);
        }
    }

    pub fn move_left(&mut self) {
        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.line_len(self.row);
        }
    }

    pub fn move_right(&mut self) {
        if self.col < self.line_len(self.row) {
            self.col += 1;
        } else if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        }
    }

    pub fn move_up(&mut self) {
        if self.row > 0 {
            self.row -= 1;
            self.col = self.col.min(self.line_len(self.row));
        }
    }

    pub fn move_down(&mut self) {
        if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = self.col.min(self.line_len(self.row));
        }
    }

    pub fn move_home(&mut self) {
        self.col = 0;
    }

    pub fn move_end(&mut self) {
        self.col = self.line_len(self.row);
    }

    /// Display cells between the start of the line and the cursor.
    pub fn cursor_x(&self) -> usize {
        self.lines[self.row]
            .chars()
            .take(self.col)
            .map(|c| c.width().unwrap_or(0))
            .sum()
    }
}

/// The part of `line` visible in a viewport of `width` display cells starting
/// at cell `left`. Double-width characters cut by an edge become a space.
pub fn visible_slice(line: &str, left: usize, width: usize) -> String {
    let mut out = String::new();
    let mut x = 0usize;
    for c in line.chars() {
        let w = c.width().unwrap_or(0);
        if w == 0 {
            continue;
        }
        if x + w <= left {
            x += w;
            continue;
        }
        if x < left {
            // wide char straddling the left edge
            out.push(' ');
            x += w;
            continue;
        }
        if x + w > left + width {
            if x < left + width {
                out.push(' '); // wide char straddling the right edge
            }
            break;
        }
        out.push(c);
        x += w;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_cursor_at_end() {
        let ed = Editor::from_text("ab\nこんにちは");
        assert_eq!(ed.text(), "ab\nこんにちは");
        assert_eq!((ed.row, ed.col), (1, 5));
    }

    #[test]
    fn empty_text() {
        let ed = Editor::from_text("");
        assert_eq!(ed.text(), "");
        assert!(ed.is_blank());
        assert_eq!((ed.row, ed.col), (0, 0));
    }

    #[test]
    fn blank_detection() {
        assert!(Editor::from_text(" \n\t").is_blank());
        assert!(!Editor::from_text(" \nあ").is_blank());
    }

    #[test]
    fn insert_and_newline() {
        let mut ed = Editor::from_text("");
        ed.insert_str("日本語");
        ed.insert_newline();
        ed.insert_char('a');
        assert_eq!(ed.text(), "日本語\na");
        assert_eq!((ed.row, ed.col), (1, 1));
    }

    #[test]
    fn newline_splits_line() {
        let mut ed = Editor::from_text("あい");
        ed.col = 1;
        ed.insert_newline();
        assert_eq!(ed.text(), "あ\nい");
    }

    #[test]
    fn backspace_within_and_across_lines() {
        let mut ed = Editor::from_text("あい\nう");
        ed.backspace(); // deletes う
        assert_eq!(ed.text(), "あい\n");
        ed.backspace(); // joins lines
        assert_eq!(ed.text(), "あい");
        assert_eq!((ed.row, ed.col), (0, 2));
    }

    #[test]
    fn delete_within_and_across_lines() {
        let mut ed = Editor::from_text("あい\nう");
        ed.row = 0;
        ed.col = 1;
        ed.delete(); // deletes い
        assert_eq!(ed.text(), "あ\nう");
        ed.delete(); // joins lines
        assert_eq!(ed.text(), "あう");
    }

    #[test]
    fn movement_wraps_lines_and_clamps() {
        let mut ed = Editor::from_text("あいう\na");
        ed.move_left(); // (1,0)
        ed.move_left(); // wraps to end of line 0 → (0,3)
        assert_eq!((ed.row, ed.col), (0, 3));
        ed.move_right(); // wraps to (1,0)
        assert_eq!((ed.row, ed.col), (1, 0));
        ed.move_up();
        ed.move_end();
        ed.move_down(); // col clamps to line length 1
        assert_eq!((ed.row, ed.col), (1, 1));
    }

    #[test]
    fn cursor_x_counts_cjk_double_width() {
        let mut ed = Editor::from_text("あaい");
        ed.col = 2; // after あ and a
        assert_eq!(ed.cursor_x(), 3);
    }

    #[test]
    fn visible_slice_plain() {
        assert_eq!(visible_slice("abcdef", 0, 4), "abcd");
        assert_eq!(visible_slice("abcdef", 2, 4), "cdef");
        assert_eq!(visible_slice("abc", 0, 10), "abc");
    }

    #[test]
    fn visible_slice_wide_chars() {
        // あいう = cells 0-1, 2-3, 4-5
        assert_eq!(visible_slice("あいう", 0, 4), "あい");
        assert_eq!(visible_slice("あいう", 1, 4), " い "); // あ and う cut by edges
        assert_eq!(visible_slice("あいう", 2, 4), "いう");
    }
}
