/// A node in the B-tree, which can be either a leaf (holding key→value pairs)
/// or an internal node (holding keys and child page pointers).
#[derive(Debug, Clone)]
pub struct BTreeNode {
    /// Page number that backs this node.
    pub page_no: u32,
    /// Whether this is a leaf node.
    pub is_leaf: bool,
    /// Sorted keys stored in this node.
    pub keys: Vec<Vec<u8>>,
    /// Values for leaf nodes (parallel to `keys`).
    pub values: Vec<Vec<u8>>,
    /// Child page numbers for internal nodes (len == keys.len() + 1).
    pub children: Vec<u32>,
}

impl BTreeNode {
    /// Maximum number of keys per node before a split is required.
    pub const MAX_KEYS: usize = 255;

    pub fn new_leaf(page_no: u32) -> Self {
        Self {
            page_no,
            is_leaf: true,
            keys: Vec::new(),
            values: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn new_internal(page_no: u32) -> Self {
        Self {
            page_no,
            is_leaf: false,
            keys: Vec::new(),
            values: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Search for `key` and return its value if present.
    pub fn search(&self, key: &[u8]) -> Option<&[u8]> {
        if !self.is_leaf {
            return None;
        }
        let idx = self.keys.partition_point(|k| k.as_slice() < key);
        if idx < self.keys.len() && self.keys[idx] == key {
            Some(&self.values[idx])
        } else {
            None
        }
    }

    /// Insert `key`/`value` into a leaf node. Returns the index of insertion.
    /// Caller is responsible for splitting if `keys.len() > MAX_KEYS`.
    pub fn insert_leaf(&mut self, key: Vec<u8>, value: Vec<u8>) {
        let idx = self.keys.partition_point(|k| k.as_slice() < key.as_slice());
        if idx < self.keys.len() && self.keys[idx] == key {
            // Update existing key.
            self.values[idx] = value;
        } else {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
        }
    }

    /// Return true if this node needs splitting.
    pub fn is_full(&self) -> bool {
        self.keys.len() > Self::MAX_KEYS
    }

    /// Split a full leaf node at the midpoint.
    /// Returns `(median_key, right_sibling)`.
    pub fn split_leaf(&mut self, new_page_no: u32) -> (Vec<u8>, BTreeNode) {
        let mid = self.keys.len() / 2;
        let right_keys = self.keys.split_off(mid);
        let right_values = self.values.split_off(mid);
        let median = right_keys[0].clone();
        let right = BTreeNode {
            page_no: new_page_no,
            is_leaf: true,
            keys: right_keys,
            values: right_values,
            children: Vec::new(),
        };
        (median, right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_search() {
        let mut node = BTreeNode::new_leaf(1);
        node.insert_leaf(b"hello".to_vec(), b"world".to_vec());
        assert_eq!(node.search(b"hello"), Some(b"world".as_ref()));
        assert_eq!(node.search(b"missing"), None);
    }

    #[test]
    fn test_insert_sorted() {
        let mut node = BTreeNode::new_leaf(1);
        node.insert_leaf(b"c".to_vec(), b"3".to_vec());
        node.insert_leaf(b"a".to_vec(), b"1".to_vec());
        node.insert_leaf(b"b".to_vec(), b"2".to_vec());
        assert_eq!(node.keys[0], b"a");
        assert_eq!(node.keys[1], b"b");
        assert_eq!(node.keys[2], b"c");
    }
}
