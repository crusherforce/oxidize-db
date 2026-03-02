use super::node::BTreeNode;

/// A cursor tracks a position within a B-tree for sequential scanning.
#[derive(Debug, Clone)]
pub struct Cursor {
    /// The page number of the node the cursor is positioned on.
    pub node_page: u32,
    /// The slot index within the node (0 = first key).
    pub slot: usize,
    /// Cached copy of the current node (for read operations).
    pub node: Option<BTreeNode>,
}

impl Cursor {
    pub fn new(node_page: u32) -> Self {
        Self {
            node_page,
            slot: 0,
            node: None,
        }
    }

    /// Move to the first slot of the current node.
    pub fn rewind(&mut self) {
        self.slot = 0;
    }

    /// Check whether the cursor is past the end of the current node.
    pub fn is_done(&self) -> bool {
        match &self.node {
            Some(n) => self.slot >= n.keys.len(),
            None => true,
        }
    }

    /// Advance to the next slot.
    pub fn advance(&mut self) {
        self.slot += 1;
    }

    /// Return the key at the current position, if any.
    pub fn current_key(&self) -> Option<&[u8]> {
        self.node
            .as_ref()?
            .keys
            .get(self.slot)
            .map(|k| k.as_slice())
    }

    /// Return the value at the current position (leaf nodes only).
    pub fn current_value(&self) -> Option<&[u8]> {
        self.node
            .as_ref()?
            .values
            .get(self.slot)
            .map(|v| v.as_slice())
    }

    /// Seek to the first slot whose key is >= `target`.
    pub fn seek(&mut self, target: &[u8]) {
        if let Some(node) = &self.node {
            self.slot = node.keys.partition_point(|k| k.as_slice() < target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cursor_with_node() -> Cursor {
        let mut node = BTreeNode::new_leaf(1);
        node.insert_leaf(b"alpha".to_vec(), b"1".to_vec());
        node.insert_leaf(b"beta".to_vec(), b"2".to_vec());
        node.insert_leaf(b"gamma".to_vec(), b"3".to_vec());
        let mut cursor = Cursor::new(1);
        cursor.node = Some(node);
        cursor
    }

    #[test]
    fn test_advance() {
        let mut c = make_cursor_with_node();
        assert_eq!(c.current_key(), Some(b"alpha".as_ref()));
        c.advance();
        assert_eq!(c.current_key(), Some(b"beta".as_ref()));
        c.advance();
        assert_eq!(c.current_key(), Some(b"gamma".as_ref()));
        c.advance();
        assert!(c.is_done());
    }

    #[test]
    fn test_seek() {
        let mut c = make_cursor_with_node();
        c.seek(b"beta");
        assert_eq!(c.current_key(), Some(b"beta".as_ref()));
    }
}
