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
        self.push(before, |_| false, true);
    }

    /// Records a step that changed no object (a view-only one, told by
    /// `is_view`): the oldest view-only step but the newest is evicted
    /// when the history grows past its bound, and with none to take the
    /// history just grows — a run of view changes never evicts a step of
    /// the construction. The redo steps are forgotten all the same.
    pub fn record_view(&mut self, before: T, is_view: impl Fn(&T) -> bool + Copy) {
        self.push(before, is_view, false);
    }

    /// Puts `before` on the undo stack. Past the bound, the first
    /// view-only step that is not the one just recorded goes (`is_view`);
    /// none being there, `evict_any` takes the oldest step instead.
    fn push(&mut self, before: T, is_view: impl Fn(&T) -> bool + Copy, evict_any: bool) {
        self.past.push(before);
        while self.past.len() > LIMIT {
            let oldest_view = self.past[..self.past.len() - 1].iter().position(is_view);
            match oldest_view.or_else(|| evict_any.then_some(0)) {
                Some(at) => {
                    self.past.remove(at);
                }
                None => break,
            }
        }
        self.future.clear();
    }

    /// Whether the step `undo` would take changed no object (`is_view`):
    /// the state the stack keeps for the way back is of the same kind, so
    /// a view-only step does not replay the journal in either direction.
    pub fn undo_is_view_only(&self, is_view: impl Fn(&T) -> bool) -> bool {
        self.past.last().is_some_and(is_view)
    }

    /// Whether the step `redo` would take changed no object (`is_view`).
    pub fn redo_is_view_only(&self, is_view: impl Fn(&T) -> bool) -> bool {
        self.future.last().is_some_and(is_view)
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// The undo and redo steps held.
    pub fn depth(&self) -> (usize, usize) {
        (self.past.len(), self.future.len())
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

    /// A run of view-only steps never evicts a step of the construction:
    /// the oldest view-only step goes first, and none being left to take,
    /// the history grows past the bound rather than dropping one.
    #[test]
    fn view_steps_are_evicted_before_construction_steps() {
        // States are numbers; 1000 and up are view-only.
        let is_view = |s: &usize| *s >= 1000;
        let mut u = Undo::default();
        for i in 0..LIMIT {
            u.record(i);
        }
        for k in 0..600 {
            u.record_view(1000 + k, is_view);
        }
        // Every construction step is still there, under the one view step
        // that stayed past the bound.
        assert_eq!(u.depth(), (LIMIT + 1, 0));
        let mut cur = LIMIT + 600;
        assert_eq!(u.undo(cur), Some(1599), "the newest view step");
        cur = 1599;
        for want in (0..LIMIT).rev() {
            match u.undo(cur) {
                Some(prev) => {
                    assert_eq!(prev, want);
                    cur = prev;
                }
                None => panic!("construction step {want} was evicted"),
            }
        }
        assert!(!u.can_undo());
        // View steps alone stay bounded.
        let mut u = Undo::default();
        for k in 0..600 {
            u.record_view(1000 + k, is_view);
        }
        assert_eq!(u.depth(), (LIMIT, 0));
        // A construction step takes the oldest step's place, which here is
        // a view-only one; the construction step stays.
        let mut u = Undo::default();
        for i in 0..LIMIT {
            u.record_view(1000 + i, is_view);
        }
        u.record(7);
        assert_eq!(u.depth(), (LIMIT, 0));
        let mut cur = 0;
        let mut saw = false;
        while let Some(prev) = u.undo(cur) {
            saw |= prev == 7;
            cur = prev;
        }
        assert!(saw, "the construction step was evicted");
        // The oldest view step (1000) went instead.
        assert_eq!(cur, 1001);
    }
}
