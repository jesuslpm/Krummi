//! `ulid` and `uuid` values (N4 of the tables PRD): their text forms (E9 of the tables with schema PRD) and their
//! generators (E15).

/// A `ulid`: 16 bytes, 48 bits of Unix milliseconds and then 80 random bits, big-endian (document 1 of the table
/// design). Its byte order is its creation order and the order of its text form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ulid(pub [u8; 16]);

/// A `uuid`, always in the byte order of RFC 9562 (E7), also for a column with the order of SQL Server: the order
/// is a property of the column's key, not of the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Uuid(pub [u8; 16]);
