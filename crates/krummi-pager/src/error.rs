//! Error classes of the engine (D8 of the root PRD).
//!
//! Each variant is one error class; it carries as separate data what the command-line tool needs to build its
//! Spanish message. The `Display` text is English.

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
    /// A tree with that name already exists (A1, A3).
    TreeAlreadyExists,
    /// A value breaks a rule of its column or of its operation (N4 of the tables PRD, E17 of the tables with schema
    /// PRD).
    InvalidValue,
    /// A row with that primary key already exists (N10 of the tables PRD).
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
    /// The prefix of a scan or a range removal of a tree.
    Prefix,
    /// A column name, measured in bytes of UTF-8 (E18).
    ColumnName,
    /// The primary key of a row, or a bound of a range of it, in its stored form (E18, E12).
    PrimaryKey,
    /// A row, in its stored form (E18).
    Row,
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
    /// A primary key column that allows nulls, is a vector or has a default value.
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
    /// A schema whose stored form, with its column names and default values, passes 256 MiB (decided 2026-11-11).
    SchemaTooLarge,
}

/// What differs in an [`Error::DifferentSchema`] (E6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SchemaDifference {
    /// The column's type.
    Type,
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
}

/// An engine error.
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
    /// `max` are inclusive.
    InvalidSize {
        folder: Option<PathBuf>,
        what: SizeSubject,
        received: u64,
        min: u64,
        max: u64,
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
    /// See [`ErrorKind::TreeAlreadyExists`]. `folder` is `None` for a temporary store.
    TreeAlreadyExists {
        folder: Option<PathBuf>,
        name: String,
    },
    /// See [`ErrorKind::InvalidValue`]. `table` and `column` name where the value is, when there is one;
    /// `sequence` names the sequence when it is the subject (E17).
    InvalidValue {
        folder: Option<PathBuf>,
        table: Option<String>,
        column: Option<String>,
        sequence: Option<String>,
        rule: ValueRule,
    },
    /// See [`ErrorKind::DuplicateKey`]. `key` is the primary key's values in their text form.
    DuplicateKey {
        folder: Option<PathBuf>,
        table: String,
        key: String,
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
    /// See [`ErrorKind::InvalidSchema`].
    InvalidSchema {
        folder: Option<PathBuf>,
        table: String,
        column: Option<String>,
        rule: SchemaRule,
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
            Error::DifferentSchema { .. } => ErrorKind::DifferentSchema,
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
            } => Error::InvalidSize {
                folder: folder.clone(),
                what: *what,
                received: *received,
                min: *min,
                max: *max,
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
            Error::TreeAlreadyExists { folder, name } => Error::TreeAlreadyExists {
                folder: folder.clone(),
                name: name.clone(),
            },
            Error::InvalidValue {
                folder,
                table,
                column,
                sequence,
                rule,
            } => Error::InvalidValue {
                folder: folder.clone(),
                table: table.clone(),
                column: column.clone(),
                sequence: sequence.clone(),
                rule: *rule,
            },
            Error::DuplicateKey { folder, table, key } => Error::DuplicateKey {
                folder: folder.clone(),
                table: table.clone(),
                key: key.clone(),
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
            Error::InvalidSchema {
                folder,
                table,
                column,
                rule,
            } => Error::InvalidSchema {
                folder: folder.clone(),
                table: table.clone(),
                column: column.clone(),
                rule: *rule,
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
            } => write!(
                f,
                "invalid {what} size in {}: {received} bytes, but it must have {min} to {max}",
                store_name(folder)
            ),
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
            Error::TreeAlreadyExists { folder, name } => {
                write!(f, "tree {name:?} already exists in {}", store_name(folder))
            }
            Error::InvalidValue {
                folder,
                table,
                column,
                sequence,
                rule,
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
                write!(f, "{}: {}", store_name(folder), value_rule_text(*rule))
            }
            Error::DuplicateKey { folder, table, key } => write!(
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
            Error::InvalidSchema {
                folder,
                table,
                column,
                rule,
            } => {
                write!(f, "invalid schema for table {table:?}")?;
                if let Some(column) = column {
                    write!(f, ", column {column:?}")?;
                }
                write!(f, " of {}: {}", store_name(folder), schema_rule_text(*rule))
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

/// A rule of [`SchemaRule`] as a short phrase.
fn schema_rule_text(rule: SchemaRule) -> &'static str {
    match rule {
        SchemaRule::NoPrimaryKey => "no primary key",
        SchemaRule::InvalidKeyColumn => {
            "a primary key column allows nulls, is a vector or has a default value"
        }
        SchemaRule::InvalidKeyReference => {
            "the primary key names a column that does not exist or repeats one"
        }
        SchemaRule::DuplicateColumnName => "two columns with the same name",
        SchemaRule::TooManyColumns => "more than 1,024 columns",
        SchemaRule::VectorOutOfLimits => "a vector outside its limits",
        SchemaRule::DecimalOutOfLimits => "a decimal outside its limits",
        SchemaRule::CosineOnBit => "the cosine metric on bit elements",
        SchemaRule::InvalidDefault => "a default value that breaks the rules of its column",
        SchemaRule::NotNullWithoutDefault => "a new column without nulls needs a default value",
        SchemaRule::UnsupportedChange => "a change that is not supported",
        SchemaRule::ColumnNotFound => "the column does not exist",
        SchemaRule::DropKeyColumn => "a primary key column cannot be dropped",
        SchemaRule::VersionExhausted => "the schema version would pass 65,535",
        SchemaRule::SchemaTooLarge => "the schema with its names and defaults would pass 256 MiB",
    }
}

/// A [`SchemaDifference`] as a short phrase.
fn difference_text(difference: SchemaDifference) -> &'static str {
    match difference {
        SchemaDifference::Type => "type",
        SchemaDifference::Nullability => "nullability",
        SchemaDifference::Default => "default value",
        SchemaDifference::MissingColumn => "column",
        SchemaDifference::PrimaryKey => "primary key",
        SchemaDifference::Projection => "projection",
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
}
