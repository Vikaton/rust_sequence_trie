//! Sequence Trie.

#![feature(slicing_syntax)]
#![allow(unstable)]

extern crate "test" as test_crate;

use std::hash::Hash;
use std::collections::hash_map::{self, HashMap, Entry};
use std::fmt::{self, Formatter, Debug};

/// A `SequenceTrie` is recursively defined as a value and a map containing child Tries.
///
/// Typically, Tries are used to store strings, which can be thought of as lists of `char`s.
/// Generalising this to any key type, a Trie is a data structure storing values for keys
/// which are themselves lists. Let the parts of such a list-key be called "key fragments".
/// In our representation of a Trie, `K` denotes the type of the key fragments.
///
/// The nesting of child Tries creates a tree structure which can be traversed by mapping
/// key fragments onto nodes. The structure is similar to a k-ary tree, except that the children
/// are stored in `HashMap`s, and there is no bound on the number of children a single node may
/// have (effectively k = ∞). In a SequenceTrie with `char` key fragments, the key
/// `['a', 'b', 'c']` might correspond to something like this (warning: not real code).
///
/// ```ignore
/// SequenceTrie {
///     value: Some(0u),
///     children: 'a' => SequenceTrie {
///         value: Some(1u),
///         children: 'b' => SequenceTrie {
///             value: None,
///             children: 'c' => SequenceTrie {
///                 value: Some(3u),
///                 children: Nil
///             }
///         }
///     }
/// }
/// ```
///
/// Values are stored optionally at each node because inserting a value for a list-key only inserts
/// a value for the last fragment of the key. The intermediate prefix nodes are created with value
/// `None` if they do not exist already.
///
/// The above SequenceTrie could be created using the following sequence of operations:
///
/// ```ignore
/// let trie: SequenceTrie<char, uint> = SequenceTrie::new();
/// trie.insert(['a', 'b', 'c'], 3u);
/// trie.insert([], 0u);
/// trie.insert(['a'], 1u);
/// ```
///
/// The order of insertion is never important.
///
/// One interesting thing about Tries is that every key is a *descendant* of the root, which itself
/// has no key fragment. Although this is a rather trivial observation, it means that every key
/// corresponds to a non-empty sequence of prefix nodes in the tree. This observation is the
/// motivation for the `get_prefix_nodes` method, which returns the nodes corresponding to the longest
/// prefix of a given key.
///
/// The empty list key, `[]`, always corresponds to the root node of the Trie.
///
/// # The Sequence Trie Invariant
/// All leaf nodes have non-trivial values (not equal to `None`). This invariant is maintained by
/// the insertion and removal methods and can be relied upon.
pub struct SequenceTrie<K, V> {
    /// Node value.
    pub value: Option<V>,

    /// Node children as a hashmap keyed by key fragments.
    pub children: HashMap<K, SequenceTrie<K, V>>
}

impl<K, V> SequenceTrie<K, V> where K: PartialEq + Eq + Hash + Clone {
    /// Create a new SequenceTrie node with no value and an empty child map.
    pub fn new() -> SequenceTrie<K, V> {
        SequenceTrie {
            value: None,
            children: HashMap::new()
        }
    }

    /// Insert a key and value into the SequenceTrie.
    ///
    /// Returns `true` if the key did not already correspond to a value.
    pub fn insert(&mut self, key: &[K], value: V) -> bool {
        let key_node = key.iter().fold(self, |current_node, fragment| {
            match current_node.children.entry(fragment.clone()) {
                Entry::Vacant(slot) => slot.insert(SequenceTrie::new()),
                Entry::Occupied(slot) => slot.into_mut()
            }
        });
        let is_new_value = match key_node.value {
            Some(_) => false,
            None => true
        };
        key_node.value = Some(value);
        is_new_value
    }

    /// Find a reference to a key's value, if it has one.
    pub fn get(&self, key: &[K]) -> Option<&V> {
        self.get_node(key).and_then(|node| node.value.as_ref())
    }

    /// Find a reference to a key's node, if it has one.
    pub fn get_node(&self, key: &[K]) -> Option<&SequenceTrie<K, V>> {
        // Recursive base case, if the key is empty, return the node.
        let fragment = match key.first() {
            Some(head) => head,
            None => return Some(self)
        };

        // Otherwise, recurse down the tree.
        match self.children.get(fragment) {
            Some(node) => node.get_node(&key[1..]),
            None => None
        }
    }

    /// Find a mutable reference to a key's value, if it has one.
    pub fn get_mut(&mut self, key: &[K]) -> Option<&mut V> {
        self.get_mut_node(key).and_then(|node| node.value.as_mut())
    }

    /// Find a mutable reference to a key's node, if it has one.
    pub fn get_mut_node(&mut self, key: &[K]) -> Option<&mut SequenceTrie<K, V>> {
        // Recursive base case, if the key is empty, return the node.
        let fragment = match key.first() {
            Some(head) => head,
            None => return Some(self)
        };

        // Otherwise, recurse down the tree.
        match self.children.get_mut(fragment) {
            Some(node) => node.get_mut_node(&key[1..]),
            None => None
        }
    }

    /// Find the longest prefix of nodes which match the given key.
    pub fn get_prefix_nodes(&self, key: &[K]) -> Vec<&SequenceTrie<K, V>> {
        let mut node_path = vec![self];

        for fragment in key.iter() {
            match node_path.last().unwrap().children.get(fragment) {
                Some(node) => node_path.push(node),
                None => break
            }
        }
        node_path
    }

    /// Find the value of the nearest ancestor with a non-empty value, if one exists.
    ///
    /// If all ancestors have empty (`None`) values, `None` is returned.
    pub fn get_ancestor(&self, key: &[K]) -> Option<&V> {
        self.get_ancestor_node(key).and_then(|node| node.value.as_ref())
    }

    /// Find the nearest ancestor with a non-empty value, if one exists.
    ///
    /// If all ancestors have empty (`None`) values, `None` is returned.
    pub fn get_ancestor_node(&self, key: &[K]) -> Option<&SequenceTrie<K, V>> {
        let node_path = self.get_prefix_nodes(key);

        for node in node_path.iter().rev() {
            if node.value.is_some() {
                return Some(*node);
            }
        }
        None
    }

    /// Remove the node corresponding to the given key.
    ///
    /// This operation is like the reverse of `insert` in that
    /// it also deletes extraneous nodes on the path from the root.
    ///
    /// If the key node has children, its value is set to `None` and no further
    /// action is taken. If the key node is a leaf, then it and its ancestors with
    /// empty values and no other children are deleted. Deletion proceeds up the tree
    /// from the key node until a node with a non-empty value or children is reached.
    ///
    /// If the key doesn't match a node in the Trie, no action is taken.
    pub fn remove(&mut self, key: &[K]) {
        self.remove_recursive(key);
    }

    /// Recursive remove method that uses the call stack to safely and
    /// efficiently remove a node and its extraneous ancestors.
    ///
    /// Return `true` if the node should be deleted.
    ///
    /// See `remove` above.
    fn remove_recursive(&mut self, key: &[K]) -> bool {
        match key.first() {
            // Base case: Leaf node, no key left to recurse on.
            None => { self.value = None; },

            // Recursive case: Inner node, delete children.
            Some(fragment) => {
                // Find the child entry in the node's hashmap.
                if let Entry::Occupied(mut entry) = self.children.entry(fragment.clone()) {
                    // Work out whether to delete the child by calling remove recursively.
                    let delete_child = entry.get_mut().remove_recursive(&key[1..]);

                    if delete_child {
                        entry.remove();
                    }
                }
                // NB: If the child isn't found, false will be returned.
                // The `self` node is either a leaf, with a non-trivial value, or an
                // inner node (with children).
            }
        }

        // If the node is childless and valueless, mark it for deletion.
        if self.children.is_empty() && self.value.is_none() {
            true
        } else {
            false
        }
    }

    /// Return an iterator over all the keys in the collection.
    ///
    /// Only nodes with values are included.
    pub fn keys<'a>(&'a self) -> Keys<'a, K, V> {
        Keys {
            stack: vec![IterItem { node: self, key: None, child_iter: self.children.keys() }]
        }
    }
}

/// An iteterator over the keys of a `SequenceTrie`.
pub struct Keys<'a, K: 'a, V: 'a> {
    stack: Vec<IterItem<'a, K, V,>>
}

struct IterItem<'a, K: 'a, V: 'a> {
    node: &'a SequenceTrie<K, V>,
    key: Option<&'a K>,
    child_iter: hash_map::Keys<'a, K, SequenceTrie<K, V>>
}

impl<'a, K, V> Iterator for Keys<'a, K, V>
where
    K: PartialEq + Eq + Hash + Clone {
    type Item = Vec<&'a K>;
    fn next(&mut self) -> Option<Vec<&'a K>> {
        loop {
            match self.stack.last().map(|x| x.node.children.is_empty()) {
                Some(true) => {
                    let result: Vec<&'a K> = self.stack.iter()
                                                        .skip(1)
                                                        .map(|x| x.key.unwrap())
                                                        .collect();
                    self.stack.pop();
                    return Some(result);
                },
                Some(false) => {
                    match self.stack.last_mut().unwrap().child_iter.next() {
                        Some(child_key) => {
                            let child = self.stack.last().unwrap().node.children.get(child_key).unwrap();
                            self.stack.push(IterItem {
                                node: child,
                                key: Some(child_key),
                                child_iter: child.children.keys()
                            });
                        }
                        None => { self.stack.pop(); }
                    }
                },
                None => return None
            }
        }
    }
}

impl<K, V> Debug for SequenceTrie<K, V>
where
    K: PartialEq + Eq + Hash + Clone + Debug,
    V: Debug {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), fmt::Error> {
        try!("Trie { value: ".fmt(fmt));
        try!(self.value.fmt(fmt));
        try!(", children: ".fmt(fmt));
        try!(self.children.fmt(fmt));
        " }".fmt(fmt)
    }
}

impl<K, V> Clone for SequenceTrie<K, V> where K: Clone, V: Clone {
    fn clone(&self) -> SequenceTrie<K, V> {
        SequenceTrie {
            value: self.value.clone(),
            children: self.children.clone()
        }
    }
}

#[cfg(test)]
mod test {
    use super::SequenceTrie;
    use std::collections::HashSet;

    fn make_trie() -> SequenceTrie<char, u32> {
        let mut trie = SequenceTrie::new();
        trie.insert(&[], 0u32);
        trie.insert(&['a'], 1u32);
        trie.insert(&['a', 'b', 'c', 'd'], 4u32);
        trie.insert(&['a', 'b', 'x', 'y'], 25u32);
        trie
    }

    #[test]
    fn get() {
        let trie = make_trie();
        let data = [
            (vec![], Some(0u32)),
            (vec!['a'], Some(1u32)),
            (vec!['a', 'b'], None),
            (vec!['a', 'b', 'c'], None),
            (vec!['a', 'b', 'x'], None),
            (vec!['a', 'b', 'c', 'd'], Some(4u32)),
            (vec!['a', 'b', 'x', 'y'], Some(25u32)),
            (vec!['b', 'x', 'y'], None)
        ];
        for &(ref key, value) in data.iter() {
            assert_eq!(trie.get(key.as_slice()), value.as_ref());
        }
    }

    #[test]
    fn get_mut() {
        let mut trie = make_trie();
        let key = ['a', 'b', 'c', 'd'];
        *trie.get_mut(&key).unwrap() = 77u32;
        assert_eq!(*trie.get(&key).unwrap(), 77u32);
    }

    #[test]
    fn get_ancestor() {
        let trie = make_trie();
        let data = [
            (vec![], 0u32),
            (vec!['a'], 1u32),
            (vec!['a', 'b'], 1u32),
            (vec!['a', 'b', 'c'], 1u32),
            (vec!['a', 'b', 'c', 'd'], 4u32),
            (vec!['a', 'b', 'x'], 1u32),
            (vec!['a', 'b', 'x', 'y'], 25u32),
            (vec!['p', 'q'], 0u32),
            (vec!['a', 'p', 'q'], 1u32)
        ];
        for &(ref key, value) in data.iter() {
            assert_eq!(*trie.get_ancestor(key.as_slice()).unwrap(), value);
        }
    }

    #[test]
    fn get_prefix_nodes() {
        let trie = make_trie();
        let prefix_nodes = trie.get_prefix_nodes(&['a', 'b', 'z']);
        // There should be 3 nodes: root, a, b.
        assert_eq!(prefix_nodes.len(), 3);
        let values = [Some(0u32), Some(1u32), None];
        for (node, value) in prefix_nodes.iter().zip(values.iter()) {
            assert_eq!(node.value, *value);
        }
    }

    #[test]
    fn remove() {
        let mut trie= make_trie();
        // If the node has children, its value should be set to `None`.
        println!("Remove ['a']");
        println!("Before: {:?}", trie);
        trie.remove(&['a']);
        println!("After: {:?}", trie);
        assert_eq!(trie.get_node(&['a']).unwrap().value, None);

        // Same as above, but for the root.
        println!("Remove []");
        println!("Before: {:?}", trie);
        trie.remove(&[]);
        println!("After: {:?}", trie);
        assert_eq!(trie.get_node(&[]).unwrap().value, None);

        // Check that lower levels are still accessible.
        assert_eq!(trie.get(&['a', 'b', 'c', 'd']), Some(&4u32));

        // Check that removing a leaf with an empty parent also
        // deletes the parent.
        println!("Remove ['a', 'b', 'c', 'd']");
        println!("Before: {:?}", trie);
        trie.remove(&['a', 'b', 'c', 'd']);
        println!("After: {:?}", trie);
        assert!(trie.get_node(&['a', 'b', 'c', 'd']).is_none());
        assert!(trie.get_node(&['a', 'b', 'c']).is_none());
        assert!(trie.get_node(&['a', 'b']).is_some());

        // Bump off the rest of the Trie!
        println!("Remove ['a', 'b', 'x', 'y']");
        println!("Before: {:?}", trie);
        trie.remove(&['a', 'b', 'x', 'y']);
        println!("After: {:?}", trie);
        assert!(trie.get_node(&['a', 'b', 'x', 'y']).is_none());
        assert!(trie.get_node(&['a', 'b', 'x']).is_none());
        assert!(trie.get_node(&['a', 'b']).is_none());
        assert!(trie.get_node(&['a']).is_none());
        assert!(trie.value.is_none());
        assert!(trie.children.is_empty());
    }

    #[test]
    fn key_iter() {
        let trie = make_trie();
        let obs_keys: HashSet<Vec<char>> = trie.keys().map(|v| -> Vec<char> {
            v.iter().map(|&&x| x).collect()
        }).collect();
        let mut exp_keys: HashSet<Vec<char>> = HashSet::new();
        exp_keys.insert(vec!['a', 'b', 'c', 'd']);
        exp_keys.insert(vec!['a', 'b', 'x', 'y']);
        assert_eq!(exp_keys, obs_keys);
    }

    #[derive(PartialEq, Eq, Hash, Clone)]
    struct Key {
        field: usize
    }

    #[test]
    fn struct_key() {
        SequenceTrie::<Key, usize>::new();
    }
}

#[cfg(test)]
mod benchmark {
    use super::SequenceTrie;
    use std::collections::HashMap;
    use test_crate::Bencher;

    macro_rules! u32_benchmark {
        ($map_constructor: expr, $test_id: ident, $num_keys: expr, $key_length: expr) => (
            #[bench]
            fn $test_id(b: &mut Bencher) {
                let mut test_data = Vec::<Vec<u32>>::with_capacity($num_keys);
                let mut map = $map_constructor;
                for i in range(0, $num_keys) {
                    let mut key = Vec::<u32>::with_capacity($key_length);
                    for j in range(0, $key_length) {
                        key[j] = (i * j) as u32;
                    }

                    test_data[i] = key;
                }

                b.iter(|| {
                    for key in &test_data {
                        map.insert(key.as_slice(), 7u32);
                    }
                });
            }
        )
    }

    u32_benchmark! { HashMap::new(), hashmap_k1024_l16, 1024, 16 }
    u32_benchmark! { SequenceTrie::new(), trie_k1024_l16, 1024, 16 }

    u32_benchmark! { HashMap::new(), hashmap_k64_l128, 64, 128 }
    u32_benchmark! { SequenceTrie::new(), trie_k64_l128, 64, 128 }
}
