//! Error classes of the engine (D8 of the root PRD).
//!
//! Each variant is one error class, except [`Error::DifferentIndex`], which is of the class of
//! [`Error::DifferentSchema`] with the data of an index; it carries as separate data what the command-line tool needs
//! to build its Spanish message. The `Display` text is English.

use std::fmt;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

/// The class of an [`Error`], for callers that only need to branch on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The folder cannot be used: it is a file, permission is missing, the file system is read-only, or the path
    /// is not valid.
    Access,
    /// The store is already open, by another process or by this one.
    StoreInUse,
    /// The folder does not contain a Krummi store.
    NotAStore,
    /// Part of the store is damaged.
    Damaged,
    /// The store uses a different format version.
    DifferentFormat,
    /// There is no space left on the disk.
    DiskFull,
    /// The operating system failed to read or write for a reason other than lack of space.
    Io,
    /// The options passed when opening are not valid.
    InvalidOptions,
    /// Growing the store would exceed the maximum size set when opening it.
    MaxSize,
    /// The system did not provide the memory the store needs.
    OutOfMemory,
    /// A request for a write transaction waited for its whole limit without getting its turn (T4).
    Timeout,
    /// The engine was called in an order or with values it does not accept, such as a second write transaction
    /// on the same thread (T5) or a tree handle used in a transaction other than its own (B1 of the trees PRD).
    /// Distinct from [`ErrorKind::InvalidOptions`], which is about opening a store.
    Misuse,
    /// A tree name, a key or a value is outside its allowed sizes (B4–B6 of the trees PRD).
    InvalidSize,
    /// A tree exists with a kind other than the one requested (B3 of the trees PRD).
    DifferentTreeType,
    /// The tree does not exist, or a handle refers to a tree its write transaction deleted (A1, A6).
    TreeNotFound,
    /// A tree with that name already exists (A1, A3), or an index with that name in its table (IX2 of the secondary
    /// indexes PRD).
    TreeAlreadyExists,
    /// A value breaks a rule of its column or of its operation (N4 of the tables PRD, E17 of the tables with schema
    /// PRD).
    InvalidValue,
    /// A row with that primary key already exists (N10 of the tables PRD), or a row repeats the values of a unique
    /// index of its table (IX10 of the secondary indexes PRD).
    DuplicateKey,
    /// A schema given to open a table, or a column of a row, does not match the table (E6).
    DifferentSchema,
    /// A table schema, or a change to it, breaks a rule (E5).
    InvalidSchema,
}

/// The kind of a tree (B2 of the trees PRD). Opening an existing tree with another kind fails with
/// [`ErrorKind::DifferentTreeType`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TreeKind {
    /// A tree of unique keys, each with one value.
    Normal,
    /// A tree of unique keys, each with an ordered set of distinct values (5.4).
    MultiValue,
    /// A tree whose keys are unsigned 64-bit integers in numeric order and whose values all have this number of
    /// bytes, 0 to 2,048, fixed when it is created (5.5): two lengths are two kinds (X3 of the fixed-size trees PRD).
    FixedSize(u16),
    /// A table: a catalog object with a schema, not a tree the application opens as one (7.1, E2).
    Table,
    /// A sequence: a catalog object that hands out increasing integers, not a tree (7.1, E14).
    Sequence,
}

/// What an [`Error::InvalidSize`] is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SizeSubject {
    /// A tree name, measured in bytes of UTF-8.
    TreeName,
    /// A key of a tree.
    Key,
    /// A value of a tree.
    Value,
    /// The prefix of a scan or a range removal of a tree, or the prefix of an interval of an index with its bound,
    /// encoded as in an entry (IX9 of the secondary indexes PRD).
    Prefix,
    /// A column name, measured in bytes of UTF-8 (E18).
    ColumnName,
    /// The primary key of a row, or a bound of a range of it, in its stored form (E18, E12).
    PrimaryKey,
    /// A row, in its stored form (E18).
    Row,
    /// An index name, measured in bytes of UTF-8 (IX1 of the secondary indexes PRD).
    IndexName,
    /// An entry of a secondary index: the values of its key columns followed by the row's primary key, in their
    /// stored form (IX10 of the secondary indexes PRD).
    IndexEntry,
}

/// Why a text is not a valid JSON document (JS8 of the JSON columns PRD). At the same position, a fault is reported
/// in the order of these variants.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum JsonCause {
    /// A byte sequence that is not valid UTF-8; the position is its first byte.
    InvalidUtf8,
    /// A fault of the RFC 8259 grammar; the position is the first character with which the text stops being the start
    /// of a JSON text, or the text's length when it ends before completing one.
    Syntax,
    /// An escaped UTF-16 surrogate that does not form a pair; the position is the `\` of its escape.
    LoneSurrogate,
    /// An object with two properties of this name (decoded); the position is the opening quote of the later one.
    RepeatedProperty(Box<str>),
    /// More than 128 nested objects or arrays; the position is the `{` or `[` that opens level 129.
    TooDeep,
}

/// Where and why a JSON text is not a valid document (JS8 of the JSON columns PRD).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JsonFault {
    pub cause: JsonCause,
    /// The number of bytes of the given text before the fault.
    pub position: u64,
    /// The RFC 6901 pointer of the innermost object or array open at the fault, `""` for the root or none.
    pub pointer: Box<str>,
}

/// The rule an [`Error::InvalidValue`] reports, in the order of E17 of the tables with schema PRD: when a row
/// breaks several, the first one in that order is reported (with the column steps of E17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ValueRule {
    /// A column given more than once.
    RepeatedColumn,
    /// A primary key column missing.
    MissingKeyColumn,
    /// A null, or no value, in a column that does not allow nulls.
    NullNotAllowed,
    /// A value of a type other than its column's.
    WrongType,
    /// A malformed `ulid` or `uuid` text (E9).
    MalformedText,
    /// A text that is not a valid JSON document (JS2 and JS8 of the JSON columns PRD); the error's `json` field says
    /// why and where.
    InvalidJson,
    /// A malformed JSON pointer given to navigate a document (JS6 of the JSON columns PRD).
    MalformedJsonPointer,
    /// A `decimal(p, s)` read from a JSON value with `p` or `s` outside 1 ≤ p ≤ 38, 0 ≤ s ≤ p (JS7 of the JSON
    /// columns PRD).
    InvalidReadScale,
    /// A value outside its type's domain, also after rounding (E8).
    OutOfDomain,
    /// A `decimal` that does not fit its precision.
    DecimalPrecision,
    /// A vector of another dimension than its column's.
    VectorDimension,
    /// A NaN or an infinity in a vector.
    VectorNotFinite,
    /// A zero vector in a column with the cosine metric.
    ZeroVectorCosine,
    /// A malformed range (E12).
    InvalidRange,
    /// A sequence at the maximum of the signed 64-bit integers (E14).
    SequenceExhausted,
    /// A generator millisecond that does not fit in 48 bits (E15).
    MillisecondOutOfRange,
    /// A projection of no columns (PY1 of the projection PRD).
    EmptyProjection,
}

/// The rule an [`Error::InvalidSchema`] reports, in the order of E5: when a schema breaks several, the first one in
/// this order is reported, and within it the first column in schema order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum SchemaRule {
    /// No primary key.
    NoPrimaryKey,
    /// A primary key column that allows nulls, is a vector or JSON (JS1 of the JSON columns PRD) or has a default
    /// value.
    InvalidKeyColumn,
    /// A primary key that names a column that does not exist, or repeats a column.
    InvalidKeyReference,
    /// Two columns with the same name.
    DuplicateColumnName,
    /// More than 1,024 live columns.
    TooManyColumns,
    /// A vector outside its limits (N4).
    VectorOutOfLimits,
    /// A `decimal(p, s)` outside 1 <= p <= 38 and 0 <= s <= p.
    DecimalOutOfLimits,
    /// The cosine metric on `bit` elements.
    CosineOnBit,
    /// A collation declared on a column that is not `string`, `binary` included (IC1, IC9 of the collation PRD).
    CollationOnNonText,
    /// A default value that breaks the rules of its column.
    InvalidDefault,
    /// An added column that does not allow nulls and has no default value.
    NotNullWithoutDefault,
    /// A change N3 does not allow: changing a type, changing the primary key, forbidding nulls.
    UnsupportedChange,
    /// Dropping, renaming or modifying a column that does not exist.
    ColumnNotFound,
    /// Dropping a primary key column (or, from 7.2, an index column).
    DropKeyColumn,
    /// A schema version past 65,535 (P19).
    VersionExhausted,
    /// An index definition without key columns (IX3 of the secondary indexes PRD, whose rules follow in its order;
    /// a column it names that does not exist is [`SchemaRule::ColumnNotFound`]).
    NoIndexKey,
    /// A vector or JSON column in the key of an index (JSON since the JSON columns PRD, JS1).
    VectorIndexKey,
    /// A column given more than once in an index definition: in its key, in its included columns or in both.
    RepeatedIndexColumn,
    /// An index key of more than 32 columns.
    TooManyIndexKeyColumns,
    /// An index key that is the whole primary key or, in an index that is not unique, a prefix of it, with its
    /// columns in the same order and all ascending.
    RedundantIndexKey,
    /// An index key equal to another index's of the table: the same columns in the same order, with the same
    /// directions and null orders.
    SameIndexKey,
    /// A vector index over a column that is not a vector or whose elements are `bit` (VI1, VI2 of the vector indexes
    /// PRD).
    VectorIndexColumn,
    /// A vector index with a metric its column does not admit: L2 or the dot product on a cosine column, the cosine
    /// on a column without it, or Hamming (VI2 of the vector indexes PRD).
    VectorIndexMetric,
    /// A vector index equal to another of the table: the same column and the same metric (VI1 of the vector indexes
    /// PRD).
    SameVectorIndex,
    /// More than 64 indexes in a table, secondary and vector ones together (VI1 of the vector indexes PRD).
    TooManyIndexes,
    /// A vector column named by an estimate, a read of statistics or analyze: vectors have no statistics (ST12 of the
    /// statistics PRD).
    VectorColumn,
    /// A JSON column named by an estimate, a read of statistics or analyze: JSON columns have no statistics (JS10 of
    /// the JSON columns PRD).
    JsonColumn,
    /// A schema whose stored form, with its column names, default values and index definitions, passes 256 MiB
    /// (decided 2026-11-11; the index definitions count since the secondary indexes PRD).
    SchemaTooLarge,
}

/// What differs in an [`Error::DifferentSchema`] (E6) or an [`Error::DifferentIndex`] (IX2 of the secondary indexes
/// PRD).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SchemaDifference {
    /// The column's type.
    Type,
    /// The collation of a `string` column (IC9 of the collation PRD): `binary`, `ci` or `ci_ai`; a given column that
    /// declares none expects `binary`.
    Collation,
    /// Whether the column allows nulls.
    Nullability,
    /// The column's default value.
    Default,
    /// A column named by the schema or by a row that the table does not have.
    MissingColumn,
    /// The primary key columns or their order.
    PrimaryKey,
    /// A row bound to a projection of another table, another store or another schema of its table (PY3 of the
    /// projection PRD).
    Projection,
    /// Whether an index is unique (IX2 of the secondary indexes PRD).
    IndexUniqueness,
    /// A key column of an index at a position (its column, its direction and its null order), or the number of its
    /// key columns.
    IndexKey,
    /// An included column of an index at a position, or the number of its included columns.
    IndexIncluded,
    /// An index that the table does not have, named by a scan of an index (IX9 of the secondary indexes PRD), or a
    /// vector index named by a search, its information or a rebuild (vector indexes PRD).
    MissingIndex,
    /// The kind of an index: secondary or vector (VI1 of the vector indexes PRD).
    IndexKind,
    /// The column of a vector index (VI1 of the vector indexes PRD).
    VectorIndexColumn,
    /// The metric of a vector index (VI1 of the vector indexes PRD).
    VectorIndexMetric,
}

/// An engine error. The texts of the data that only the errors of secondary indexes carry are boxed (`Box<str>`), as
/// those of [`Error::DifferentSchema`], so that a result stays small.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// See [`ErrorKind::Access`].
    Access { folder: PathBuf, source: io::Error },
    /// See [`ErrorKind::StoreInUse`].
    StoreInUse { folder: PathBuf },
    /// See [`ErrorKind::NotAStore`].
    NotAStore { folder: PathBuf },
    /// See [`ErrorKind::Damaged`]. `page` is `None` when what is damaged is what the store needs to open.
    Damaged {
        folder: Option<PathBuf>,
        page: Option<u64>,
    },
    /// See [`ErrorKind::DifferentFormat`].
    DifferentFormat {
        folder: PathBuf,
        found: u32,
        expected: u32,
    },
    /// See [`ErrorKind::DiskFull`].
    DiskFull { folder: PathBuf },
    /// See [`ErrorKind::Io`].
    Io { folder: PathBuf, source: io::Error },
    /// See [`ErrorKind::InvalidOptions`].
    InvalidOptions { reason: &'static str },
    /// See [`ErrorKind::MaxSize`]. Sizes are in bytes.
    MaxSize { max_size: u64, needed: u64 },
    /// See [`ErrorKind::OutOfMemory`]. `requested` is in bytes.
    OutOfMemory { requested: u64 },
    /// See [`ErrorKind::Timeout`]. `folder` is `None` for a temporary store; `limit` is the limit requested.
    Timeout {
        folder: Option<PathBuf>,
        limit: Duration,
    },
    /// See [`ErrorKind::Misuse`]. `folder` is `None` for a temporary store.
    Misuse {
        folder: Option<PathBuf>,
        reason: &'static str,
    },
    /// See [`ErrorKind::InvalidSize`]. `folder` is `None` for a temporary store; sizes are in bytes and `min` and
    /// `max` are inclusive. `table` and `index` name the index whose entry is too large ([`SizeSubject::IndexEntry`],
    /// IX10 of the secondary indexes PRD) or whose scan has a prefix that does not fit ([`SizeSubject::Prefix`], IX9);
    /// both are `None` for every other error.
    InvalidSize {
        folder: Option<PathBuf>,
        what: SizeSubject,
        received: u64,
        min: u64,
        max: u64,
        table: Option<Box<str>>,
        index: Option<Box<str>>,
    },
    /// See [`ErrorKind::DifferentTreeType`]. `folder` is `None` for a temporary store.
    DifferentTreeType {
        folder: Option<PathBuf>,
        name: String,
        found: TreeKind,
        requested: TreeKind,
    },
    /// See [`ErrorKind::TreeNotFound`]. `folder` is `None` for a temporary store; `name` is the name requested
    /// or, for a handle of a deleted tree, the last name the tree had (A6).
    TreeNotFound {
        folder: Option<PathBuf>,
        name: String,
    },
    /// See [`ErrorKind::TreeAlreadyExists`]. `folder` is `None` for a temporary store. For an index (IX2 of the
    /// secondary indexes PRD), `name` is its table's name and `index` the index's; otherwise `index` is `None`.
    TreeAlreadyExists {
        folder: Option<PathBuf>,
        name: String,
        index: Option<Box<str>>,
    },
    /// See [`ErrorKind::InvalidValue`]. `table` and `column` name where the value is, when there is one;
    /// `sequence` names the sequence when it is the subject (E17). `json` says why and where a text is not a valid
    /// JSON document, with the rule [`ValueRule::InvalidJson`] (JS8 of the JSON columns PRD), and is `None` otherwise.
    InvalidValue {
        folder: Option<PathBuf>,
        table: Option<String>,
        column: Option<String>,
        sequence: Option<String>,
        rule: ValueRule,
        json: Option<Box<JsonFault>>,
    },
    /// See [`ErrorKind::DuplicateKey`]. `key` is the primary key's values in their text form. When a unique index
    /// repeats (IX10 of the secondary indexes PRD), `index` names it, `index_key` gives the values of its key columns
    /// that repeat, in their text form, and `key` is the primary key of the row being stored (when creating the index,
    /// of the second of the first two rows that repeat them, in the index's order); both are `None` for a repeated
    /// primary key.
    DuplicateKey {
        folder: Option<PathBuf>,
        table: String,
        key: String,
        index: Option<Box<str>>,
        index_key: Option<Box<str>>,
    },
    /// See [`ErrorKind::DifferentSchema`]. `expected` and `found` describe in text what differs: what the schema or
    /// the row gives and what the table has.
    DifferentSchema {
        folder: Option<PathBuf>,
        table: String,
        column: Option<String>,
        difference: SchemaDifference,
        expected: Box<str>,
        found: Box<str>,
    },
    /// Of the class [`ErrorKind::DifferentSchema`], for an index (IX2 of the secondary indexes PRD): the definition
    /// given to create the index `index` of `table` differs from the table's index of that name in `difference`;
    /// `expected` and `found` describe in text what the definition gives and what the index has. A scan of an index
    /// that `table` does not have (IX9) gives [`SchemaDifference::MissingIndex`], expecting `an index` and finding
    /// `no index`. A variant of its own,
    /// so that [`Error::DifferentSchema`] keeps its data and the size of a result.
    DifferentIndex {
        folder: Option<PathBuf>,
        table: String,
        index: String,
        difference: SchemaDifference,
        expected: Box<str>,
        found: Box<str>,
    },
    /// See [`ErrorKind::InvalidSchema`]. `index` names the index whose definition breaks the rule (IX3 of the
    /// secondary indexes PRD), or the index with a column that cannot be dropped (IX5).
    InvalidSchema {
        folder: Option<PathBuf>,
        table: String,
        column: Option<String>,
        rule: SchemaRule,
        index: Option<Box<str>>,
    },
}

impl Error {
    /// The class of this error.
    pub fn kind(&self) -> ErrorKind {
        match self {
            Error::Access { .. } => ErrorKind::Access,
            Error::StoreInUse { .. } => ErrorKind::StoreInUse,
            Error::NotAStore { .. } => ErrorKind::NotAStore,
            Error::Damaged { .. } => ErrorKind::Damaged,
            Error::DifferentFormat { .. } => ErrorKind::DifferentFormat,
            Error::DiskFull { .. } => ErrorKind::DiskFull,
            Error::Io { .. } => ErrorKind::Io,
            Error::InvalidOptions { .. } => ErrorKind::InvalidOptions,
            Error::MaxSize { .. } => ErrorKind::MaxSize,
            Error::OutOfMemory { .. } => ErrorKind::OutOfMemory,
            Error::Timeout { .. } => ErrorKind::Timeout,
            Error::Misuse { .. } => ErrorKind::Misuse,
            Error::InvalidSize { .. } => ErrorKind::InvalidSize,
            Error::DifferentTreeType { .. } => ErrorKind::DifferentTreeType,
            Error::TreeNotFound { .. } => ErrorKind::TreeNotFound,
            Error::TreeAlreadyExists { .. } => ErrorKind::TreeAlreadyExists,
            Error::InvalidValue { .. } => ErrorKind::InvalidValue,
            Error::DuplicateKey { .. } => ErrorKind::DuplicateKey,
            Error::DifferentSchema { .. } | Error::DifferentIndex { .. } => {
                ErrorKind::DifferentSchema
            }
            Error::InvalidSchema { .. } => ErrorKind::InvalidSchema,
        }
    }

    /// A copy of this error, of the same class and with the same data; an operating-system cause is copied as
    /// its kind and message. A write transaction keeps its first failure to report it again on commit (T11).
    pub fn duplicate(&self) -> Error {
        let source = |e: &io::Error| io::Error::new(e.kind(), e.to_string());
        match self {
            Error::Access { folder, source: e } => Error::Access {
                folder: folder.clone(),
                source: source(e),
            },
            Error::StoreInUse { folder } => Error::StoreInUse {
                folder: folder.clone(),
            },
            Error::NotAStore { folder } => Error::NotAStore {
                folder: folder.clone(),
            },
            Error::Damaged { folder, page } => Error::Damaged {
                folder: folder.clone(),
                page: *page,
            },
            Error::DifferentFormat {
                folder,
                found,
                expected,
            } => Error::DifferentFormat {
                folder: folder.clone(),
                found: *found,
                expected: *expected,
            },
            Error::DiskFull { folder } => Error::DiskFull {
                folder: folder.clone(),
            },
            Error::Io { folder, source: e } => Error::Io {
                folder: folder.clone(),
                source: source(e),
            },
            Error::InvalidOptions { reason } => Error::InvalidOptions { reason },
            Error::MaxSize { max_size, needed } => Error::MaxSize {
                max_size: *max_size,
                needed: *needed,
            },
            Error::OutOfMemory { requested } => Error::OutOfMemory {
                requested: *requested,
            },
            Error::Timeout { folder, limit } => Error::Timeout {
                folder: folder.clone(),
                limit: *limit,
            },
            Error::Misuse { folder, reason } => Error::Misuse {
                folder: folder.clone(),
                reason,
            },
            Error::InvalidSize {
                folder,
                what,
                received,
                min,
                max,
                table,
                index,
            } => Error::InvalidSize {
                folder: folder.clone(),
                what: *what,
                received: *received,
                min: *min,
                max: *max,
                table: table.clone(),
                index: index.clone(),
            },
            Error::DifferentTreeType {
                folder,
                name,
                found,
                requested,
            } => Error::DifferentTreeType {
                folder: folder.clone(),
                name: name.clone(),
                found: *found,
                requested: *requested,
            },
            Error::TreeNotFound { folder, name } => Error::TreeNotFound {
                folder: folder.clone(),
                name: name.clone(),
            },
            Error::TreeAlreadyExists {
                folder,
                name,
                index,
            } => Error::TreeAlreadyExists {
                folder: folder.clone(),
                name: name.clone(),
                index: index.clone(),
            },
            Error::InvalidValue {
                folder,
                table,
                column,
                sequence,
                rule,
                json,
            } => Error::InvalidValue {
                folder: folder.clone(),
                table: table.clone(),
                column: column.clone(),
                sequence: sequence.clone(),
                rule: *rule,
                json: json.clone(),
            },
            Error::DuplicateKey {
                folder,
                table,
                key,
                index,
                index_key,
            } => Error::DuplicateKey {
                folder: folder.clone(),
                table: table.clone(),
                key: key.clone(),
                index: index.clone(),
                index_key: index_key.clone(),
            },
            Error::DifferentSchema {
                folder,
                table,
                column,
                difference,
                expected,
                found,
            } => Error::DifferentSchema {
                folder: folder.clone(),
                table: table.clone(),
                column: column.clone(),
                difference: *difference,
                expected: expected.clone(),
                found: found.clone(),
            },
            Error::DifferentIndex {
                folder,
                table,
                index,
                difference,
                expected,
                found,
            } => Error::DifferentIndex {
                folder: folder.clone(),
                table: table.clone(),
                index: index.clone(),
                difference: *difference,
                expected: expected.clone(),
                found: found.clone(),
            },
            Error::InvalidSchema {
                folder,
                table,
                column,
                rule,
                index,
            } => Error::InvalidSchema {
                folder: folder.clone(),
                table: table.clone(),
                column: column.clone(),
                rule: *rule,
                index: index.clone(),
            },
        }
    }

    /// Classifies a failed write, size change or flush: lack of space is [`Error::DiskFull`], memory exhaustion
    /// is [`Error::OutOfMemory`], anything else is [`Error::Io`].
    pub(crate) fn from_write(folder: &std::path::Path, source: io::Error, requested: u64) -> Error {
        match source.kind() {
            io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded => Error::DiskFull {
                folder: folder.to_path_buf(),
            },
            io::ErrorKind::OutOfMemory => Error::OutOfMemory { requested },
            _ => Error::Io {
                folder: folder.to_path_buf(),
                source,
            },
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Access { folder, source } => {
                write!(f, "cannot use folder {}: {source}", folder.display())
            }
            Error::StoreInUse { folder } => write!(f, "store {} is already open", folder.display()),
            Error::NotAStore { folder } => write!(
                f,
                "folder {} does not contain a Krummi store",
                folder.display()
            ),
            Error::Damaged { folder, page } => {
                let store = store_name(folder);
                match page {
                    Some(page) => write!(f, "page {page} of {store} is damaged"),
                    None => write!(f, "{store} is damaged and cannot be opened"),
                }
            }
            Error::DifferentFormat {
                folder,
                found,
                expected,
            } => write!(
                f,
                "store {} has format {found}, but this version of Krummi uses format {expected}",
                folder.display()
            ),
            Error::DiskFull { folder } => {
                write!(f, "no space left on disk for store {}", folder.display())
            }
            Error::Io { folder, source } => write!(
                f,
                "input/output error in store {}: {source}",
                folder.display()
            ),
            Error::InvalidOptions { reason } => write!(f, "invalid options: {reason}"),
            Error::MaxSize { max_size, needed } => {
                write!(
                    f,
                    "the store needs {needed} bytes but its maximum size is {max_size} bytes"
                )
            }
            Error::OutOfMemory { requested } => {
                write!(f, "out of memory while reserving {requested} bytes")
            }
            Error::Timeout { folder, limit } => write!(
                f,
                "no write transaction was available on {} within {limit:?}",
                store_name(folder)
            ),
            Error::Misuse { folder, reason } => {
                write!(f, "misuse of {}: {reason}", store_name(folder))
            }
            Error::InvalidSize {
                folder,
                what,
                received,
                min,
                max,
                table,
                index,
            } => {
                write!(f, "invalid {what} size in ")?;
                if let (Some(table), Some(index)) = (table, index) {
                    write!(f, "index {index:?} of table {table:?} of ")?;
                }
                write!(
                    f,
                    "{}: {received} bytes, but it must have {min} to {max}",
                    store_name(folder)
                )
            }
            Error::DifferentTreeType {
                folder,
                name,
                found,
                requested,
            } if is_tree(found) && is_tree(requested) => write!(
                f,
                "tree {name:?} of {} is a {found} tree, not a {requested} tree",
                store_name(folder)
            ),
            Error::DifferentTreeType {
                folder,
                name,
                found,
                requested,
            } => write!(
                f,
                "{name:?} of {} is a {}, not a {}",
                store_name(folder),
                kind_noun(found),
                kind_noun(requested)
            ),
            Error::TreeNotFound { folder, name } => {
                write!(f, "tree {name:?} not found in {}", store_name(folder))
            }
            Error::TreeAlreadyExists {
                folder,
                name,
                index: None,
            } => {
                write!(f, "tree {name:?} already exists in {}", store_name(folder))
            }
            Error::TreeAlreadyExists {
                folder,
                name,
                index: Some(index),
            } => write!(
                f,
                "index {index:?} already exists in table {name:?} of {}",
                store_name(folder)
            ),
            Error::InvalidValue {
                folder,
                table,
                column,
                sequence,
                rule,
                json,
            } => {
                f.write_str("invalid value in ")?;
                match (table, column, sequence) {
                    (Some(table), Some(column), _) => {
                        write!(f, "column {column:?} of table {table:?} of ")?
                    }
                    (None, Some(column), _) => write!(f, "column {column:?} of ")?,
                    (Some(table), None, _) => write!(f, "table {table:?} of ")?,
                    (None, None, Some(sequence)) => write!(f, "sequence {sequence:?} of ")?,
                    (None, None, None) => {}
                }
                write!(f, "{}: {}", store_name(folder), value_rule_text(*rule))?;
                if let Some(fault) = json {
                    write!(f, ": {}", JsonFaultText(fault))?;
                }
                Ok(())
            }
            Error::DuplicateKey {
                folder,
                table,
                key,
                index: Some(index),
                index_key,
            } => write!(
                f,
                "duplicate key {} of index {index:?} in table {table:?} of {}, by the row of primary key {key}",
                index_key.as_deref().unwrap_or("()"),
                store_name(folder)
            ),
            Error::DuplicateKey {
                folder, table, key, ..
            } => write!(
                f,
                "duplicate primary key {key} in table {table:?} of {}",
                store_name(folder)
            ),
            Error::DifferentSchema {
                folder,
                table,
                column,
                difference,
                expected,
                found,
            } => {
                write!(f, "different schema for table {table:?}")?;
                if let Some(column) = column {
                    write!(f, ", column {column:?}")?;
                }
                write!(
                    f,
                    " of {}: {} expected {expected}, found {found}",
                    store_name(folder),
                    difference_text(*difference)
                )
            }
            Error::DifferentIndex {
                folder,
                table,
                index,
                difference,
                expected,
                found,
            } => write!(
                f,
                "different index {index:?} of table {table:?} of {}: {} expected {expected}, found {found}",
                store_name(folder),
                difference_text(*difference)
            ),
            Error::InvalidSchema {
                folder,
                table,
                column,
                rule,
                index,
            } => {
                write!(f, "invalid schema for table {table:?}")?;
                if let Some(index) = index {
                    write!(f, ", index {index:?}")?;
                }
                if let Some(column) = column {
                    write!(f, ", column {column:?}")?;
                }
                let text = match (rule, index) {
                    // The column of an index that is not a primary key column (IX5).
                    (SchemaRule::DropKeyColumn, Some(_)) => "an index column cannot be dropped",
                    _ => schema_rule_text(*rule),
                };
                write!(f, " of {}: {text}", store_name(folder))
            }
        }
    }
}

impl fmt::Display for TreeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TreeKind::Normal => f.write_str("normal"),
            TreeKind::MultiValue => f.write_str("multi-value"),
            TreeKind::FixedSize(len) => write!(f, "fixed-size of {len} bytes"),
            TreeKind::Table => f.write_str("table"),
            TreeKind::Sequence => f.write_str("sequence"),
        }
    }
}

impl fmt::Display for SizeSubject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SizeSubject::TreeName => "tree name",
            SizeSubject::Key => "key",
            SizeSubject::Value => "value",
            SizeSubject::Prefix => "prefix",
            SizeSubject::ColumnName => "column name",
            SizeSubject::PrimaryKey => "primary key",
            SizeSubject::Row => "row",
            SizeSubject::IndexName => "index name",
            SizeSubject::IndexEntry => "index entry",
        })
    }
}

/// Whether `kind` is a kind of tree, not another class of catalog object (N1).
fn is_tree(kind: &TreeKind) -> bool {
    !matches!(kind, TreeKind::Table | TreeKind::Sequence)
}

/// A kind as a noun: "normal tree", "fixed-size tree of 8 bytes", "table".
fn kind_noun(kind: &TreeKind) -> String {
    match kind {
        TreeKind::FixedSize(len) => format!("fixed-size tree of {len} bytes"),
        tree if is_tree(tree) => format!("{tree} tree"),
        other => other.to_string(),
    }
}

/// A rule of [`ValueRule`] as a short phrase.
fn value_rule_text(rule: ValueRule) -> &'static str {
    match rule {
        ValueRule::RepeatedColumn => "a column given more than once",
        ValueRule::MissingKeyColumn => "a primary key column is missing",
        ValueRule::NullNotAllowed => "null is not allowed",
        ValueRule::WrongType => "a value of another type than its column",
        ValueRule::MalformedText => "malformed text",
        ValueRule::InvalidJson => "not a valid JSON document",
        ValueRule::MalformedJsonPointer => "a malformed JSON pointer",
        ValueRule::InvalidReadScale => "a decimal precision or scale outside its limits",
        ValueRule::OutOfDomain => "outside its domain",
        ValueRule::DecimalPrecision => "the decimal does not fit its precision",
        ValueRule::VectorDimension => "a vector of another dimension",
        ValueRule::VectorNotFinite => "a NaN or an infinity in a vector",
        ValueRule::ZeroVectorCosine => "a zero vector with the cosine metric",
        ValueRule::InvalidRange => "a malformed range",
        ValueRule::SequenceExhausted => "the sequence is at its maximum",
        ValueRule::MillisecondOutOfRange => "the millisecond does not fit in 48 bits",
        ValueRule::EmptyProjection => "a projection without columns",
    }
}

/// A [`JsonFault`] as a phrase: its cause, its position and its pointer.
struct JsonFaultText<'a>(&'a JsonFault);

impl fmt::Display for JsonFaultText<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fault = self.0;
        match &fault.cause {
            JsonCause::InvalidUtf8 => f.write_str("invalid UTF-8")?,
            JsonCause::Syntax => f.write_str("syntax error")?,
            JsonCause::LoneSurrogate => f.write_str("a lone surrogate")?,
            JsonCause::RepeatedProperty(name) => write!(f, "repeated property {name:?}")?,
            JsonCause::TooDeep => f.write_str("more than 128 nested levels")?,
        }
        write!(f, " at byte {} in {:?}", fault.position, fault.pointer)
    }
}

/// A rule of [`SchemaRule`] as a short phrase.
fn schema_rule_text(rule: SchemaRule) -> &'static str {
    match rule {
        SchemaRule::NoPrimaryKey => "no primary key",
        SchemaRule::InvalidKeyColumn => {
            "a primary key column allows nulls, is a vector or JSON, or has a default value"
        }
        SchemaRule::InvalidKeyReference => {
            "the primary key names a column that does not exist or repeats one"
        }
        SchemaRule::DuplicateColumnName => "two columns with the same name",
        SchemaRule::TooManyColumns => "more than 1,024 columns",
        SchemaRule::VectorOutOfLimits => "a vector outside its limits",
        SchemaRule::DecimalOutOfLimits => "a decimal outside its limits",
        SchemaRule::CosineOnBit => "the cosine metric on bit elements",
        SchemaRule::CollationOnNonText => "a collation on a column that is not text",
        SchemaRule::InvalidDefault => "a default value that breaks the rules of its column",
        SchemaRule::NotNullWithoutDefault => "a new column without nulls needs a default value",
        SchemaRule::UnsupportedChange => "a change that is not supported",
        SchemaRule::ColumnNotFound => "the column does not exist",
        SchemaRule::DropKeyColumn => "a primary key column cannot be dropped",
        SchemaRule::VersionExhausted => "the schema version would pass 65,535",
        SchemaRule::NoIndexKey => "an index without key columns",
        SchemaRule::VectorIndexKey => "a vector or JSON column in the key of an index",
        SchemaRule::RepeatedIndexColumn => "a column given more than once in an index",
        SchemaRule::TooManyIndexKeyColumns => "more than 32 key columns in an index",
        SchemaRule::RedundantIndexKey => {
            "an index key that is the primary key or, not unique, a prefix of it"
        }
        SchemaRule::SameIndexKey => "an index key equal to another index's",
        SchemaRule::VectorIndexColumn => {
            "a vector index over a column that is not a vector of f32, f16 or i8"
        }
        SchemaRule::VectorIndexMetric => "a vector index with a metric its column does not admit",
        SchemaRule::SameVectorIndex => "a vector index equal to another's column and metric",
        SchemaRule::TooManyIndexes => "more than 64 indexes",
        SchemaRule::VectorColumn => "a vector column, which has no statistics",
        SchemaRule::JsonColumn => "a JSON column, which has no statistics",
        SchemaRule::SchemaTooLarge => "the schema with its names and defaults would pass 256 MiB",
    }
}

/// A [`SchemaDifference`] as a short phrase.
fn difference_text(difference: SchemaDifference) -> &'static str {
    match difference {
        SchemaDifference::Type => "type",
        SchemaDifference::Collation => "collation",
        SchemaDifference::Nullability => "nullability",
        SchemaDifference::Default => "default value",
        SchemaDifference::MissingColumn => "column",
        SchemaDifference::PrimaryKey => "primary key",
        SchemaDifference::Projection => "projection",
        SchemaDifference::IndexUniqueness => "uniqueness",
        SchemaDifference::IndexKey => "key",
        SchemaDifference::IndexIncluded => "included columns",
        SchemaDifference::MissingIndex => "index",
        SchemaDifference::IndexKind => "kind of index",
        SchemaDifference::VectorIndexColumn => "vector column",
        SchemaDifference::VectorIndexMetric => "metric",
    }
}

/// "store <folder>", or "temporary store" when there is no folder.
fn store_name(folder: &Option<PathBuf>) -> String {
    folder.as_ref().map_or_else(
        || "temporary store".to_owned(),
        |p| format!("store {}", p.display()),
    )
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Access { source, .. } | Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Result type of the engine.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    fn tree_type(found: TreeKind, requested: TreeKind) -> String {
        Error::DifferentTreeType {
            folder: None,
            name: "t".to_owned(),
            found,
            requested,
        }
        .to_string()
    }

    /// Between two trees the text names them as trees, as before the tables; a table or a sequence is a noun.
    #[test]
    fn different_tree_type_reads_well_for_trees_and_for_other_classes() {
        assert_eq!(
            tree_type(TreeKind::MultiValue, TreeKind::Normal),
            "tree \"t\" of temporary store is a multi-value tree, not a normal tree"
        );
        assert_eq!(
            tree_type(TreeKind::Table, TreeKind::FixedSize(8)),
            "\"t\" of temporary store is a table, not a fixed-size tree of 8 bytes"
        );
        assert_eq!(
            tree_type(TreeKind::Normal, TreeKind::Table),
            "\"t\" of temporary store is a normal tree, not a table"
        );
    }

    fn invalid_value(table: Option<&str>, column: Option<&str>, sequence: Option<&str>) -> String {
        Error::InvalidValue {
            folder: None,
            table: table.map(str::to_owned),
            column: column.map(str::to_owned),
            sequence: sequence.map(str::to_owned),
            rule: ValueRule::OutOfDomain,
            json: None,
        }
        .to_string()
    }

    /// No dangling comma when the table is absent, and the rule as a phrase, not its debug form.
    #[test]
    fn invalid_value_names_where_the_value_is() {
        assert_eq!(
            invalid_value(Some("t"), Some("c"), None),
            "invalid value in column \"c\" of table \"t\" of temporary store: outside its domain"
        );
        assert_eq!(
            invalid_value(None, Some("c"), None),
            "invalid value in column \"c\" of temporary store: outside its domain"
        );
        assert_eq!(
            invalid_value(Some("t"), None, None),
            "invalid value in table \"t\" of temporary store: outside its domain"
        );
        assert_eq!(
            invalid_value(None, None, Some("s")),
            "invalid value in sequence \"s\" of temporary store: outside its domain"
        );
        assert_eq!(
            invalid_value(None, None, None),
            "invalid value in temporary store: outside its domain"
        );
    }

    #[test]
    fn schema_errors_read_as_phrases() {
        let invalid = Error::InvalidSchema {
            folder: None,
            table: "t".to_owned(),
            column: Some("c".to_owned()),
            rule: SchemaRule::NotNullWithoutDefault,
            index: None,
        };
        assert_eq!(
            invalid.to_string(),
            "invalid schema for table \"t\", column \"c\" of temporary store: a new column without nulls needs a \
             default value"
        );
        let different = Error::DifferentSchema {
            folder: None,
            table: "t".to_owned(),
            column: Some("c".to_owned()),
            difference: SchemaDifference::Type,
            expected: "i64".into(),
            found: "i32".into(),
        };
        assert_eq!(
            different.to_string(),
            "different schema for table \"t\", column \"c\" of temporary store: type expected i64, found i32"
        );
    }

    /// The errors that gained the data of an index (IX10 of the secondary indexes PRD) read as before without it.
    #[test]
    fn errors_without_an_index_read_as_before() {
        let size = Error::InvalidSize {
            folder: None,
            what: SizeSubject::Row,
            received: 9,
            min: 0,
            max: 8,
            table: None,
            index: None,
        };
        assert_eq!(
            size.to_string(),
            "invalid row size in temporary store: 9 bytes, but it must have 0 to 8"
        );
        let exists = Error::TreeAlreadyExists {
            folder: None,
            name: "t".to_owned(),
            index: None,
        };
        assert_eq!(
            exists.to_string(),
            "tree \"t\" already exists in temporary store"
        );
        let duplicate = Error::DuplicateKey {
            folder: None,
            table: "t".to_owned(),
            key: "(1)".to_owned(),
            index: None,
            index_key: None,
        };
        assert_eq!(
            duplicate.to_string(),
            "duplicate primary key (1) in table \"t\" of temporary store"
        );
        let drop = Error::InvalidSchema {
            folder: None,
            table: "t".to_owned(),
            column: Some("id".to_owned()),
            rule: SchemaRule::DropKeyColumn,
            index: None,
        };
        assert_eq!(
            drop.to_string(),
            "invalid schema for table \"t\", column \"id\" of temporary store: a primary key column cannot be dropped"
        );
    }

    /// The errors of an index name it after its table.
    #[test]
    fn index_errors_name_the_index() {
        let size = Error::InvalidSize {
            folder: None,
            what: SizeSubject::IndexEntry,
            received: 2049,
            min: 1,
            max: 2048,
            table: Some("t".into()),
            index: Some("ix".into()),
        };
        assert_eq!(
            size.to_string(),
            "invalid index entry size in index \"ix\" of table \"t\" of temporary store: 2049 bytes, but it must \
             have 1 to 2048"
        );
        let exists = Error::TreeAlreadyExists {
            folder: None,
            name: "t".to_owned(),
            index: Some("ix".into()),
        };
        assert_eq!(
            exists.to_string(),
            "index \"ix\" already exists in table \"t\" of temporary store"
        );
        let duplicate = Error::DuplicateKey {
            folder: None,
            table: "t".to_owned(),
            key: "(2)".to_owned(),
            index: Some("ix".into()),
            index_key: Some("(7)".into()),
        };
        assert_eq!(
            duplicate.to_string(),
            "duplicate key (7) of index \"ix\" in table \"t\" of temporary store, by the row of primary key (2)"
        );
        let invalid = Error::InvalidSchema {
            folder: None,
            table: "t".to_owned(),
            column: Some("n".to_owned()),
            rule: SchemaRule::DropKeyColumn,
            index: Some("ix".into()),
        };
        assert_eq!(
            invalid.to_string(),
            "invalid schema for table \"t\", index \"ix\", column \"n\" of temporary store: an index column cannot be \
             dropped"
        );
        let different = Error::DifferentIndex {
            folder: None,
            table: "t".to_owned(),
            index: "ix".to_owned(),
            difference: SchemaDifference::IndexUniqueness,
            expected: "unique".into(),
            found: "not unique".into(),
        };
        assert_eq!(different.kind(), ErrorKind::DifferentSchema);
        assert_eq!(
            different.to_string(),
            "different index \"ix\" of table \"t\" of temporary store: uniqueness expected unique, found not unique"
        );
        let missing = Error::DifferentIndex {
            folder: None,
            table: "t".to_owned(),
            index: "ix".to_owned(),
            difference: SchemaDifference::MissingIndex,
            expected: "an index".into(),
            found: "no index".into(),
        };
        assert_eq!(missing.kind(), ErrorKind::DifferentSchema);
        assert_eq!(
            missing.to_string(),
            "different index \"ix\" of table \"t\" of temporary store: index expected an index, found no index"
        );
        let prefix = Error::InvalidSize {
            folder: None,
            what: SizeSubject::Prefix,
            received: 2049,
            min: 1,
            max: 2048,
            table: Some("t".into()),
            index: Some("ix".into()),
        };
        assert_eq!(
            prefix.to_string(),
            "invalid prefix size in index \"ix\" of table \"t\" of temporary store: 2049 bytes, but it must have 1 to \
             2048"
        );
    }

    /// A result stays as small as before the secondary indexes: the errors of an index box their texts, and
    /// [`Error::DifferentSchema`], already at the limit, has a variant for the indexes.
    #[test]
    fn an_error_stays_small() {
        assert!(std::mem::size_of::<Error>() <= 120);
    }
}
