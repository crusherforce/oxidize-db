use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::Cursor;

use crate::pager::PAGE_SIZE;

/// Page types matching SQLite's b-tree page layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PageType {
    /// Interior (internal) b-tree page: holds keys and child page pointers.
    Internal = 0x05,
    /// Leaf b-tree page: holds keys and their associated data.
    Leaf = 0x0D,
    /// Overflow page for data that doesn't fit in a single page.
    Overflow = 0x00,
}

impl TryFrom<u8> for PageType {
    type Error = crate::error::OxidizeError;

    fn try_from(value: u8) -> crate::Result<Self> {
        match value {
            0x05 => Ok(PageType::Internal),
            0x0D => Ok(PageType::Leaf),
            0x00 => Ok(PageType::Overflow),
            other => Err(crate::error::OxidizeError::CorruptDatabase(format!(
                "unknown page type byte: {other:#04x}"
            ))),
        }
    }
}

/// Fixed-size page header stored at the start of every b-tree page.
#[derive(Debug, Clone)]
pub struct PageHeader {
    pub page_type: PageType,
    /// Byte offset of the first freeblock, or 0 if none.
    pub first_freeblock: u16,
    /// Number of cells on this page.
    pub cell_count: u16,
    /// Byte offset of the start of the cell content area.
    pub cell_content_start: u16,
    /// Number of fragmented free bytes within the cell content area.
    pub fragmented_free_bytes: u8,
    /// Right-most child page number (internal pages only).
    pub right_child: u32,
}

impl PageHeader {
    pub const SIZE: usize = 12; // bytes

    pub fn read(data: &[u8]) -> crate::Result<Self> {
        let mut cur = Cursor::new(data);
        let page_type = PageType::try_from(cur.read_u8()?)?;
        let first_freeblock = cur.read_u16::<BigEndian>()?;
        let cell_count = cur.read_u16::<BigEndian>()?;
        let cell_content_start = cur.read_u16::<BigEndian>()?;
        let fragmented_free_bytes = cur.read_u8()?;
        let right_child = cur.read_u32::<BigEndian>()?;

        Ok(Self {
            page_type,
            first_freeblock,
            cell_count,
            cell_content_start,
            fragmented_free_bytes,
            right_child,
        })
    }

    pub fn write(&self, buf: &mut Vec<u8>) -> crate::Result<()> {
        buf.write_u8(self.page_type as u8)?;
        buf.write_u16::<BigEndian>(self.first_freeblock)?;
        buf.write_u16::<BigEndian>(self.cell_count)?;
        buf.write_u16::<BigEndian>(self.cell_content_start)?;
        buf.write_u8(self.fragmented_free_bytes)?;
        buf.write_u32::<BigEndian>(self.right_child)?;
        Ok(())
    }

    /// Create a default header for a new empty leaf page.
    pub fn new_leaf() -> Self {
        Self {
            page_type: PageType::Leaf,
            first_freeblock: 0,
            cell_count: 0,
            cell_content_start: PAGE_SIZE as u16,
            fragmented_free_bytes: 0,
            right_child: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_trip_header() {
        let hdr = PageHeader::new_leaf();
        let mut buf = Vec::new();
        hdr.write(&mut buf).unwrap();
        let hdr2 = PageHeader::read(&buf).unwrap();
        assert_eq!(hdr2.page_type, PageType::Leaf);
        assert_eq!(hdr2.cell_count, 0);
    }
}
