//! Soft-wrap layout: mapping logical lines to visual rows, CJK-width aware.

use unicode_width::UnicodeWidthChar;

/// One visual row: the char range `[start, end)` of logical line `row`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    pub row: usize,
    pub start: usize,
    pub end: usize,
}

/// Char-offset ranges of the visual rows of one logical line.
pub fn wrap_ranges(line: &str, width: usize) -> Vec<(usize, usize)> {
    let width = width.max(1);
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut x = 0usize;
    let mut count = 0usize;
    for (i, c) in line.chars().enumerate() {
        let w = c.width().unwrap_or(0);
        if x + w > width && x > 0 {
            out.push((start, i));
            start = i;
            x = 0;
        }
        x += w;
        count = i + 1;
    }
    out.push((start, count));
    out
}

/// Visual rows of the whole buffer at the given width.
pub fn layout(lines: &[String], width: usize) -> Vec<Segment> {
    let mut segments = Vec::new();
    for (row, line) in lines.iter().enumerate() {
        for (start, end) in wrap_ranges(line, width) {
            segments.push(Segment { row, start, end });
        }
    }
    segments
}

/// The chars of `line` in `[start, end)`.
pub fn slice_range(line: &str, start: usize, end: usize) -> String {
    line.chars().skip(start).take(end - start).collect()
}

/// Display width of the chars of `line` in `[start, end)`.
pub fn width_range(line: &str, start: usize, end: usize) -> usize {
    line.chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .map(|c| c.width().unwrap_or(0))
        .sum()
}

/// Visual row index and x-offset of the cursor at logical (row, col).
/// A cursor sitting exactly on a wrap boundary belongs to the next segment.
pub fn find_cursor(
    segments: &[Segment],
    lines: &[String],
    row: usize,
    col: usize,
) -> (usize, usize) {
    for (i, s) in segments.iter().enumerate() {
        if s.row != row {
            continue;
        }
        let last_of_row = i + 1 == segments.len() || segments[i + 1].row != row;
        if col < s.end || last_of_row {
            let col = col.clamp(s.start, s.end);
            return (i, width_range(&lines[row], s.start, col));
        }
    }
    (0, 0)
}

/// Char offset within `[start, end)` whose display x is closest to `x`
/// without passing it.
pub fn col_at_x(line: &str, start: usize, end: usize, x: usize) -> usize {
    let mut acc = 0usize;
    let mut col = start;
    for c in line.chars().skip(start).take(end - start) {
        let w = c.width().unwrap_or(0);
        if acc + w > x {
            break;
        }
        acc += w;
        col += 1;
    }
    col
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn wrap_ascii() {
        assert_eq!(wrap_ranges("abcdef", 4), vec![(0, 4), (4, 6)]);
        assert_eq!(wrap_ranges("abcd", 4), vec![(0, 4)]);
        assert_eq!(wrap_ranges("", 4), vec![(0, 0)]);
    }

    #[test]
    fn wrap_cjk_never_splits_wide_chars() {
        // each char is 2 cells; width 5 fits two chars (4 cells) per row
        assert_eq!(wrap_ranges("あいうえお", 5), vec![(0, 2), (2, 4), (4, 5)]);
    }

    #[test]
    fn layout_multiple_lines() {
        let l = lines(&["abcdef", "", "あい"]);
        let segs = layout(&l, 4);
        assert_eq!(
            segs,
            vec![
                Segment {
                    row: 0,
                    start: 0,
                    end: 4
                },
                Segment {
                    row: 0,
                    start: 4,
                    end: 6
                },
                Segment {
                    row: 1,
                    start: 0,
                    end: 0
                },
                Segment {
                    row: 2,
                    start: 0,
                    end: 2
                },
            ]
        );
    }

    #[test]
    fn slice_and_width() {
        assert_eq!(slice_range("あいう", 1, 3), "いう");
        assert_eq!(width_range("あaい", 0, 2), 3);
    }

    #[test]
    fn cursor_positions() {
        let l = lines(&["abcdef", "あい"]);
        let segs = layout(&l, 4);
        assert_eq!(find_cursor(&segs, &l, 0, 2), (0, 2));
        // on the wrap boundary → start of the continuation row
        assert_eq!(find_cursor(&segs, &l, 0, 4), (1, 0));
        assert_eq!(find_cursor(&segs, &l, 0, 6), (1, 2));
        assert_eq!(find_cursor(&segs, &l, 1, 1), (2, 2));
    }

    #[test]
    fn col_from_x_snaps_to_char_start() {
        // あ=cells 0-1, い=cells 2-3
        assert_eq!(col_at_x("あい", 0, 2, 0), 0);
        assert_eq!(col_at_x("あい", 0, 2, 1), 0); // inside あ
        assert_eq!(col_at_x("あい", 0, 2, 2), 1);
        assert_eq!(col_at_x("あい", 0, 2, 9), 2); // past the end
    }
}
