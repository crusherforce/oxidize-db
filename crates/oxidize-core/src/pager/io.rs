use std::path::Path;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};

use super::PAGE_SIZE;
use crate::error::OxidizeError;
use crate::Result;

/// Low-level file I/O for reading and writing fixed-size pages.
pub struct FileIo {
    file: File,
}

impl FileIo {
    /// Open (or create) a database file.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .await?;
        Ok(Self { file })
    }

    /// Read the page at `page_no` (0-indexed) into a fixed-size buffer.
    pub async fn read_page(&mut self, page_no: u32) -> Result<Vec<u8>> {
        let offset = page_no as u64 * PAGE_SIZE as u64;
        self.file.seek(SeekFrom::Start(offset)).await?;

        let mut buf = vec![0u8; PAGE_SIZE];
        let n = self.file.read(&mut buf).await?;
        if n == 0 && page_no != 0 {
            return Err(OxidizeError::CorruptDatabase(format!(
                "page {page_no} is beyond end of file"
            )));
        }
        Ok(buf)
    }

    /// Write `data` (must be PAGE_SIZE bytes) to `page_no`.
    pub async fn write_page(&mut self, page_no: u32, data: &[u8]) -> Result<()> {
        assert_eq!(
            data.len(),
            PAGE_SIZE,
            "page data must be exactly PAGE_SIZE bytes"
        );
        let offset = page_no as u64 * PAGE_SIZE as u64;
        self.file.seek(SeekFrom::Start(offset)).await?;
        self.file.write_all(data).await?;
        Ok(())
    }

    /// Flush OS buffers to disk.
    pub async fn sync(&self) -> Result<()> {
        self.file.sync_all().await?;
        Ok(())
    }
}
