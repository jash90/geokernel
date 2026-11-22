// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.
// Textbook red-black insertion fix-up reproducing the ordering of GeoGebra's Java sorted set.

use super::Budget;
use std::cmp::Ordering;

/// The reference uses a red-black tree with comparators that can report
/// Greater in both directions. Preserve its insertion search and rotations,
/// since a normal sort or global deduplication changes the locus.
pub(super) struct Tree<T> {
    nodes: Vec<Node<T>>,
    root: usize,
}

const NIL: usize = usize::MAX;

struct Node<T> {
    value: T,
    parent: usize,
    children: [usize; 2],
    red: bool,
}

impl<T> Default for Tree<T> {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            root: NIL,
        }
    }
}

impl<T: Copy> Tree<T> {
    pub(super) fn insert(
        &mut self,
        value: T,
        cmp: impl Fn(T, T) -> Ordering,
        work: &mut Budget,
    ) -> Option<bool> {
        work.charge(1)?;
        let mut parent = NIL;
        let mut side = 0;
        let mut at = self.root;
        while at != NIL {
            work.charge(1)?;
            side = match cmp(value, self.nodes[at].value) {
                Ordering::Less => 0,
                Ordering::Greater => 1,
                Ordering::Equal => return Some(false),
            };
            parent = at;
            at = self.nodes[at].children[side];
        }
        let at = self.nodes.len();
        self.nodes.push(Node {
            value,
            parent,
            children: [NIL; 2],
            red: parent != NIL,
        });
        if parent == NIL {
            self.root = at;
        } else {
            self.nodes[parent].children[side] = at;
            self.balance(at, work)?;
        }
        Some(true)
    }

    fn red(&self, node: usize) -> bool {
        self.nodes.get(node).is_some_and(|node| node.red)
    }

    fn side(&self, node: usize, parent: usize) -> usize {
        usize::from(self.nodes[parent].children[1] == node)
    }

    fn rotate(&mut self, old: usize, side: usize) {
        let parent = self.nodes[old].parent;
        let new = self.nodes[old].children[side];
        let middle = self.nodes[new].children[1 - side];
        self.nodes[old].children[side] = middle;
        if middle != NIL {
            self.nodes[middle].parent = old;
        }
        self.nodes[new].parent = parent;
        if parent == NIL {
            self.root = new;
        } else {
            let at = self.side(old, parent);
            self.nodes[parent].children[at] = new;
        }
        self.nodes[new].children[1 - side] = old;
        self.nodes[old].parent = new;
    }

    fn balance(&mut self, mut at: usize, work: &mut Budget) -> Option<()> {
        while self.red(self.nodes[at].parent) {
            work.charge(1)?;
            let mut parent = self.nodes[at].parent;
            let grand = self.nodes[parent].parent;
            let side = self.side(parent, grand);
            let uncle = self.nodes[grand].children[1 - side];
            if self.red(uncle) {
                self.nodes[parent].red = false;
                self.nodes[uncle].red = false;
                self.nodes[grand].red = true;
                at = grand;
            } else {
                if self.side(at, parent) != side {
                    self.rotate(parent, 1 - side);
                    parent = at;
                }
                self.nodes[parent].red = false;
                self.nodes[grand].red = true;
                self.rotate(grand, side);
                break;
            }
        }
        self.nodes[self.root].red = false;
        Some(())
    }

    pub(super) fn into_values(self, work: &mut Budget) -> Option<Vec<T>> {
        let mut ordered = Vec::with_capacity(self.nodes.len());
        let mut pending = Vec::new();
        let mut at = self.root;
        loop {
            work.charge(1)?;
            if at != NIL {
                pending.push(at);
                at = self.nodes[at].children[0];
            } else if let Some(i) = pending.pop() {
                ordered.push(self.nodes[i].value);
                at = self.nodes[i].children[1];
            } else {
                return Some(ordered);
            }
        }
    }
}
