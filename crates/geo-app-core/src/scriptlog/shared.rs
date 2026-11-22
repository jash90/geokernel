//! The storage of a journal that copies share: a copy (an undo step, the
//! state before a drag) costs a pointer, not the journal, and two states
//! that differ by a few steps share everything else. The history of 500
//! undo steps then takes memory in proportion to what the steps changed,
//! not 500 times the journal.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

struct Node<T> {
    item: T,
    prev: Option<Arc<Node<T>>>,
}

/// A list that grows at its end, stored newest first as a chain of shared
/// nodes: a clone shares every node, a push adds one node, replacing the
/// last item replaces one, and a change in the middle copies only the
/// items after it.
pub(crate) struct Chain<T> {
    head: Option<Arc<Node<T>>>,
    len: usize,
}

impl<T> Default for Chain<T> {
    fn default() -> Self {
        Chain { head: None, len: 0 }
    }
}

impl<T> Clone for Chain<T> {
    fn clone(&self) -> Self {
        Chain {
            head: self.head.clone(),
            len: self.len,
        }
    }
}

impl<T> Drop for Chain<T> {
    /// Frees the nodes no other copy holds one after another: dropping a
    /// long chain node by node through `Arc` would recurse once per node.
    fn drop(&mut self) {
        let mut next = self.head.take();
        while let Some(node) = next {
            next = match Arc::try_unwrap(node) {
                Ok(mut n) => n.prev.take(),
                Err(_) => None,
            };
        }
    }
}

impl<T: Clone> Chain<T> {
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn push(&mut self, item: T) {
        let prev = self.head.take();
        self.head = Some(Arc::new(Node { item, prev }));
        self.len += 1;
    }

    pub fn last(&self) -> Option<&T> {
        self.head.as_deref().map(|n| &n.item)
    }

    /// Replaces the last item (one new node; copies keep the old one).
    pub fn replace_last(&mut self, item: T) {
        if let Some(node) = self.head.take() {
            self.head = match Arc::try_unwrap(node) {
                Ok(mut n) => n.prev.take(),
                Err(shared) => shared.prev.clone(),
            };
            self.len -= 1;
        }
        self.push(item);
    }

    /// The nodes, oldest first.
    fn nodes(&self) -> Vec<&Arc<Node<T>>> {
        let mut out = Vec::with_capacity(self.len);
        let mut at = self.head.as_ref();
        while let Some(n) = at {
            out.push(n);
            at = n.prev.as_ref();
        }
        out.reverse();
        out
    }

    /// The items, newest first, without collecting them.
    pub fn items_rev(&self) -> impl Iterator<Item = &T> {
        let mut at = self.head.as_deref();
        std::iter::from_fn(move || {
            let n = at?;
            at = n.prev.as_deref();
            Some(&n.item)
        })
    }

    /// The items, oldest first.
    pub fn items(&self) -> Vec<&T> {
        self.nodes().into_iter().map(|n| &n.item).collect()
    }

    /// The first `n` items, sharing their nodes with `self`.
    pub fn prefix(&self, n: usize) -> Chain<T> {
        let nodes = self.nodes();
        Chain {
            head: n.checked_sub(1).map(|i| nodes[i].clone()),
            len: n,
        }
    }

    /// Keeps the items for which `keep` holds. The items before the first
    /// one dropped stay shared.
    pub fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        let items = self.items();
        let Some(first) = items.iter().position(|i| !keep(i)) else {
            return;
        };
        let rest: Vec<T> = items[first + 1..]
            .iter()
            .filter(|i| keep(i))
            .map(|i| (*i).clone())
            .collect();
        let mut out = self.prefix(first);
        for item in rest {
            out.push(item);
        }
        *self = out;
    }

    /// Inserts `item` at `at` (`at <= len`). The items before it stay
    /// shared; the items from `at` on are copied after it, like the tail a
    /// change in the middle copies.
    pub fn insert(&mut self, at: usize, item: T) {
        let items = self.items();
        let mut out = self.prefix(at);
        out.push(item);
        for i in &items[at..] {
            out.push((*i).clone());
        }
        *self = out;
    }

    /// The distinct nodes of `chains` (the memory they take together, in
    /// items).
    #[cfg(test)]
    pub fn distinct_nodes<'a>(chains: impl IntoIterator<Item = &'a Chain<T>>) -> usize
    where
        T: 'a,
    {
        let mut seen = std::collections::HashSet::new();
        for c in chains {
            let mut at = c.head.as_ref();
            // Newest first: the nodes before a node seen were seen with it.
            while let Some(n) = at.filter(|n| seen.insert(Arc::as_ptr(n))) {
                at = n.prev.as_ref();
            }
        }
        seen.len()
    }
}

impl<T: Clone + PartialEq> PartialEq for Chain<T> {
    fn eq(&self, other: &Self) -> bool {
        let same_head = match (&self.head, &other.head) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        self.len == other.len && (same_head || self.items() == other.items())
    }
}

impl<T: Clone + fmt::Debug> fmt::Debug for Chain<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.items()).finish()
    }
}

/// A set of labels that copies share until one of them changes it.
#[derive(Clone, Default, PartialEq)]
pub struct Labels(Arc<BTreeSet<String>>);

impl Labels {
    pub fn contains(&self, label: &str) -> bool {
        self.0.contains(label)
    }

    /// Adds `label`; returns whether it was not there.
    pub fn insert(&mut self, label: String) -> bool {
        !self.0.contains(&label) && Arc::make_mut(&mut self.0).insert(label)
    }

    /// Removes `label`; returns whether it was there.
    pub fn remove(&mut self, label: &str) -> bool {
        self.0.contains(label) && Arc::make_mut(&mut self.0).remove(label)
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&String) -> bool) {
        if !self.0.iter().all(&mut keep) {
            Arc::make_mut(&mut self.0).retain(keep);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a Labels {
    type Item = &'a String;
    type IntoIter = std::collections::btree_set::Iter<'a, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl fmt::Debug for Labels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_share_nodes_and_changes_copy_only_what_follows() {
        let mut a: Chain<u32> = Chain::default();
        for i in 0..100 {
            a.push(i);
        }
        let b = a.clone();
        let mut c = a.clone();
        c.replace_last(1000);
        let mut d = a.clone();
        d.retain(|&i| i != 90);
        assert_eq!(a.items().len(), 100);
        assert_eq!(*c.last().unwrap(), 1000);
        assert_eq!(d.len(), 99);
        assert!(d.items().iter().all(|&&i| i != 90));
        assert_eq!(a, b);
        assert_ne!(a, c);
        // 100 shared, 1 for c's last, 9 for d's tail after 90.
        assert_eq!(Chain::distinct_nodes([&a, &b, &c, &d]), 110);
        assert_eq!(a.prefix(3).items(), [&0, &1, &2]);
        assert_eq!(a.prefix(0).len(), 0);
    }

    #[test]
    fn an_insert_in_the_middle_copies_only_what_follows() {
        let mut a: Chain<u32> = Chain::default();
        for i in 0..10 {
            a.push(i);
        }
        let b = a.clone();
        let mut c = a.clone();
        c.insert(4, 40);
        assert_eq!(c.items(), [&0, &1, &2, &3, &40, &4, &5, &6, &7, &8, &9]);
        assert_eq!(a, b);
        // 10 shared, 7 for c's tail after the inserted item.
        assert_eq!(Chain::distinct_nodes([&a, &b, &c]), 17);
    }

    #[test]
    fn a_long_chain_drops_without_deep_recursion() {
        let mut a: Chain<u64> = Chain::default();
        for i in 0..1_000_000 {
            a.push(i);
        }
        let b = a.prefix(10);
        drop(a);
        assert_eq!(b.items().len(), 10);
    }

    #[test]
    fn labels_are_shared_until_changed() {
        let mut a = Labels::default();
        assert!(a.insert("A".into()));
        assert!(!a.insert("A".into()));
        let b = a.clone();
        assert!(Arc::ptr_eq(&a.0, &b.0));
        a.retain(|_| true);
        assert!(a.remove("A") && !a.remove("A"));
        assert!(b.contains("A") && a.is_empty() && b.len() == 1);
    }
}
