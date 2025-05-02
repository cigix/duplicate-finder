//! A manager for SCCs.
//!
//! Clusterer keeps track of SCCs as links get registered.

use std::collections::{HashMap, HashSet};

use std::hash::Hash;

pub struct Clusterer<T: Clone + Eq + Hash> {
    index: usize,
    sccs: HashMap<usize, HashSet<T>>,
    entries: HashMap<T, usize>
}

impl<T: Clone + Eq + Hash> Clusterer<T> {
    pub fn new() -> Self
    {
        Clusterer {
            index: 0,
            sccs: HashMap::new(),
            entries: HashMap::new(),
        }
    }

    fn get_next_index(&mut self) -> usize
    {
        let index = self.index;
        self.index += 1;
        index
    }

    /// Add a singular entry in the Clusterer. If the entry already exists, do
    /// nothing.
    pub fn add_single(&mut self, entry: &T)
    {
        if !self.entries.contains_key(entry) {
            let mut scc = HashSet::new();
            scc.insert(entry.clone());

            let index = self.get_next_index();

            self.sccs.insert(index, scc);
            self.entries.insert(entry.clone(), index);
        }
    }

    /// Add a bidirectional link between two entries. If the entries do not
    /// exist, they are added.
    pub fn add_link(&mut self, a: &T, b: &T)
    {
        match (self.entries.get(a).copied(), self.entries.get(b).copied()) {
            (None, None) => {
                // Create a new SCC with a and b, register a and b with that SCC
                let mut scc = HashSet::new();
                scc.insert(a.clone());
                scc.insert(b.clone());

                let index = self.get_next_index();

                self.sccs.insert(index, scc);
                self.entries.insert(a.clone(), index);
                self.entries.insert(b.clone(), index);
            }
            (Some(a_index), None) => {
                // Add b to a's SCC, register b with a's SCC
                self.sccs.get_mut(&a_index).unwrap().insert(b.clone());
                self.entries.insert(b.clone(), a_index);
            }
            (None, Some(b_index)) => {
                // Add a to b's SCC, register a with b's SCC
                self.sccs.get_mut(&b_index).unwrap().insert(a.clone());
                self.entries.insert(a.clone(), b_index);
            }
            (Some(a_index), Some(b_index)) => {
                // 1. Add all members of b's SCC to a's
                // 2. Register all members of b's SCC to a's
                // 3. Delete b's SCC
                let b_scc = self.sccs.remove(&b_index).unwrap(); // 3.
                let a_scc = self.sccs.get_mut(&a_index).unwrap();
                for entry in b_scc {
                    a_scc.insert(entry.clone()); // 1.
                    self.entries.insert(entry.clone(), a_index); // 2.
                }
            }
        }
    }

    /// Consume the Clusterer to get its SCCs.
    pub fn into_sccs(self) -> Vec<HashSet<T>>
    {
        self.sccs.into_values().collect()
    }
}
