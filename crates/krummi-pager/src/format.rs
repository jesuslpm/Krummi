//! On-disk format of `krummi.data` (decisions P2–P6 of the Pager PRD).
//!
//! Layout of the two header pages:
//!
//! - page 0: the identification mark (bytes 0..16), then header copy A at byte 64;
//! - page 1: header copy B at byte 0.
//!
//! Format 2 (module 3) adds to each header copy how far the journal has been transferred to `krummi.data`. Format 1
//! copies are still decoded, with their own layout, so that a store of format 1 is refused as an older format and
//! not as damage (J3 of the Journal PRD). Format 3 (the rename to Krummi) changes only the mark and the file names;
//! its header copies have the layout of format 2, so a format 2 header with the new mark is an older format (R5 of
//! the rename-to-krummi PRD).
//!
//! Every other page starts with a 16-byte page header (checksum, page number) followed by its payload. The
//! checksum covers the page number and the payload, so a page written at the wrong position is also detected.

use xxhash_rust::xxh3::xxh3_64;
use zerocopy::little_endian::{U32, U64};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

/// Size of a page in bytes (P2).
pub const PAGE_SIZE: usize = 8192;
/// [`PAGE_SIZE`] as a `u64`, for file offsets.
pub const PAGE_SIZE_U64: u64 = PAGE_SIZE as u64;
/// Format version written by this version of the engine (P5).
pub const FORMAT_VERSION: u32 = 3;
/// Identification mark at the start of `krummi.data` (P4). The `\r\n`, `\x1a` and `\n` bytes detect text-mode
/// transfers that would corrupt the file, as in the PNG signature.
pub const MARK: [u8; 16] = *b"\x89Krummi\r\n\x1a\n\0\0\0\0\0";
/// Number of pages that hold the header copies; data pages start after them.
pub const HEADER_PAGES: u64 = 2;
/// Size of the per-page header: checksum and page number.
pub const PAGE_HEADER_SIZE: usize = 16;
/// Bytes of a page available to its user.
pub const PAGE_PAYLOAD_SIZE: usize = PAGE_SIZE - PAGE_HEADER_SIZE;

const HEADER_A_OFFSET: usize = 64;
const HEADER_B_OFFSET: usize = PAGE_SIZE;
/// Byte offsets of the two header copies in `krummi.data`.
pub const HEADER_COPY_OFFSETS: [u64; 2] = [HEADER_A_OFFSET as u64, HEADER_B_OFFSET as u64];

/// A header copy of format 2.
#[repr(C)]
#[derive(FromBytes, IntoBytes, Immutable, KnownLayout, Clone, Copy)]
struct RawHeader {
    format_version: U32,
    page_size: U32,
    revision: U64,
    applied_txid: U64,
    journal_first: U64,
    checksum: U64,
}

/// A header copy of format 1 (modules 1 and 2).
#[repr(C)]
#[derive(FromBytes, IntoBytes, Immutable, KnownLayout, Clone, Copy)]
struct RawHeaderV1 {
    format_version: U32,
    page_size: U32,
    revision: U64,
    reserved: [u8; 8],
    checksum: U64,
}

/// Size in bytes of an encoded header copy.
pub const RAW_HEADER_SIZE: usize = size_of::<RawHeader>();
const CHECKED_HEADER_BYTES: usize = RAW_HEADER_SIZE - size_of::<U64>();
const CHECKED_HEADER_V1_BYTES: usize = size_of::<RawHeaderV1>() - size_of::<U64>();

/// The decoded content of one header copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub format_version: u32,
    pub page_size: u32,
    /// Grows each time the header is written; the intact copy with the highest revision is the current one.
    pub revision: u64,
    /// Last commit whose changes are all in `krummi.data` and flushed to the disk (J6 of the Journal PRD).
    pub applied_txid: u64,
    /// Number of the first journal file that may hold commits after `applied_txid`.
    pub journal_first: u64,
}

impl Header {
    /// The header of a store created by this version of the engine.
    pub fn new_store() -> Header {
        Header {
            format_version: FORMAT_VERSION,
            page_size: PAGE_SIZE as u32,
            revision: 1,
            applied_txid: 0,
            journal_first: 1,
        }
    }

    /// The encoded bytes of one header copy.
    pub fn encode(&self) -> [u8; RAW_HEADER_SIZE] {
        let mut raw = RawHeader {
            format_version: U32::new(self.format_version),
            page_size: U32::new(self.page_size),
            revision: U64::new(self.revision),
            applied_txid: U64::new(self.applied_txid),
            journal_first: U64::new(self.journal_first),
            checksum: U64::new(0),
        };
        raw.checksum = U64::new(xxh3_64(&raw.as_bytes()[..CHECKED_HEADER_BYTES]));
        let mut out = [0; RAW_HEADER_SIZE];
        out.copy_from_slice(raw.as_bytes());
        out
    }

    /// Decodes a header copy, or `None` when its integrity check fails. A copy whose check passes with the
    /// layout of format 1 decodes as a header of that format, without journal data.
    fn decode(bytes: &[u8]) -> Option<Header> {
        let raw = RawHeader::read_from_bytes(bytes.get(..RAW_HEADER_SIZE)?).ok()?;
        if raw.format_version.get() != 1
            && raw.checksum.get() == xxh3_64(&raw.as_bytes()[..CHECKED_HEADER_BYTES])
        {
            return Some(Header {
                format_version: raw.format_version.get(),
                page_size: raw.page_size.get(),
                revision: raw.revision.get(),
                applied_txid: raw.applied_txid.get(),
                journal_first: raw.journal_first.get(),
            });
        }
        let v1 = RawHeaderV1::read_from_bytes(bytes.get(..size_of::<RawHeaderV1>())?).ok()?;
        if v1.checksum.get() != xxh3_64(&v1.as_bytes()[..CHECKED_HEADER_V1_BYTES]) {
            return None;
        }
        Some(Header {
            format_version: v1.format_version.get(),
            page_size: v1.page_size.get(),
            revision: v1.revision.get(),
            applied_txid: 0,
            journal_first: 1,
        })
    }
}

/// Encodes a header copy with the layout of format 1, as the stores of modules 1 and 2 have it (for tests).
pub fn encode_v1(format_version: u32, revision: u64) -> Vec<u8> {
    let mut raw = RawHeaderV1 {
        format_version: U32::new(format_version),
        page_size: U32::new(PAGE_SIZE as u32),
        revision: U64::new(revision),
        reserved: [0; 8],
        checksum: U64::new(0),
    };
    raw.checksum = U64::new(xxh3_64(&raw.as_bytes()[..CHECKED_HEADER_V1_BYTES]));
    raw.as_bytes().to_vec()
}

/// Writes the mark and both header copies into the first two pages of `pages`.
pub fn write_header_pages(pages: &mut [u8], header: &Header) {
    pages[..MARK.len()].copy_from_slice(&MARK);
    let encoded = header.encode();
    pages[HEADER_A_OFFSET..HEADER_A_OFFSET + RAW_HEADER_SIZE].copy_from_slice(&encoded);
    pages[HEADER_B_OFFSET..HEADER_B_OFFSET + RAW_HEADER_SIZE].copy_from_slice(&encoded);
}

/// What the start of a data file says about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderRead {
    /// The file does not start with the identification mark (D10, P4).
    NotAStore,
    /// The mark is intact, but the header pages are incomplete or no header copy is intact (P6).
    Damaged,
    /// The current header: the intact copy with the highest revision.
    Found(Header),
}

/// Reads the identification mark and the header copies from the start of a data file.
pub fn read_header_pages(start: &[u8]) -> HeaderRead {
    if start.len() < MARK.len() || start[..MARK.len()] != MARK {
        return HeaderRead::NotAStore;
    }
    if start.len() < HEADER_PAGES as usize * PAGE_SIZE {
        return HeaderRead::Damaged;
    }
    let [a, b] = header_copies(start);
    let current = match (a, b) {
        (Some(a), Some(b)) => Some(if b.revision > a.revision { b } else { a }),
        (Some(h), None) | (None, Some(h)) => Some(h),
        (None, None) => None,
    };
    match current {
        // A page size other than ours is not a format this engine ever wrote: treat it as damage.
        Some(h) if h.page_size as usize == PAGE_SIZE => HeaderRead::Found(h),
        _ => HeaderRead::Damaged,
    }
}

/// The two header copies in the header pages at the start of `start`, each `None` when damaged.
///
/// # Panics
///
/// If `start` is shorter than the header pages.
pub fn header_copies(start: &[u8]) -> [Option<Header>; 2] {
    [
        Header::decode(&start[HEADER_A_OFFSET..]),
        Header::decode(&start[HEADER_B_OFFSET..]),
    ]
}

fn page_checksum(page: &[u8]) -> u64 {
    xxh3_64(&page[8..PAGE_SIZE])
}

/// Fills `page` with the page header and `payload`; the rest of the payload area is zeroed.
pub fn seal_page(page_number: u64, payload: &[u8], page: &mut [u8; PAGE_SIZE]) {
    assert!(
        payload.len() <= PAGE_PAYLOAD_SIZE,
        "payload of {} bytes does not fit in a page",
        payload.len()
    );
    page[8..16].copy_from_slice(&page_number.to_le_bytes());
    page[PAGE_HEADER_SIZE..PAGE_HEADER_SIZE + payload.len()].copy_from_slice(payload);
    page[PAGE_HEADER_SIZE + payload.len()..].fill(0);
    let checksum = page_checksum(page);
    page[..8].copy_from_slice(&checksum.to_le_bytes());
}

/// Whether `page` is intact and was written as page `page_number` (G6).
pub fn verify_page(page_number: u64, page: &[u8]) -> bool {
    page.len() == PAGE_SIZE
        && page[8..16] == page_number.to_le_bytes()
        && page[..8] == page_checksum(page).to_le_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_pages(header: &Header) -> Vec<u8> {
        let mut pages = vec![0; HEADER_PAGES as usize * PAGE_SIZE];
        write_header_pages(&mut pages, header);
        pages
    }

    #[test]
    fn header_pages_round_trip() {
        let pages = header_pages(&Header::new_store());
        assert_eq!(
            read_header_pages(&pages),
            HeaderRead::Found(Header::new_store())
        );
        assert_eq!(Header::new_store().format_version, 3);
    }

    #[test]
    fn a_format_1_copy_decodes_as_format_1() {
        let mut pages = header_pages(&Header::new_store());
        let v1 = encode_v1(1, 3);
        for offset in [HEADER_A_OFFSET, HEADER_B_OFFSET] {
            pages[offset..offset + v1.len()].copy_from_slice(&v1);
            pages[offset + v1.len()..offset + RAW_HEADER_SIZE].fill(0);
        }
        match read_header_pages(&pages) {
            HeaderRead::Found(h) => assert_eq!((h.format_version, h.revision), (1, 3)),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_file_without_the_mark_is_not_a_store() {
        let mut pages = header_pages(&Header::new_store());
        pages[3] ^= 0xFF;
        assert_eq!(read_header_pages(&pages), HeaderRead::NotAStore);
        assert_eq!(read_header_pages(&[]), HeaderRead::NotAStore);
        assert_eq!(read_header_pages(&MARK[..10]), HeaderRead::NotAStore);
    }

    #[test]
    fn the_mark_with_incomplete_header_pages_is_damage() {
        let pages = header_pages(&Header::new_store());
        assert_eq!(
            read_header_pages(&pages[..PAGE_SIZE + 10]),
            HeaderRead::Damaged
        );
    }

    #[test]
    fn one_damaged_copy_is_tolerated_and_two_are_damage() {
        let header = Header::new_store();
        for offset in [HEADER_A_OFFSET, HEADER_B_OFFSET] {
            let mut pages = header_pages(&header);
            pages[offset + 2] ^= 0x01;
            assert_eq!(
                read_header_pages(&pages),
                HeaderRead::Found(header),
                "copy at {offset}"
            );
        }
        let mut pages = header_pages(&header);
        pages[HEADER_A_OFFSET + 5] ^= 0x01;
        pages[HEADER_B_OFFSET + 5] ^= 0x01;
        assert_eq!(read_header_pages(&pages), HeaderRead::Damaged);
    }

    #[test]
    fn the_copy_with_the_highest_revision_wins() {
        let mut pages = header_pages(&Header::new_store());
        let newer = Header {
            revision: 7,
            ..Header::new_store()
        };
        pages[HEADER_B_OFFSET..HEADER_B_OFFSET + RAW_HEADER_SIZE].copy_from_slice(&newer.encode());
        assert_eq!(read_header_pages(&pages), HeaderRead::Found(newer));
    }

    #[test]
    fn every_byte_of_a_header_copy_is_covered_by_its_check() {
        let header = Header::new_store();
        for byte in 0..RAW_HEADER_SIZE {
            let mut encoded = header.encode();
            encoded[byte] ^= 0x10;
            assert_eq!(Header::decode(&encoded), None, "byte {byte}");
        }
    }

    #[test]
    fn a_sealed_page_verifies_and_any_flipped_byte_does_not() {
        let mut page = [0; PAGE_SIZE];
        seal_page(5, b"hello", &mut page);
        assert!(verify_page(5, &page));
        assert!(
            !verify_page(6, &page),
            "a page read at the wrong position is damage"
        );
        for byte in [0, 7, 8, 15, 16, 20, PAGE_SIZE - 1] {
            let mut flipped = page;
            flipped[byte] ^= 0x01;
            assert!(!verify_page(5, &flipped), "byte {byte}");
        }
    }

    #[test]
    fn a_never_written_page_does_not_verify() {
        assert!(!verify_page(3, &[0; PAGE_SIZE]));
    }
}
