//! Table schemas (E4, E5 and E6 of the tables with schema PRD; document 1 of the table design, *Schema evolution*;
//! document 11, §2.2 and P19): the schema an application gives, its validation, the four changes of E4, the
//! compatibility of a schema given to an opening, and the encodings the catalog and the schema history store.
//!
//! A table's catalog entry (D3 of the execution design) holds, after the catalog's fixed part, the root of its
//! schema history tree and its current schema ([`TableSchema::encode`]). The history tree has one entry per version,
//! keyed by the big-endian `u16` version, whose value lists that version's columns for decoding its rows
//! ([`encode_version`]); it also holds every identity the table has used, keyed by its 8 big-endian bytes with an
//! empty value, so that a new identity equal to one of its history is drawn again in one lookup (P12).

use std::collections::HashSet;
use std::fmt;

use krummi_pager::{SchemaDifference, SchemaRule};
use krummi_tree::catalog::MAX_EXTRA;

use super::row::{RowColumn, RowLayout};
#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
use super::value::{
    ColumnType, UuidOrder, Value, VectorElement, VectorType, check_type, normalize,
};

/// The most live columns of a table (E4).
pub const MAX_COLUMNS: usize = 1024;
/// The longest column name, in bytes of UTF-8 (E4, E18); the shortest has 1.
pub const MAX_COLUMN_NAME: usize = 1024;

/// A column as the application declares it, to create a table or to add a column (E4). It allows nulls unless it
/// says otherwise (N3); a null default is no default.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub name: String,
    pub ty: ColumnType,
    pub nullable: bool,
    pub default: Option<Value<'static>>,
}

impl Column {
    /// A column that allows nulls, without a default value.
    pub fn new(name: impl Into<String>, ty: ColumnType) -> Column {
        Column {
            name: name.into(),
            ty,
            nullable: true,
            default: None,
        }
    }

    /// The column without nulls.
    pub fn not_null(self) -> Column {
        Column {
            nullable: false,
            ..self
        }
    }

    /// The column with the default value `value`, a constant of its type (E4).
    pub fn with_default(self, value: Value<'static>) -> Column {
        Column {
            default: Some(value),
            ..self
        }
    }
}

/// A schema as the application gives it to create or open a table (E1, E4): its columns in schema order and the
/// names of its primary key columns in key order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Schema {
    pub columns: Vec<Column>,
    pub primary_key: Vec<String>,
}

impl Schema {
    pub fn new(columns: Vec<Column>, primary_key: &[&str]) -> Schema {
        Schema {
            columns,
            primary_key: primary_key.iter().map(|name| (*name).to_owned()).collect(),
        }
    }
}

/// A live column of a table, as [`TableSchema`] gives it (E2).
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaColumn {
    /// The column's identity: random, kept when it is renamed (P12, P19).
    pub id: u64,
    pub name: String,
    pub ty: ColumnType,
    pub nullable: bool,
    /// The default value, normalized for the column (E8).
    pub default: Option<Value<'static>>,
}

/// The current schema of a table, as Describe returns it (E2): an owned copy. Its columns are the live ones in schema
/// order; the version starts at 1 and grows by one with each change that changes something (E4); the table's identity
/// and the schema state are random 64-bit values (P12), the state drawn again at each new version.
#[derive(Debug, Clone, PartialEq)]
pub struct TableSchema {
    table_id: u64,
    columns: Vec<SchemaColumn>,
    /// The identities of the primary key columns, in key order.
    key: Vec<u64>,
    version: u16,
    schema_state: u64,
}

/// A rule of E5 that a schema or a change breaks, with the column it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Invalid {
    pub rule: SchemaRule,
    pub column: Option<String>,
}

/// A difference of E6 between a given schema and a table's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Different {
    pub column: Option<String>,
    pub difference: SchemaDifference,
    /// What the given schema says.
    pub expected: String,
    /// What the table has.
    pub found: String,
}

/// A change of a table's schema (E4).
#[derive(Debug, Clone, Copy)]
pub(crate) enum Change<'a> {
    Add(&'a Column),
    Drop(&'a str),
    Rename {
        from: &'a str,
        to: &'a str,
    },
    Modify {
        name: &'a str,
        ty: ColumnType,
        nullable: bool,
    },
}

impl Change<'_> {
    /// The column names the change gives, whose sizes are checked before anything else (E18).
    pub(crate) fn names(&self) -> Vec<&str> {
        match self {
            Change::Add(column) => vec![&column.name],
            Change::Drop(name) | Change::Modify { name, .. } => vec![name],
            Change::Rename { from, to } => vec![from, to],
        }
    }
}

/// A schema after a change, before its new identities are drawn: see [`Next::identify`].
pub(crate) struct Next {
    schema: TableSchema,
    /// Whether the last column is new and needs an identity.
    added: bool,
}

impl Next {
    /// Whether the change adds a column, which needs an identity besides the schema state.
    pub(crate) fn adds_column(&self) -> bool {
        self.added
    }

    /// The new schema with its new schema state and, when the change adds a column, that column's identity.
    ///
    /// # Panics
    ///
    /// Unless `column_id` is given exactly when [`adds_column`](Self::adds_column) says: an added column never keeps
    /// the placeholder identity of the plan.
    pub(crate) fn identify(mut self, schema_state: u64, column_id: Option<u64>) -> TableSchema {
        self.schema.schema_state = schema_state;
        match (self.added, column_id) {
            (true, Some(id)) => self.schema.columns.last_mut().expect("the added column").id = id,
            (true, None) => panic!("an added column needs its identity"),
            (false, Some(_)) => panic!("only an added column takes an identity"),
            (false, None) => {}
        }
        self.schema
    }
}

/// A column of a schema being checked: one the application declares or one the table has.
struct Draft<'a> {
    name: &'a str,
    ty: ColumnType,
    nullable: bool,
    /// Not null: a null default is no default (E4).
    default: Option<&'a Value<'static>>,
}

impl<'a> Draft<'a> {
    fn of_column(column: &'a Column) -> Draft<'a> {
        Draft {
            name: &column.name,
            ty: column.ty,
            nullable: column.nullable,
            default: column
                .default
                .as_ref()
                .filter(|v| !matches!(v, Value::Null)),
        }
    }

    fn of_schema_column(column: &'a SchemaColumn) -> Draft<'a> {
        Draft {
            name: &column.name,
            ty: column.ty,
            nullable: column.nullable,
            default: column.default.as_ref(),
        }
    }
}

fn invalid<T>(rule: SchemaRule, column: Option<&str>) -> Result<T, Invalid> {
    Err(Invalid {
        rule,
        column: column.map(str::to_owned),
    })
}

/// Checks the rules of E5 that any schema must keep, in E5's order and, within a rule, at the first column in schema
/// order (in key order for the primary key's references), and returns each column's default normalized for it.
fn check_general(
    columns: &[Draft<'_>],
    key: &[&str],
) -> Result<Vec<Option<Value<'static>>>, Invalid> {
    if key.is_empty() {
        return invalid(SchemaRule::NoPrimaryKey, None);
    }
    for column in columns {
        let bad_key = column.nullable
            || matches!(column.ty, ColumnType::Vector(_))
            || column.default.is_some();
        if bad_key && key.contains(&column.name) {
            return invalid(SchemaRule::InvalidKeyColumn, Some(column.name));
        }
    }
    let mut seen = HashSet::new();
    for name in key {
        if !seen.insert(*name) || !columns.iter().any(|c| c.name == *name) {
            return invalid(SchemaRule::InvalidKeyReference, Some(name));
        }
    }
    let mut names = HashSet::new();
    for column in columns {
        if !names.insert(column.name) {
            return invalid(SchemaRule::DuplicateColumnName, Some(column.name));
        }
    }
    if columns.len() > MAX_COLUMNS {
        return invalid(SchemaRule::TooManyColumns, None);
    }
    let broken_type = columns
        .iter()
        .filter_map(|c| check_type(&c.ty).err().map(|rule| (rule, c.name)))
        .min_by_key(|(rule, _)| *rule);
    if let Some((rule, name)) = broken_type {
        return invalid(rule, Some(name));
    }
    let defaults = columns
        .iter()
        .map(|c| match c.default {
            None => Ok(None),
            Some(value) => normalize(&c.ty, c.nullable, value.clone())
                .map(|v| Some(v.into_owned()))
                .or_else(|_| invalid(SchemaRule::InvalidDefault, Some(c.name))),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(defaults)
}

/// Fails with an invalid default unless a schema with `columns` (name, type and normalized default, in schema order)
/// and `key_len` primary key columns fits in a table's catalog entry, whose bytes after the fixed part are at most
/// [`MAX_EXTRA`]: a default may make a valid row of up to 256 MiB (E18) and still not fit with the rest of the schema.
/// The error is the last rule of E5, a schema too large (decided by the user on 2026-11-11, R1 of the code review),
/// and names the column where the schema passes the maximum.
fn check_fits<'a>(
    columns: impl IntoIterator<Item = (&'a str, ColumnType, Option<&'a Value<'a>>)>,
    key_len: usize,
) -> Result<(), Invalid> {
    let mut len = FIXED_LEN + 8 * key_len;
    for (name, ty, default) in columns {
        len += encoded_column_len(name, ty, default);
        if len > MAX_EXTRA {
            return invalid(SchemaRule::SchemaTooLarge, Some(name));
        }
    }
    Ok(())
}

/// Checks a schema given to create a table (E5) and returns each column's default normalized for it.
pub(crate) fn check_new(schema: &Schema) -> Result<Vec<Option<Value<'static>>>, Invalid> {
    let drafts: Vec<Draft<'_>> = schema.columns.iter().map(Draft::of_column).collect();
    let key: Vec<&str> = schema.primary_key.iter().map(String::as_str).collect();
    let defaults = check_general(&drafts, &key)?;
    let stored = drafts.iter().zip(&defaults);
    check_fits(stored.map(|(c, d)| (c.name, c.ty, d.as_ref())), key.len())?;
    Ok(defaults)
}

/// The column names a schema gives, whose sizes are checked before anything else (E18): its columns in schema order,
/// then its primary key in key order.
pub(crate) fn schema_names(schema: &Schema) -> impl Iterator<Item = &str> {
    schema
        .columns
        .iter()
        .map(|c| c.name.as_str())
        .chain(schema.primary_key.iter().map(String::as_str))
}

impl TableSchema {
    /// The schema of a new table at version 1 from `schema`, which [`check_new`] accepted with `defaults`, with the
    /// identities drawn for the table, each column (in schema order) and the schema state.
    pub(crate) fn new(
        table_id: u64,
        schema: &Schema,
        defaults: Vec<Option<Value<'static>>>,
        column_ids: &[u64],
        schema_state: u64,
    ) -> TableSchema {
        let columns: Vec<SchemaColumn> = schema
            .columns
            .iter()
            .zip(defaults)
            .zip(column_ids)
            .map(|((column, default), &id)| SchemaColumn {
                id,
                name: column.name.clone(),
                ty: column.ty,
                nullable: column.nullable,
                default,
            })
            .collect();
        let key = schema
            .primary_key
            .iter()
            .map(|name| {
                columns
                    .iter()
                    .find(|c| &c.name == name)
                    .expect("checked")
                    .id
            })
            .collect();
        TableSchema {
            table_id,
            columns,
            key,
            version: 1,
            schema_state,
        }
    }

    /// The table's identity (P12).
    pub fn table_id(&self) -> u64 {
        self.table_id
    }

    /// The live columns, in schema order.
    pub fn columns(&self) -> &[SchemaColumn] {
        &self.columns
    }

    /// The live column `name`.
    pub fn column(&self, name: &str) -> Option<&SchemaColumn> {
        self.columns.iter().find(|c| c.name == name)
    }

    /// The names of the primary key columns, in key order.
    pub fn primary_key(&self) -> Vec<&str> {
        self.key
            .iter()
            .map(|id| self.by_id(*id).expect("a live key column").name.as_str())
            .collect()
    }

    /// The schema version: 1 when the table was created, one more at each change that changed something (E4).
    pub fn version(&self) -> u16 {
        self.version
    }

    /// The schema state, drawn again at each new version (P12).
    pub fn schema_state(&self) -> u64 {
        self.schema_state
    }

    /// The columns of the current version as the row layout needs them, in schema order, which is the version's.
    pub fn row_columns(&self) -> Vec<RowColumn> {
        self.columns
            .iter()
            .map(|c| RowColumn {
                id: c.id,
                ty: c.ty,
                nullable: c.nullable,
                key: self.key.contains(&c.id),
            })
            .collect()
    }

    /// Fails with a schema too large unless the schema fits in its table's catalog entry (see [`check_fits`]).
    fn check_fits(&self) -> Result<(), Invalid> {
        let columns = self.columns.iter();
        check_fits(
            columns.map(|c| (c.name.as_str(), c.ty, c.default.as_ref())),
            self.key.len(),
        )
    }

    /// The layout of the rows written with the current version.
    pub fn layout(&self) -> RowLayout {
        RowLayout::new(self.version, self.row_columns())
    }

    /// Every identity of this schema: its columns' and its state, which a new identity must differ from.
    pub(crate) fn identities(&self) -> impl Iterator<Item = u64> + '_ {
        self.columns.iter().map(|c| c.id).chain([self.schema_state])
    }

    fn by_id(&self, id: u64) -> Option<&SchemaColumn> {
        self.columns.iter().find(|c| c.id == id)
    }

    fn position(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }

    /// The schema after `change`, `None` when it changes nothing (E4), or the first rule of E5 it breaks: the rules
    /// of the schema that would result in E5's order, then the change's own (not null without default, unsupported
    /// change, column not found, dropping a key column) and last the exhaustion of the version (P19), all detected
    /// before anything changes.
    pub(crate) fn plan(&self, change: &Change<'_>) -> Result<Option<Next>, Invalid> {
        let mut next = self.clone();
        let mut added = false;
        let column = match *change {
            Change::Add(column) => {
                let mut drafts: Vec<Draft<'_>> =
                    self.columns.iter().map(Draft::of_schema_column).collect();
                drafts.push(Draft::of_column(column));
                let key: Vec<&str> = self.primary_key();
                let defaults = check_general(&drafts, &key)?;
                let draft = drafts.last().expect("the added column");
                if !draft.nullable && draft.default.is_none() {
                    return invalid(SchemaRule::NotNullWithoutDefault, Some(&column.name));
                }
                next.columns.push(SchemaColumn {
                    id: 0,
                    name: column.name.clone(),
                    ty: column.ty,
                    nullable: column.nullable,
                    default: defaults.into_iter().last().flatten(),
                });
                added = true;
                &column.name
            }
            Change::Drop(name) => {
                let Some(at) = self.position(name) else {
                    return invalid(SchemaRule::ColumnNotFound, Some(name));
                };
                if self.key.contains(&self.columns[at].id) {
                    return invalid(SchemaRule::DropKeyColumn, Some(name));
                }
                next.columns.remove(at);
                name
            }
            Change::Rename { from, to } => {
                let Some(at) = self.position(from) else {
                    return invalid(SchemaRule::ColumnNotFound, Some(from));
                };
                if from == to {
                    return Ok(None);
                }
                if self.position(to).is_some() {
                    return invalid(SchemaRule::DuplicateColumnName, Some(to));
                }
                next.columns[at].name = to.to_owned();
                from
            }
            Change::Modify { name, ty, nullable } => {
                let at = self.position(name);
                let key = at.is_some_and(|at| self.key.contains(&self.columns[at].id));
                if key && (nullable || matches!(ty, ColumnType::Vector(_))) {
                    return invalid(SchemaRule::InvalidKeyColumn, Some(name));
                }
                if let Err(rule) = check_type(&ty) {
                    return invalid(rule, Some(name));
                }
                let Some(at) = at else {
                    return invalid(SchemaRule::ColumnNotFound, Some(name));
                };
                let current = &self.columns[at];
                if current.ty == ty && current.nullable == nullable {
                    return Ok(None);
                }
                // Only allowing nulls in a column outside the key is supported (N3).
                if current.ty != ty || !nullable || key {
                    return invalid(SchemaRule::UnsupportedChange, Some(name));
                }
                next.columns[at].nullable = true;
                name
            }
        };
        let Some(version) = self.version.checked_add(1) else {
            return invalid(SchemaRule::VersionExhausted, Some(column));
        };
        next.version = version;
        // Adding a column or lengthening a name may make the schema pass the most bytes of its catalog entry: the
        // last rule of E5, which names the column of the change.
        if let Change::Add(_) | Change::Rename { .. } = change {
            let named = match *change {
                Change::Rename { to, .. } => to,
                _ => column,
            };
            next.check_fits()
                .or_else(|_| invalid(SchemaRule::SchemaTooLarge, Some(named)))?;
        }
        Ok(Some(Next {
            schema: next,
            added,
        }))
    }

    /// Whether `given`, a schema that [`check_new`] accepted with `defaults`, is compatible with this one (E6), or
    /// its first difference: each column of `given` in its order, by type, nullability and default; then the primary
    /// key. The table may have other columns.
    pub(crate) fn compare(
        &self,
        given: &Schema,
        defaults: &[Option<Value<'static>>],
    ) -> Result<(), Different> {
        for (column, default) in given.columns.iter().zip(defaults) {
            let different = |difference, expected: String, found: String| {
                Err(Different {
                    column: Some(column.name.clone()),
                    difference,
                    expected,
                    found,
                })
            };
            let Some(found) = self.column(&column.name) else {
                return different(
                    SchemaDifference::MissingColumn,
                    column.ty.to_string(),
                    "no column".to_owned(),
                );
            };
            if found.ty != column.ty {
                return different(
                    SchemaDifference::Type,
                    column.ty.to_string(),
                    found.ty.to_string(),
                );
            }
            if found.nullable != column.nullable {
                return different(
                    SchemaDifference::Nullability,
                    nullability(column.nullable).to_owned(),
                    nullability(found.nullable).to_owned(),
                );
            }
            if !same_default(default.as_ref(), found.default.as_ref()) {
                return different(
                    SchemaDifference::Default,
                    default_text(default.as_ref()),
                    default_text(found.default.as_ref()),
                );
            }
        }
        let table_key = self.primary_key();
        if given
            .primary_key
            .iter()
            .map(String::as_str)
            .ne(table_key.iter().copied())
        {
            return Err(Different {
                column: None,
                difference: SchemaDifference::PrimaryKey,
                expected: format!("({})", given.primary_key.join(", ")),
                found: format!("({})", table_key.join(", ")),
            });
        }
        Ok(())
    }
}

fn nullability(nullable: bool) -> &'static str {
    if nullable {
        "nulls allowed"
    } else {
        "not null"
    }
}

fn default_text(default: Option<&Value<'_>>) -> String {
    default.map_or_else(|| "no default".to_owned(), ToString::to_string)
}

/// Whether two normalized defaults are the same value (see [`Value::same`]).
fn same_default(a: Option<&Value<'_>>, b: Option<&Value<'_>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.same(b),
        (a, b) => a.is_none() && b.is_none(),
    }
}

impl fmt::Display for SchemaColumn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} {}", self.name, self.ty)?;
        if !self.nullable {
            f.write_str(" not null")?;
        }
        match &self.default {
            Some(default) => write!(f, " default {default}"),
            None => Ok(()),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Encodings, all integers little-endian unless said otherwise.

/// Builds the bytes of an encoding.
struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }

    fn u16(&mut self, v: u16) {
        self.0.extend(v.to_le_bytes());
    }

    fn u64(&mut self, v: u64) {
        self.0.extend(v.to_le_bytes());
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.0.extend(
            u32::try_from(bytes.len())
                .expect("under 4 GiB")
                .to_le_bytes(),
        );
        self.0.extend(bytes);
    }

    fn ty(&mut self, ty: &ColumnType) {
        let code = match ty {
            ColumnType::Bool => 0,
            ColumnType::I8 => 1,
            ColumnType::I16 => 2,
            ColumnType::I32 => 3,
            ColumnType::I64 => 4,
            ColumnType::U8 => 5,
            ColumnType::U16 => 6,
            ColumnType::U32 => 7,
            ColumnType::U64 => 8,
            ColumnType::F32 => 9,
            ColumnType::F64 => 10,
            ColumnType::Decimal { .. } => 11,
            ColumnType::String => 12,
            ColumnType::Bytes => 13,
            ColumnType::Date => 14,
            ColumnType::Time => 15,
            ColumnType::Timestamp => 16,
            ColumnType::Duration => 17,
            ColumnType::Uuid(_) => 18,
            ColumnType::Ulid => 19,
            ColumnType::Vector(_) => 20,
        };
        self.u8(code);
        match ty {
            ColumnType::Decimal { precision, scale } => {
                self.u8(*precision);
                self.u8(*scale);
            }
            ColumnType::Uuid(order) => self.u8(match order {
                UuidOrder::Rfc9562 => 0,
                UuidOrder::SqlServer => 1,
            }),
            ColumnType::Vector(vector) => {
                self.u8(match vector.element {
                    VectorElement::F32 => 0,
                    VectorElement::F16 => 1,
                    VectorElement::I8 => 2,
                    VectorElement::Bit => 3,
                });
                self.u16(vector.dimension);
                self.u8(u8::from(vector.cosine));
            }
            _ => {}
        }
    }
}

/// Reads the bytes of an encoding; every read is `None` past the end.
struct Reader<'b>(&'b [u8]);

impl<'b> Reader<'b> {
    fn take(&mut self, n: usize) -> Option<&'b [u8]> {
        if self.0.len() < n {
            return None;
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Some(head)
    }

    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }

    fn bytes(&mut self) -> Option<&'b [u8]> {
        let len = u32::from_le_bytes(self.take(4)?.try_into().ok()?);
        self.take(usize::try_from(len).ok()?)
    }

    /// A type that passes [`check_type`].
    fn ty(&mut self) -> Option<ColumnType> {
        let ty = match self.u8()? {
            0 => ColumnType::Bool,
            1 => ColumnType::I8,
            2 => ColumnType::I16,
            3 => ColumnType::I32,
            4 => ColumnType::I64,
            5 => ColumnType::U8,
            6 => ColumnType::U16,
            7 => ColumnType::U32,
            8 => ColumnType::U64,
            9 => ColumnType::F32,
            10 => ColumnType::F64,
            11 => ColumnType::Decimal {
                precision: self.u8()?,
                scale: self.u8()?,
            },
            12 => ColumnType::String,
            13 => ColumnType::Bytes,
            14 => ColumnType::Date,
            15 => ColumnType::Time,
            16 => ColumnType::Timestamp,
            17 => ColumnType::Duration,
            18 => ColumnType::Uuid(match self.u8()? {
                0 => UuidOrder::Rfc9562,
                1 => UuidOrder::SqlServer,
                _ => return None,
            }),
            19 => ColumnType::Ulid,
            20 => {
                let element = match self.u8()? {
                    0 => VectorElement::F32,
                    1 => VectorElement::F16,
                    2 => VectorElement::I8,
                    3 => VectorElement::Bit,
                    _ => return None,
                };
                let dimension = self.u16()?;
                let cosine = match self.u8()? {
                    0 => false,
                    1 => true,
                    _ => return None,
                };
                ColumnType::Vector(VectorType {
                    element,
                    dimension,
                    cosine,
                })
            }
            _ => return None,
        };
        check_type(&ty).ok().map(|()| ty)
    }

    /// The number of live columns of a schema in the catalog: 1 to [`MAX_COLUMNS`].
    fn column_count(&mut self) -> Option<usize> {
        let count = usize::from(self.u16()?);
        (1..=MAX_COLUMNS).contains(&count).then_some(count)
    }

    /// A live column of a schema in the catalog (see [`TableSchema::encode`]), its name and default borrowing the
    /// bytes.
    fn column(&mut self) -> Option<StoredColumn<'b>> {
        let id = self.u64()?;
        let name = std::str::from_utf8(self.bytes()?).ok()?;
        if !(1..=MAX_COLUMN_NAME).contains(&name.len()) {
            return None;
        }
        let ty = self.ty()?;
        let flags = self.u8()?;
        if flags & !(NULLABLE | DEFAULT) != 0 {
            return None;
        }
        let default = if flags & DEFAULT != 0 {
            let stored = self.bytes()?;
            Some(default_layout(ty).row(stored).ok()?.value(0).ok()??)
        } else {
            None
        };
        Some(StoredColumn {
            id,
            name,
            ty,
            nullable: flags & NULLABLE != 0,
            default,
        })
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A live column as a table's catalog entry stores it, borrowing the entry's bytes.
struct StoredColumn<'b> {
    id: u64,
    name: &'b str,
    ty: ColumnType,
    nullable: bool,
    default: Option<Value<'b>>,
}

/// The identity and the default of each live column, in schema order, of a schema in a table's catalog entry
/// (`bytes`, what [`TableSchema::encode`] writes), the defaults borrowing the entry's bytes (D11); `None` when the bytes
/// are not such a schema: the caller reports damage of the catalog page (G6). A row written before a column with a
/// default was added reads it from here, without a copy made for that read (AC20).
pub(crate) fn decode_defaults(bytes: &[u8]) -> Option<Vec<(u64, Option<Value<'_>>)>> {
    let mut r = Reader(bytes);
    r.take(8 + 2 + 8)?; // the history tree's root, the version and the schema state
    let count = r.column_count()?;
    let mut defaults = Vec::with_capacity(count);
    for _ in 0..count {
        let column = r.column()?;
        defaults.push((column.id, column.default));
    }
    Some(defaults)
}

/// The flag of a column that allows nulls.
const NULLABLE: u8 = 1;
/// The flag of a primary key column, in a version of the history.
const KEY: u8 = 2;
/// The flag of a column with a default value, in the catalog.
const DEFAULT: u8 = 4;

/// The bytes of a schema in its catalog entry ([`TableSchema::encode`]) besides its columns and its primary key
/// columns' identities: the history tree's root, the version, the schema state and the two counts.
const FIXED_LEN: usize = 8 + 2 + 8 + 2 + 2;

/// The bytes that [`TableSchema::encode`] writes for a column named `name` of type `ty` with `default`.
fn encoded_column_len(name: &str, ty: ColumnType, default: Option<&Value<'_>>) -> usize {
    let mut w = Writer(Vec::new());
    w.ty(&ty);
    let default = default.map_or(0, |default| {
        4 + default_layout(ty).encoded_len(std::slice::from_ref(default))
    });
    8 + 4 + name.len() + w.0.len() + 1 + default
}

/// The single-column layout that stores a default value of type `ty`: the row format of document 1, so that a
/// default is stored as a row stores its value.
fn default_layout(ty: ColumnType) -> RowLayout {
    RowLayout::new(
        0,
        vec![RowColumn {
            id: 0,
            ty,
            nullable: false,
            key: false,
        }],
    )
}

impl TableSchema {
    /// The bytes after the catalog's fixed part of the table's entry: the root of its history tree `history`, then
    /// the version, the schema state, the live columns in schema order (identity, name, type, flags, default) and the
    /// identities of the primary key columns in key order.
    pub fn encode(&self, history: u64) -> Vec<u8> {
        let mut w = Writer(Vec::new());
        w.u64(history);
        w.u16(self.version);
        w.u64(self.schema_state);
        w.u16(u16::try_from(self.columns.len()).expect("at most 1,024 columns"));
        for column in &self.columns {
            w.u64(column.id);
            w.bytes(column.name.as_bytes());
            w.ty(&column.ty);
            let flags = if column.nullable { NULLABLE } else { 0 }
                | if column.default.is_some() { DEFAULT } else { 0 };
            w.u8(flags);
            if let Some(default) = &column.default {
                w.bytes(&default_layout(column.ty).encode(std::slice::from_ref(default)));
            }
        }
        w.u16(u16::try_from(self.key.len()).expect("at most 1,024 key columns"));
        for id in &self.key {
            w.u64(*id);
        }
        debug_assert_eq!(
            w.0.len(),
            FIXED_LEN
                + 8 * self.key.len()
                + (self.columns.iter())
                    .map(|c| encoded_column_len(&c.name, c.ty, c.default.as_ref()))
                    .sum::<usize>(),
            "check_fits counts what encode writes"
        );
        w.0
    }

    /// The schema of the table `table_id` and the root of its history tree from `bytes`, or `None` when they are not
    /// what [`encode`](Self::encode) writes: the caller reports damage of the catalog page (G6).
    pub fn decode(table_id: u64, bytes: &[u8]) -> Option<(TableSchema, u64)> {
        let mut r = Reader(bytes);
        let history = r.u64()?;
        let version = r.u16()?;
        let schema_state = r.u64()?;
        let count = r.column_count()?;
        let mut columns = Vec::with_capacity(count);
        for _ in 0..count {
            let column = r.column()?;
            columns.push(SchemaColumn {
                id: column.id,
                name: column.name.to_owned(),
                ty: column.ty,
                nullable: column.nullable,
                default: column.default.map(Value::into_owned),
            });
        }
        let key_count = usize::from(r.u16()?);
        if key_count == 0 || key_count > count {
            return None;
        }
        let mut key = Vec::with_capacity(key_count);
        for _ in 0..key_count {
            let id = r.u64()?;
            let column = columns.iter().find(|c| c.id == id)?;
            // A primary key column without nulls, not a vector, without a default (E4).
            let bad_key = column.nullable
                || matches!(column.ty, ColumnType::Vector(_))
                || column.default.is_some();
            if key.contains(&id) || bad_key {
                return None;
            }
            key.push(id);
        }
        // The other invariants of E5 that Krummi never writes broken: distinct identities and names, a version.
        let mut ids = HashSet::with_capacity(count);
        let mut names = HashSet::with_capacity(count);
        let distinct = columns
            .iter()
            .all(|c| ids.insert(c.id) && names.insert(c.name.as_str()));
        if version == 0 || !distinct {
            return None;
        }
        r.is_empty().then_some((
            TableSchema {
                table_id,
                columns,
                key,
                version,
                schema_state,
            },
            history,
        ))
    }

    /// The value of this version in the history tree (see [`encode_version`]).
    pub fn encode_version(&self) -> Vec<u8> {
        encode_version(self.schema_state, &self.row_columns())
    }
}

/// The key of `version` in the history tree: its big-endian bytes (D3).
pub fn version_key(version: u16) -> [u8; 2] {
    version.to_be_bytes()
}

/// The key of an identity the table has used in its history tree: its big-endian bytes.
pub fn identity_key(id: u64) -> [u8; 8] {
    id.to_be_bytes()
}

/// The value of a version in the history tree: its schema state and its columns in version order, each with its
/// identity, type and flags (nullable, primary key), which is what decoding the rows of that version needs.
pub fn encode_version(schema_state: u64, columns: &[RowColumn]) -> Vec<u8> {
    let mut w = Writer(Vec::new());
    w.u64(schema_state);
    w.u16(u16::try_from(columns.len()).expect("at most 1,024 columns"));
    for column in columns {
        w.u64(column.id);
        w.ty(&column.ty);
        w.u8(if column.nullable { NULLABLE } else { 0 } | if column.key { KEY } else { 0 });
    }
    w.0
}

/// The schema state and the columns of a version from its value in the history tree, or `None` when it is not what
/// [`encode_version`] writes (damage).
pub fn decode_version(bytes: &[u8]) -> Option<(u64, Vec<RowColumn>)> {
    let mut r = Reader(bytes);
    let schema_state = r.u64()?;
    let count = usize::from(r.u16()?);
    let mut columns = Vec::with_capacity(count.min(MAX_COLUMNS));
    for _ in 0..count {
        let id = r.u64()?;
        let ty = r.ty()?;
        let flags = r.u8()?;
        if flags & !(NULLABLE | KEY) != 0 {
            return None;
        }
        columns.push(RowColumn {
            id,
            ty,
            nullable: flags & NULLABLE != 0,
            key: flags & KEY != 0,
        });
    }
    r.is_empty().then_some((schema_state, columns))
}
