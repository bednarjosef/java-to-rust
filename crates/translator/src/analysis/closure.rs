//! What each method reaches through the calls it makes, transitively, computed once per group of
//! methods that call each other (the strongly connected parts of the call graph) instead of by
//! going over every call again until nothing changes. On a program of tens of thousands of
//! methods, the second way takes as many rounds as the longest chain of calls, each over every
//! call.

/// A set of small numbers (type or method indexes), one bit each.
#[derive(Clone, Default)]
pub struct Bits {
    words: Vec<u64>,
}

/// The same members, however many words each set has.
impl PartialEq for Bits {
    fn eq(&self, other: &Bits) -> bool {
        let n = self.words.len().max(other.words.len());
        (0..n).all(|i| self.words.get(i).unwrap_or(&0) == other.words.get(i).unwrap_or(&0))
    }
}

impl Eq for Bits {}

impl Bits {
    pub fn insert(&mut self, i: usize) -> bool {
        let (word, bit) = (i / 64, 1u64 << (i % 64));
        if self.words.len() <= word {
            self.words.resize(word + 1, 0);
        }
        let new = self.words[word] & bit == 0;
        self.words[word] |= bit;
        new
    }

    pub fn contains(&self, i: usize) -> bool {
        self.words.get(i / 64).is_some_and(|w| w & (1u64 << (i % 64)) != 0)
    }

    /// The words of the set, the bits of members 0 to 63 first; there may be empty words last.
    pub fn words(&self) -> &[u64] {
        &self.words
    }

    /// Adds every member of `other`.
    pub fn union(&mut self, other: &Bits) {
        if self.words.len() < other.words.len() {
            self.words.resize(other.words.len(), 0);
        }
        for (w, o) in self.words.iter_mut().zip(&other.words) {
            *w |= o;
        }
    }

    /// Adds every member of `other`; whether that added any.
    pub fn union_grew(&mut self, other: &Bits) -> bool {
        if self.words.len() < other.words.len() {
            self.words.resize(other.words.len(), 0);
        }
        let mut grew = false;
        for (w, o) in self.words.iter_mut().zip(&other.words) {
            grew |= o & !*w != 0;
            *w |= o;
        }
        grew
    }

    /// Adds every member of `other` that is not in `except`.
    pub fn union_except(&mut self, other: &Bits, except: &Bits) {
        if self.words.len() < other.words.len() {
            self.words.resize(other.words.len(), 0);
        }
        for (i, (w, o)) in self.words.iter_mut().zip(&other.words).enumerate() {
            *w |= o & !except.words.get(i).unwrap_or(&0);
        }
    }

    /// The members that are not in `other`, in increasing order.
    pub fn without(&self, other: &Bits) -> Vec<usize> {
        let mut out = Vec::new();
        for (i, &w) in self.words.iter().enumerate() {
            let mut w = w & !other.words.get(i).unwrap_or(&0);
            while w != 0 {
                out.push(i * 64 + w.trailing_zeros() as usize);
                w &= w - 1;
            }
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|&w| w == 0)
    }

    /// The members, in increasing order.
    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().enumerate().flat_map(|(i, &w)| {
            let mut w = w;
            std::iter::from_fn(move || {
                if w == 0 {
                    return None;
                }
                let bit = w.trailing_zeros() as usize;
                w &= w - 1;
                Some(i * 64 + bit)
            })
        })
    }
}

impl FromIterator<usize> for Bits {
    fn from_iter<I: IntoIterator<Item = usize>>(iter: I) -> Bits {
        let mut bits = Bits::default();
        for i in iter {
            bits.insert(i);
        }
        bits
    }
}

/// For each node of the graph whose edges are `next`, the union of `own` over the nodes it
/// reaches, itself included: the least fixpoint of `reach[n] = own[n] ∪ reach[m]` for every edge
/// `n → m`.
pub fn reached_union(next: &[Vec<usize>], own: &[Bits]) -> Vec<Bits> {
    let (component, components) = components(next);
    // Each component's union, from those it calls, which come before it.
    let mut unions: Vec<Bits> = Vec::with_capacity(components.len());
    for (c, members) in components.iter().enumerate() {
        let mut union = Bits::default();
        for &n in members {
            union.union(&own[n]);
            for &m in &next[n] {
                if component[m] != c {
                    union.union(&unions[component[m]]);
                }
            }
        }
        unions.push(union);
    }
    (0..next.len()).map(|n| unions[component[n]].clone()).collect()
}

/// The strongly connected components of the graph, as each node's component and the members of
/// each, ordered so that every component comes after those its edges lead to (Tarjan's
/// algorithm, without recursion).
pub fn components(next: &[Vec<usize>]) -> (Vec<usize>, Vec<Vec<usize>>) {
    const NONE: usize = usize::MAX;
    let n = next.len();
    let mut index = vec![NONE; n];
    let mut low = vec![0; n];
    let mut on_stack = vec![false; n];
    let mut stack = Vec::new();
    let mut component = vec![NONE; n];
    let mut components: Vec<Vec<usize>> = Vec::new();
    let mut counter = 0;
    // The nodes being visited, with the position of the next edge to follow.
    let mut path: Vec<(usize, usize)> = Vec::new();
    for root in 0..n {
        if index[root] != NONE {
            continue;
        }
        path.push((root, 0));
        index[root] = counter;
        low[root] = counter;
        counter += 1;
        stack.push(root);
        on_stack[root] = true;
        while let Some(&(v, edge)) = path.last() {
            if let Some(&w) = next[v].get(edge) {
                if let Some(last) = path.last_mut() {
                    last.1 += 1;
                }
                if index[w] == NONE {
                    index[w] = counter;
                    low[w] = counter;
                    counter += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    path.push((w, 0));
                } else if on_stack[w] {
                    low[v] = low[v].min(index[w]);
                }
                continue;
            }
            path.pop();
            if let Some(&(parent, _)) = path.last() {
                low[parent] = low[parent].min(low[v]);
            }
            if low[v] == index[v] {
                let mut members = Vec::new();
                loop {
                    let w = stack.pop().expect("the component's nodes are on the stack");
                    on_stack[w] = false;
                    component[w] = components.len();
                    members.push(w);
                    if w == v {
                        break;
                    }
                }
                components.push(members);
            }
        }
    }
    (component, components)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unions_follow_cycles_and_chains() {
        // 0 → 1 → 2 → 1, 3 alone, 4 → 0.
        let next = vec![vec![1], vec![2], vec![1], vec![], vec![0]];
        let own: Vec<Bits> = (0..5).map(|n| Bits::from_iter([n * 10])).collect();
        let reach = reached_union(&next, &own);
        let members = |n: usize| reach[n].iter().collect::<Vec<_>>();
        assert_eq!(members(0), vec![0, 10, 20]);
        assert_eq!(members(1), vec![10, 20]);
        assert_eq!(members(2), vec![10, 20]);
        assert_eq!(members(3), vec![30]);
        assert_eq!(members(4), vec![0, 10, 20, 40]);
    }
}
