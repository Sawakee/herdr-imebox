//! The Ctrl+R history picker: incremental substring filtering and selection.

/// Indices into `hist` of the entries matching `query`, newest first.
/// The query is split on whitespace; an entry matches when it contains
/// every word (substring match, case-insensitive).
pub fn filter(hist: &[String], query: &str) -> Vec<usize> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    (0..hist.len())
        .rev()
        .filter(|&i| {
            let entry = hist[i].to_lowercase();
            words.iter().all(|w| entry.contains(w.as_str()))
        })
        .collect()
}

/// One line of list display: newlines flattened to a visible marker.
pub fn flatten(text: &str) -> String {
    text.replace('\n', "⏎")
}

/// State of an open picker: the query, the matches, and the selection.
pub struct Picker {
    pub query: String,
    pub matches: Vec<usize>,
    pub selected: usize,
    /// First visible list row; the renderer keeps `selected` in view.
    pub top: usize,
}

impl Picker {
    pub fn new(hist: &[String]) -> Self {
        Picker {
            query: String::new(),
            matches: filter(hist, ""),
            selected: 0,
            top: 0,
        }
    }

    fn refilter(&mut self, hist: &[String]) {
        self.matches = filter(hist, &self.query);
        self.selected = 0;
        self.top = 0;
    }

    pub fn push_char(&mut self, c: char, hist: &[String]) {
        self.query.push(c);
        self.refilter(hist);
    }

    pub fn pop_char(&mut self, hist: &[String]) {
        self.query.pop();
        self.refilter(hist);
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.matches.len() {
            self.selected += 1;
        }
    }

    /// The hist index of the selected entry, if any match.
    pub fn current(&self) -> Option<usize> {
        self.matches.get(self.selected).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hist(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_query_lists_everything_newest_first() {
        let h = hist(&["old", "mid", "new"]);
        assert_eq!(filter(&h, ""), vec![2, 1, 0]);
        assert_eq!(filter(&h, "   "), vec![2, 1, 0]);
    }

    #[test]
    fn substring_match_japanese() {
        let h = hist(&["ビルドを直して", "テストを追加", "CIのビルドを調査"]);
        assert_eq!(filter(&h, "ビルド"), vec![2, 0]);
        assert_eq!(filter(&h, "存在しない"), Vec::<usize>::new());
    }

    #[test]
    fn whitespace_words_are_anded() {
        let h = hist(&["ビルドを調査して", "ビルドを直して", "調査だけして"]);
        assert_eq!(filter(&h, "ビルド 調査"), vec![0]);
        // full-width space also separates words
        assert_eq!(filter(&h, "ビルド\u{3000}調査"), vec![0]);
    }

    #[test]
    fn ascii_match_is_case_insensitive() {
        let h = hist(&["Fix the CI build", "readme update"]);
        assert_eq!(filter(&h, "ci"), vec![0]);
        assert_eq!(filter(&h, "README"), vec![1]);
    }

    #[test]
    fn multiline_entries_match_across_the_whole_text() {
        let h = hist(&["1行目\n2行目はビルドの話"]);
        assert_eq!(filter(&h, "ビルド"), vec![0]);
    }

    #[test]
    fn flatten_replaces_newlines() {
        assert_eq!(flatten("a\nb\nc"), "a⏎b⏎c");
        assert_eq!(flatten("plain"), "plain");
    }

    #[test]
    fn new_picker_selects_the_newest_entry() {
        let h = hist(&["old", "new"]);
        let p = Picker::new(&h);
        assert_eq!(p.query, "");
        assert_eq!(p.matches, vec![1, 0]);
        assert_eq!(p.current(), Some(1));
    }

    #[test]
    fn selection_moves_and_clamps() {
        let h = hist(&["a", "b", "c"]);
        let mut p = Picker::new(&h);
        p.move_up(); // already at the top
        assert_eq!(p.selected, 0);
        p.move_down();
        p.move_down();
        p.move_down(); // clamped at the last match
        assert_eq!(p.selected, 2);
        assert_eq!(p.current(), Some(0));
        p.move_up();
        assert_eq!(p.selected, 1);
    }

    #[test]
    fn editing_the_query_refilters_and_resets_selection() {
        let h = hist(&["ビルド調査", "テスト", "ビルド修正"]);
        let mut p = Picker::new(&h);
        p.move_down();
        for c in "ビルド".chars() {
            p.push_char(c, &h);
        }
        assert_eq!(p.query, "ビルド");
        assert_eq!(p.matches, vec![2, 0]);
        assert_eq!(p.selected, 0);
        p.move_down();
        p.pop_char(&h);
        assert_eq!(p.query, "ビル");
        assert_eq!(p.selected, 0);
    }

    #[test]
    fn no_match_has_no_current_selection() {
        let h = hist(&["abc"]);
        let mut p = Picker::new(&h);
        p.push_char('z', &h);
        assert_eq!(p.matches, Vec::<usize>::new());
        assert_eq!(p.current(), None);
        p.move_down(); // must not panic on an empty list
        p.move_up();
        assert_eq!(p.current(), None);
    }
}
