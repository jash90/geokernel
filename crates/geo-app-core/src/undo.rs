//! A snapshot undo stack with no UI types.

const LIMIT: usize = 500;

/// Undo and redo over snapshots of the state `T`.
#[derive(Debug)]
pub struct Undo<T> {
    past: Vec<T>,
    future: Vec<T>,
}

impl<T> Default for Undo<T> {
    fn default() -> Self {
        Undo {
            past: Vec::new(),
            future: Vec::new(),
        }
    }
}

impl<T> Undo<T> {
    /// Records `before` (the state prior to a change) as one undo step and
    /// forgets the redo steps.
    pub fn record(&mut self, before: T) {
        self.past.push(before);
        if self.past.len() > LIMIT {
            self.past.remove(0);
        }
        self.future.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Steps back: returns the previous state and keeps `current` for redo.
    pub fn undo(&mut self, current: T) -> Option<T> {
        let prev = self.past.pop()?;
        self.future.push(current);
        Some(prev)
    }

    /// Steps forward: returns the next state and keeps `current` for undo.
    pub fn redo(&mut self, current: T) -> Option<T> {
        let next = self.future.pop()?;
        self.past.push(current);
        Some(next)
    }

    /// Drops the newest undo step without touching the redo steps.
    pub fn discard_last(&mut self) {
        self.past.pop();
    }

    /// Forgets the redo steps.
    pub fn clear_redo(&mut self) {
        self.future.clear();
    }

    pub fn clear(&mut self) {
        self.past.clear();
        self.future.clear();
    }

    /// Every state kept, undo steps then redo steps (for the tests that
    /// measure what the history holds).
    #[cfg(test)]
    pub fn states(&self) -> impl Iterator<Item = &T> {
        self.past.iter().chain(&self.future)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_and_redo_walk_the_history() {
        let mut u = Undo::default();
        // States 0 -> 1 -> 2.
        u.record(0);
        u.record(1);
        assert!(u.can_undo() && !u.can_redo());
        assert_eq!(u.undo(2), Some(1));
        assert_eq!(u.undo(1), Some(0));
        assert_eq!(u.undo(0), None);
        assert!(u.can_redo());
        assert_eq!(u.redo(0), Some(1));
        assert_eq!(u.redo(1), Some(2));
        assert_eq!(u.redo(2), None);
    }

    #[test]
    fn a_new_change_drops_the_redo_steps() {
        let mut u = Undo::default();
        u.record(0);
        assert_eq!(u.undo(1), Some(0));
        assert!(u.can_redo());
        u.record(0);
        assert!(!u.can_redo());
    }

    #[test]
    fn history_is_bounded() {
        let mut u = Undo::default();
        for i in 0..(LIMIT + 10) {
            u.record(i);
        }
        let mut n = 0;
        let mut cur = 0;
        while let Some(prev) = u.undo(cur) {
            cur = prev;
            n += 1;
        }
        assert_eq!(n, LIMIT);
        // The oldest steps were dropped.
        assert_eq!(cur, 10);
    }
}
