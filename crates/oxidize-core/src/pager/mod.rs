pub mod cache;
pub mod io;

pub use cache::PageCache;
pub use io::FileIo;

use crate::Result;
use std::path::Path;

/// Page size in bytes — mirrors SQLite's default.
pub const PAGE_SIZE: usize = 4096;

/// Default LRU cache capacity (number of pages held in memory).
const DEFAULT_CACHE_CAPACITY: u64 = 256;

/// Combines FileIo + PageCache into a single page-management layer.
pub struct Pager {
    io: FileIo,
    cache: PageCache,
}

impl Pager {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let io = FileIo::open(path).await?;
        let cache = PageCache::new(DEFAULT_CACHE_CAPACITY);
        Ok(Self { io, cache })
    }

    /// Return the bytes for `page_no`, consulting cache first.
    pub async fn read_page(&mut self, page_no: u32) -> Result<Vec<u8>> {
        if let Some(cached) = self.cache.get(page_no) {
            return Ok(cached);
        }
        let data = self.io.read_page(page_no).await?;
        self.cache.insert(page_no, data.clone());
        Ok(data)
    }

    /// Write `data` to `page_no` (updates cache and file).
    pub async fn write_page(&mut self, page_no: u32, data: Vec<u8>) -> Result<()> {
        self.io.write_page(page_no, &data).await?;
        self.cache.insert(page_no, data);
        Ok(())
    }

    /// Flush pending writes to disk.
    pub async fn flush(&self) -> Result<()> {
        self.io.sync().await
    }
}
