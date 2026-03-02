use crate::pager::PAGE_SIZE;
use crate::Result;

/// A single WAL entry: a snapshot of one page.
#[derive(Debug, Clone)]
pub struct WalEntry {
    /// The transaction ID that wrote this entry.
    pub tx_id: u64,
    /// The page number being recorded.
    pub page_no: u32,
    /// The full page contents at the time of the write.
    pub data: Box<[u8; PAGE_SIZE]>,
}

/// Write-Ahead Log (WAL) for crash recovery and ACID durability.
///
/// The WAL records page-level changes before they are written to the
/// main database file. On crash recovery, WAL entries are replayed
/// to bring the database to a consistent state.
pub struct Wal {
    /// In-memory log of uncommitted entries.
    entries: Vec<WalEntry>,
    /// Current transaction ID counter.
    next_tx_id: u64,
}

impl Wal {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_tx_id: 1,
        }
    }

    /// Begin a new transaction and return its ID.
    pub fn begin_transaction(&mut self) -> u64 {
        let id = self.next_tx_id;
        self.next_tx_id += 1;
        id
    }

    /// Append a page write to the WAL for the given transaction.
    pub fn append(&mut self, tx_id: u64, page_no: u32, data: &[u8]) -> Result<()> {
        assert_eq!(
            data.len(),
            PAGE_SIZE,
            "WAL entry must be exactly PAGE_SIZE bytes"
        );
        let mut page_data = Box::new([0u8; PAGE_SIZE]);
        page_data.copy_from_slice(data);
        self.entries.push(WalEntry {
            tx_id,
            page_no,
            data: page_data,
        });
        Ok(())
    }

    /// Return WAL entries that should be applied to restore a consistent state.
    pub fn replay(&self) -> impl Iterator<Item = &WalEntry> {
        self.entries.iter()
    }

    /// Move committed WAL entries back to the main database file and truncate.
    ///
    /// This is a stub — full implementation requires integrating with Pager.
    pub fn checkpoint(&mut self) -> Result<()> {
        // TODO: flush each entry to the main database file via Pager,
        //       then truncate the WAL.
        self.entries.clear();
        Ok(())
    }

    /// Number of pending (un-checkpointed) WAL entries.
    pub fn pending_count(&self) -> usize {
        self.entries.len()
    }
}

impl Default for Wal {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_and_replay() {
        let mut wal = Wal::new();
        let tx = wal.begin_transaction();
        let page = vec![0xABu8; PAGE_SIZE];
        wal.append(tx, 0, &page).unwrap();
        assert_eq!(wal.pending_count(), 1);
        let entry = wal.replay().next().unwrap();
        assert_eq!(entry.page_no, 0);
        assert_eq!(entry.data[0], 0xAB);
    }

    #[test]
    fn test_checkpoint_clears_entries() {
        let mut wal = Wal::new();
        let tx = wal.begin_transaction();
        let page = vec![0u8; PAGE_SIZE];
        wal.append(tx, 1, &page).unwrap();
        wal.checkpoint().unwrap();
        assert_eq!(wal.pending_count(), 0);
    }
}
