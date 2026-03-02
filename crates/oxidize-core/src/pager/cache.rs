use moka::sync::Cache;

use super::PAGE_SIZE;

/// LRU page cache: maps page number → page bytes.
pub struct PageCache {
    inner: Cache<u32, Vec<u8>>,
}

impl PageCache {
    /// Create a cache with `capacity` pages.
    pub fn new(capacity: u64) -> Self {
        let inner = Cache::new(capacity);
        Self { inner }
    }

    pub fn get(&self, page_no: u32) -> Option<Vec<u8>> {
        self.inner.get(&page_no)
    }

    pub fn insert(&self, page_no: u32, data: Vec<u8>) {
        debug_assert_eq!(data.len(), PAGE_SIZE);
        self.inner.insert(page_no, data);
    }

    pub fn invalidate(&self, page_no: u32) {
        self.inner.invalidate(&page_no);
    }

    pub fn invalidate_all(&self) {
        self.inner.invalidate_all();
    }
}
