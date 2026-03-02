pub mod cursor;
pub mod node;
pub mod page;

pub use cursor::Cursor;
pub use node::BTreeNode;
pub use page::{PageHeader, PageType};

use crate::error::OxidizeError;
use crate::pager::Pager;
use crate::Result;

/// A B-tree index backed by a Pager.
///
/// Currently this is a stub that holds the in-memory root node only.
/// Future work: persist nodes via Pager, implement proper split/merge.
pub struct BTree {
    /// The pager used for persistent storage (None in unit-test stubs).
    _pager: Option<Pager>,
    /// Root node held in memory for the stub implementation.
    root: BTreeNode,
    /// Next page number to allocate.
    next_page: u32,
}

impl BTree {
    pub async fn open(pager: Pager) -> Result<Self> {
        Ok(Self {
            _pager: Some(pager),
            root: BTreeNode::new_leaf(0),
            next_page: 1,
        })
    }

    /// Look up a key and return its value.
    pub fn get(&self, key: &[u8]) -> Option<&[u8]> {
        self.root.search(key)
    }

    /// Insert or update a key→value mapping.
    pub fn insert(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        self.root.insert_leaf(key, value);
        if self.root.is_full() {
            // Stub: allocate page numbers for the split, but don't persist yet.
            let new_page = self.next_page;
            self.next_page += 1;
            let (_median, _right) = self.root.split_leaf(new_page);
            // TODO: promote median to a new internal root and persist.
        }
        Ok(())
    }

    /// Delete a key. Returns `KeyNotFound` if absent.
    pub fn delete(&mut self, key: &[u8]) -> Result<()> {
        let idx = self.root.keys.partition_point(|k| k.as_slice() < key);
        if idx < self.root.keys.len() && self.root.keys[idx] == key {
            self.root.keys.remove(idx);
            self.root.values.remove(idx);
            Ok(())
        } else {
            Err(OxidizeError::KeyNotFound)
        }
    }

    /// Return a cursor positioned at the first entry >= `start_key`.
    pub fn scan(&self, start_key: Option<&[u8]>) -> Cursor {
        let mut cursor = Cursor::new(self.root.page_no);
        cursor.node = Some(self.root.clone());
        if let Some(key) = start_key {
            cursor.seek(key);
        }
        cursor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_btree() -> BTree {
        BTree {
            _pager: None,
            root: BTreeNode::new_leaf(0),
            next_page: 1,
        }
    }

    #[test]
    fn test_insert_get() {
        let mut bt = make_btree();
        bt.insert(b"foo".to_vec(), b"bar".to_vec()).unwrap();
        assert_eq!(bt.get(b"foo"), Some(b"bar".as_ref()));
    }

    #[test]
    fn test_delete() {
        let mut bt = make_btree();
        bt.insert(b"key".to_vec(), b"val".to_vec()).unwrap();
        bt.delete(b"key").unwrap();
        assert_eq!(bt.get(b"key"), None);
    }

    #[test]
    fn test_delete_not_found() {
        let mut bt = make_btree();
        let res = bt.delete(b"ghost");
        assert!(matches!(res, Err(OxidizeError::KeyNotFound)));
    }
}
