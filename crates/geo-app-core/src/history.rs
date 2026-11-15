//! Recall of previously entered input lines, like a shell prompt.

/// Entered lines, oldest first, and a browsing position.
#[derive(Debug, Default)]
pub struct History {
    entries: Vec<String>,
    /// Index of the entry being shown, or `None` when not browsing.
    cursor: Option<usize>,
    /// What was in the field when browsing started.
    draft: String,
}

impl History {
    /// Records an accepted line and stops browsing. Blank lines and a repeat
    /// of the newest entry are not stored.
    pub fn push(&mut self, line: &str) {
        self.cursor = None;
        self.draft.clear();
        let line = line.trim();
        if line.is_empty() || self.entries.last().is_some_and(|l| l == line) {
            return;
        }
        self.entries.push(line.to_owned());
    }

    /// Steps to the previous (older) entry. `current` is the field's text; it
    /// is kept as a draft when browsing starts. Returns the text to show, or
    /// `None` when there is nothing older.
    pub fn up(&mut self, current: &str) -> Option<&str> {
        let next = match self.cursor {
            None if self.entries.is_empty() => return None,
            None => {
                self.draft = current.to_owned();
                self.entries.len() - 1
            }
            Some(0) => return None,
            Some(i) => i - 1,
        };
        self.cursor = Some(next);
        Some(&self.entries[next])
    }

    /// Steps to the next (newer) entry; past the newest, returns the draft
    /// and stops browsing. `None` when not browsing.
    pub fn down(&mut self) -> Option<&str> {
        let i = self.cursor?;
        if i + 1 < self.entries.len() {
            self.cursor = Some(i + 1);
            Some(&self.entries[i + 1])
        } else {
            self.cursor = None;
            Some(&self.draft)
        }
    }

    /// Forgets the browsing position, for when the user edits the text.
    pub fn stop_browsing(&mut self) {
        self.cursor = None;
    }

    #[cfg(test)]
    #[allow(clippy::len_without_is_empty)] // a test helper of an exported type
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled() -> History {
        let mut h = History::default();
        for l in ["a = (0, 0)", "b = (1, 0)", "c = (2, 0)"] {
            h.push(l);
        }
        h
    }

    #[test]
    fn up_walks_back_and_stops_at_the_oldest() {
        let mut h = filled();
        assert_eq!(h.up(""), Some("c = (2, 0)"));
        assert_eq!(h.up(""), Some("b = (1, 0)"));
        assert_eq!(h.up(""), Some("a = (0, 0)"));
        assert_eq!(h.up(""), None);
        assert_eq!(h.up(""), None);
    }

    #[test]
    fn down_walks_forward_and_restores_the_draft() {
        let mut h = filled();
        assert_eq!(h.up("half typed"), Some("c = (2, 0)"));
        assert_eq!(h.up("ignored"), Some("b = (1, 0)"));
        assert_eq!(h.down(), Some("c = (2, 0)"));
        assert_eq!(h.down(), Some("half typed"));
        assert_eq!(h.down(), None);
    }

    #[test]
    fn empty_history_does_nothing() {
        let mut h = History::default();
        assert_eq!(h.up("x"), None);
        assert_eq!(h.down(), None);
    }

    #[test]
    fn push_ignores_blank_and_repeated_lines_and_trims() {
        let mut h = History::default();
        h.push("   ");
        h.push("  p = (1, 1)  ");
        h.push("p = (1, 1)");
        assert_eq!(h.len(), 1);
        assert_eq!(h.up(""), Some("p = (1, 1)"));
    }

    #[test]
    fn push_resets_browsing() {
        let mut h = filled();
        h.up("");
        h.up("");
        h.push("d = (3, 0)");
        assert_eq!(h.up("new"), Some("d = (3, 0)"));
        assert_eq!(h.down(), Some("new"));
    }

    #[test]
    fn editing_restarts_from_the_newest_entry() {
        let mut h = filled();
        h.up("");
        h.up("");
        h.stop_browsing();
        assert_eq!(h.up("edited"), Some("c = (2, 0)"));
        assert_eq!(h.down(), Some("edited"));
    }
}
