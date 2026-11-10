//! Tables in the catalog (E1–E4, E16, E17 and E19 of the tables with schema PRD; N1 of the tables PRD; D2, D3 and
//! D6 of the execution design): opening a table in the modes of A1, Describe, Delete, Rename, the row count and the
//! four schema changes.
//!
//! A table is a catalog entry of kind [`TreeKind::Table`]: the catalog's fixed part holds its random identity and the
//! root and row count of its rows tree, and the bytes after it the root of its schema history tree and its current
//! schema ([`TableSchema::encode`]). So opening, Describe, Exists, List and the row count read only the catalog
//! (E19). Neither internal tree has a name, so no catalog operation reaches it (N1, AC13).
//!
//! A write transaction keeps the state of every table it opened, shared by all its handles (B9), as it does for
//! trees: a schema change writes the catalog entry at once, and the rows tree's root and count reach the catalog at
//! commit. An operation that changes nothing writes nothing (E16).

use std::cell::Ref;
use std::marker::PhantomData;

use krummi_pager::{Error, Result, SizeSubject, TreeKind};
use krummi_tree::PageSource;
use krummi_tree::catalog::{self, Entry};

use super::identity;
use super::schema::{
    Change, Column, Different, Invalid, MAX_COLUMN_NAME, Schema, TableSchema, check_new,
    identity_key, schema_names, version_key,
};
use super::value::ColumnType;
use crate::{Folder, ReadTransaction, Store, WriteTransaction};

/// A table opened in a read transaction: it sees the transaction's snapshot, with the rules of a
/// [`ReadTree`](crate::ReadTree): it can be sent to other threads and shared between them, cannot outlive its store,
/// and fails with [`ErrorKind::Misuse`](crate::ErrorKind::Misuse) without effect in another transaction.
pub struct ReadTable<'s> {
    serial: u64,
    root: u64,
    count: u64,
    history: u64,
    schema: TableSchema,
    _store: PhantomData<&'s Store>,
}

/// A table opened in a write transaction, with the rules of a [`WriteTree`](crate::WriteTree): every handle of the
/// table in that transaction sees the same state, with its rows and its schema as any of them left them (E4, B9); it
/// stays on its thread, cannot outlive its store, fails with [`ErrorKind::Misuse`](crate::ErrorKind::Misuse) without
/// effect in another transaction, and with [`ErrorKind::TreeNotFound`](crate::ErrorKind::TreeNotFound) once its
/// table was deleted, without reading anything (A7).
pub struct WriteTable<'s> {
    serial: u64,
    /// The table's identity (P12).
    id: u64,
    /// The raw pointer keeps the handle on its thread: it is neither `Send` nor `Sync`.
    _store: PhantomData<(&'s Store, *const ())>,
}

/// How [`WriteTransaction::open_table_with`] opens a table (E1, the modes of A1): the modes that may create the table
/// take its schema; [`WriteTransaction::open_table`] only opens.
#[derive(Debug, Clone, Copy)]
pub enum TableMode<'a> {
    /// Creates the empty table with the schema; fails with [`ErrorKind::TreeAlreadyExists`](crate::ErrorKind) if
    /// the table exists.
    CreateNew(&'a Schema),
    /// Opens the table if it exists and the schema is compatible with it (E6), otherwise creates it empty.
    OpenOrCreate(&'a Schema),
    /// Creates the table empty if it does not exist; otherwise, if the schema is compatible with it, removes all its
    /// rows (E3).
    CreateOrEmpty(&'a Schema),
    /// Removes all the rows of the table (E3); fails with [`ErrorKind::TreeNotFound`](crate::ErrorKind) if it does
    /// not exist.
    Empty,
}

impl<'a> TableMode<'a> {
    fn schema(&self) -> Option<&'a Schema> {
        match *self {
            TableMode::CreateNew(schema)
            | TableMode::OpenOrCreate(schema)
            | TableMode::CreateOrEmpty(schema) => Some(schema),
            TableMode::Empty => None,
        }
    }
}

/// The roots of a table's internal trees, which no catalog name reaches (a test hook, AC13).
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableTrees {
    /// The rows tree, keyed by the encoded primary key.
    pub rows: u64,
    /// The schema history tree.
    pub history: u64,
}

/// The tables a write transaction opened, sorted by identity.
#[derive(Default)]
pub(crate) struct Tables(Vec<TableState>);

impl Tables {
    fn find(&self, id: u64) -> std::result::Result<usize, usize> {
        self.0.binary_search_by_key(&id, |state| state.id)
    }

    fn get(&self, id: u64) -> Option<&TableState> {
        self.find(id).ok().map(|at| &self.0[at])
    }

    fn get_mut(&mut self, id: u64) -> Option<&mut TableState> {
        self.find(id).ok().map(|at| &mut self.0[at])
    }
}

/// A table as its write transaction sees it.
pub(crate) struct TableState {
    /// Its identity (P12).
    id: u64,
    /// Its current name or, once deleted, the last one it had (A6).
    name: String,
    /// The root and the row count of its rows tree.
    root: u64,
    count: u64,
    /// The root of its schema history tree.
    history: u64,
    schema: TableSchema,
    deleted: bool,
    /// Its rows tree's root or count differ from its catalog entry, which the commit then rewrites.
    dirty: bool,
}

impl TableState {
    fn entry(&self) -> Entry {
        Entry {
            id: self.id,
            kind: TreeKind::Table,
            root: self.root,
            count: self.count,
        }
    }
}

/// A table found in the catalog.
struct Found {
    entry: Entry,
    schema: TableSchema,
    history: u64,
}

/// The table `name` that `tx` sees, failing with [`Error::DifferentTreeType`] when the name is of another class of
/// object (N1, B3) and with damage of the catalog page when its entry does not decode (G6). It reads only the catalog.
fn find_table<S: PageSource>(folder: Folder<'_>, tx: &S, name: &str) -> Result<Option<Found>> {
    let Some(located) = catalog::lookup_with_extra(tx, name)? else {
        return Ok(None);
    };
    if located.entry.kind != TreeKind::Table {
        return Err(folder.different_class(name, located.entry.kind, TreeKind::Table));
    }
    let (schema, history) = TableSchema::decode(located.entry.id, located.extra)
        .ok_or_else(|| tx.damaged(located.page))?;
    Ok(Some(Found {
        entry: located.entry,
        schema,
        history,
    }))
}

impl Folder<'_> {
    /// Fails with [`Error::InvalidSize`] unless the column name has 1 to 1,024 bytes (E4, E18).
    fn check_column_name(self, name: &str) -> Result<()> {
        self.check_size(SizeSubject::ColumnName, name.len(), 1, MAX_COLUMN_NAME)
    }

    /// The error of a name of class `found` used as one of class `requested` (N1, B3).
    pub(crate) fn different_class(self, name: &str, found: TreeKind, requested: TreeKind) -> Error {
        Error::DifferentTreeType {
            folder: self.owned(),
            name: name.to_owned(),
            found,
            requested,
        }
    }

    fn invalid_schema(self, table: &str, invalid: Invalid) -> Error {
        Error::InvalidSchema {
            folder: self.owned(),
            table: table.to_owned(),
            column: invalid.column,
            rule: invalid.rule,
        }
    }

    fn different_schema(self, table: &str, different: Different) -> Error {
        Error::DifferentSchema {
            folder: self.owned(),
            table: table.to_owned(),
            column: different.column,
            difference: different.difference,
            expected: different.expected.into(),
            found: different.found.into(),
        }
    }

    fn foreign_table(self) -> Error {
        Error::Misuse {
            folder: self.owned(),
            reason: "the table was opened in another transaction",
        }
    }
}

impl<'s> ReadTransaction<'s> {
    /// Opens the table `name` (E1). Fails with [`Error::InvalidSize`] if the name is empty or longer than 1,024
    /// bytes, with [`Error::DifferentTreeType`] if it is a tree or a sequence, and with [`Error::TreeNotFound`] if
    /// the snapshot does not have it. It reads only the catalog.
    pub fn open_table(&self, name: &str) -> Result<ReadTable<'s>> {
        self.folder.check_name(name)?;
        let found = find_table(self.folder, &self.inner, name)?;
        let found = found.ok_or_else(|| self.folder.not_found(name))?;
        Ok(ReadTable {
            serial: self.serial,
            root: found.entry.root,
            count: found.entry.count,
            history: found.history,
            schema: found.schema,
            _store: PhantomData,
        })
    }

    /// The schema of the table `name` in the snapshot, as an owned copy (E2), with the failures of
    /// [`open_table`](Self::open_table). It reads only the catalog.
    pub fn describe_table(&self, name: &str) -> Result<TableSchema> {
        Ok(self.open_table(name)?.schema)
    }

    /// The schema of `table` in the snapshot, as an owned copy.
    pub fn table_schema(&self, table: &ReadTable<'_>) -> Result<TableSchema> {
        self.check_table(table)?;
        Ok(table.schema.clone())
    }

    /// The number of rows of `table` in the snapshot, read from the catalog without reading the table (E19).
    pub fn row_count(&self, table: &ReadTable<'_>) -> Result<u64> {
        self.check_table(table)?;
        Ok(table.count)
    }

    /// A test hook, not part of the interface: the roots of the internal trees of `table`.
    #[doc(hidden)]
    pub fn table_trees(&self, table: &ReadTable<'_>) -> Result<TableTrees> {
        self.check_table(table)?;
        Ok(TableTrees {
            rows: table.root,
            history: table.history,
        })
    }

    fn check_table(&self, table: &ReadTable<'_>) -> Result<()> {
        if table.serial == self.serial {
            Ok(())
        } else {
            Err(self.folder.foreign_table())
        }
    }
}

/// The operations on tables of a write transaction, with the rules of its operations on trees: the ones that take
/// `&mut self` may modify it and end the references read before, also when they fail or change nothing (E16). The
/// failures of public validation ([`Error::InvalidSize`], [`Error::DifferentTreeType`], [`Error::TreeNotFound`],
/// [`Error::TreeAlreadyExists`], [`Error::InvalidSchema`], [`Error::DifferentSchema`] and [`Error::Misuse`] for a
/// handle of another transaction) have no effect and do not prevent the commit; any other failure does (T11). When an
/// operation breaks several rules, it reports the first in the order of E17: names' sizes, a handle of a deleted
/// table, the catalog (another class, not found, already exists), invalid schema, different schema.
impl<'s> WriteTransaction<'s> {
    /// Opens the table `name` (E1), with the failures of [`ReadTransaction::open_table`]. It reads only the catalog
    /// and never compares the schema.
    pub fn open_table(&self, name: &str) -> Result<WriteTable<'s>> {
        self.folder.check_name(name)?;
        let found = find_table(self.folder, &self.inner, name)?;
        let found = found.ok_or_else(|| self.folder.not_found(name))?;
        self.join(name, found)
    }

    /// Opens the table `name` in `mode` (E1). The modes with a schema check the sizes of its column names (1 to 1,024
    /// bytes, [`Error::InvalidSize`]) after the table's name, then the catalog, then validate the schema (E5,
    /// [`Error::InvalidSchema`]) whether or not the table exists, and for an existing table check that the schema is
    /// compatible with it (E6, [`Error::DifferentSchema`]): each column it names exists with the same type,
    /// nullability and default, and the primary key has the same columns in the same order; the table may have
    /// other columns. A name of a tree or a sequence fails with [`Error::DifferentTreeType`] in every mode. Emptying
    /// a table frees the pages of its rows and keeps its schema, its identity and its handles, which see it empty
    /// (E3).
    pub fn open_table_with(&mut self, name: &str, mode: TableMode<'_>) -> Result<WriteTable<'s>> {
        self.folder.check_name(name)?;
        if let Some(schema) = mode.schema() {
            for column in schema_names(schema) {
                self.folder.check_column_name(column)?;
            }
        }
        let found = find_table(self.folder, &self.inner, name)?;
        let checked = |schema: &Schema| {
            check_new(schema).map_err(|invalid| self.folder.invalid_schema(name, invalid))
        };
        match (found, mode) {
            (None, TableMode::Empty) => Err(self.folder.not_found(name)),
            (
                None,
                TableMode::CreateNew(schema)
                | TableMode::OpenOrCreate(schema)
                | TableMode::CreateOrEmpty(schema),
            ) => {
                let defaults = checked(schema)?;
                self.create_table(name, schema, defaults)
            }
            (Some(_), TableMode::CreateNew(_)) => Err(self.folder.already_exists(name)),
            (Some(found), TableMode::OpenOrCreate(schema)) => {
                let defaults = checked(schema)?;
                self.compatible(name, &found, schema, &defaults)?;
                self.join(name, found)
            }
            (Some(found), TableMode::CreateOrEmpty(schema)) => {
                let defaults = checked(schema)?;
                self.compatible(name, &found, schema, &defaults)?;
                self.empty_table(name, found)
            }
            (Some(found), TableMode::Empty) => self.empty_table(name, found),
        }
    }

    /// The schema of the table `name` that this transaction sees, with its own changes, as an owned copy (E2), with
    /// the failures of [`open_table`](Self::open_table). It reads only the catalog.
    pub fn describe_table(&self, name: &str) -> Result<TableSchema> {
        self.folder.check_name(name)?;
        let found = find_table(self.folder, &self.inner, name)?;
        let found = found.ok_or_else(|| self.folder.not_found(name))?;
        Ok(match self.tables.borrow().get(found.entry.id) {
            Some(state) => state.schema.clone(),
            None => found.schema,
        })
    }

    /// The schema of `table` with this transaction's changes, as an owned copy.
    pub fn table_schema(&self, table: &WriteTable<'_>) -> Result<TableSchema> {
        Ok(self.live_table(table)?.schema.clone())
    }

    /// The number of rows of `table`, with this transaction's changes, without reading the table (E19).
    pub fn row_count(&self, table: &WriteTable<'_>) -> Result<u64> {
        Ok(self.live_table(table)?.count)
    }

    /// A test hook, not part of the interface: the roots of the internal trees of `table`.
    #[doc(hidden)]
    pub fn table_trees(&self, table: &WriteTable<'_>) -> Result<TableTrees> {
        let state = self.live_table(table)?;
        Ok(TableTrees {
            rows: state.root,
            history: state.history,
        })
    }

    /// Deletes the table `name` with its rows and frees its pages (E2); a missing name does nothing. Fails with
    /// [`Error::DifferentTreeType`] if the name is of a tree or a sequence. The table's handles then fail with
    /// [`Error::TreeNotFound`].
    pub fn delete_table(&mut self, name: &str) -> Result<()> {
        self.folder.check_name(name)?;
        match find_table(self.folder, &self.inner, name)? {
            Some(found) => self.drop_table(name, found),
            None => Ok(()),
        }
    }

    /// Renames the table `from` to `to`; it keeps its schema, its rows, its identity and its handles (E2). Fails with
    /// [`Error::TreeNotFound`] if `from` does not exist, also when `to` is the same name, with
    /// [`Error::DifferentTreeType`] if `from` is a tree or a sequence, and with [`Error::TreeAlreadyExists`] if `to`
    /// exists, unless `overwrite` is set: then a table `to` is deleted first, as [`delete_table`](Self::delete_table)
    /// does, and a tree or a sequence fails with [`Error::DifferentTreeType`] (N1). Renaming a table to its own name
    /// does nothing.
    pub fn rename_table(&mut self, from: &str, to: &str, overwrite: bool) -> Result<()> {
        self.folder.check_name(from)?;
        self.folder.check_name(to)?;
        let source = find_table(self.folder, &self.inner, from)?;
        let source = source.ok_or_else(|| self.folder.not_found(from))?;
        if from == to {
            return Ok(());
        }
        let target = catalog::lookup(&self.inner, to)?;
        match target {
            Some(_) if !overwrite => return Err(self.folder.already_exists(to)),
            Some(entry) if entry.kind != TreeKind::Table => {
                return Err(self.folder.different_class(to, entry.kind, TreeKind::Table));
            }
            _ => {}
        }
        let renamed = krummi_tree::copy_str(&self.inner, to)?;
        let id = self.join(from, source)?.id;
        if target.is_some() {
            let target = find_table(self.folder, &self.inner, to)?.expect("looked up above");
            self.drop_table(to, target)?;
        }
        let state = self.tables.get_mut().get_mut(id).expect("joined above");
        state.name = renamed;
        state.dirty = false;
        let (entry, extra) = (state.entry(), state.schema.encode(state.history));
        catalog::put_with_extra(&mut self.inner, to, &entry, &extra)?;
        catalog::remove(&mut self.inner, from)
    }

    /// Adds `column` at the end of the schema of `table` (E4): a new column with a new identity, also when a dropped
    /// column had its name, whose rows written before read it null or with its default. Fails with
    /// [`Error::InvalidSize`] if its name is empty or longer than 1,024 bytes, and with [`Error::InvalidSchema`] if the
    /// schema would break a rule of E5: a name that exists, more than 1,024 columns, its type's limits, a default
    /// that breaks the rules of its column, no nulls without a default, a version past 65,535. It writes the same
    /// pages whatever the number of rows.
    pub fn add_column(&mut self, table: &WriteTable<'_>, column: &Column) -> Result<()> {
        self.change_schema(table, Change::Add(column))
    }

    /// Drops the column `name` of `table` (E4): no row reads it again. Fails with [`Error::InvalidSchema`] if it does
    /// not exist or is a primary key column, or the version would pass 65,535.
    pub fn drop_column(&mut self, table: &WriteTable<'_>, name: &str) -> Result<()> {
        self.change_schema(table, Change::Drop(name))
    }

    /// Renames the column `from` of `table` to `to`; it keeps its identity (E2, E4). Renaming it to its own name
    /// changes nothing. Fails with [`Error::InvalidSchema`] if `from` does not exist, `to` does, or the version would
    /// pass 65,535.
    pub fn rename_column(&mut self, table: &WriteTable<'_>, from: &str, to: &str) -> Result<()> {
        self.change_schema(table, Change::Rename { from, to })
    }

    /// Modifies the column `name` of `table` to type `ty` and nullability `nullable` (E4): only allowing nulls in a
    /// column outside the primary key is supported; giving its type and nullability changes nothing. Fails with
    /// [`Error::InvalidSchema`] for a type outside its limits, a column that does not exist, a primary key column that
    /// would allow nulls or be a vector, a change of type, forbidding nulls or changing a primary key column, and a
    /// version past 65,535.
    pub fn modify_column(
        &mut self,
        table: &WriteTable<'_>,
        name: &str,
        ty: ColumnType,
        nullable: bool,
    ) -> Result<()> {
        self.change_schema(table, Change::Modify { name, ty, nullable })
    }

    /// A handle of the table of `found`, now named `name`; the table's state joins the transaction on its first
    /// opening.
    fn join(&self, name: &str, found: Found) -> Result<WriteTable<'s>> {
        let mut tables = self.tables.borrow_mut();
        if let Err(at) = tables.find(found.entry.id) {
            let name = krummi_tree::copy_str(&self.inner, name)?;
            krummi_tree::reserve(&self.inner, &mut tables.0, 1)?;
            tables.0.insert(
                at,
                TableState {
                    id: found.entry.id,
                    name,
                    root: found.entry.root,
                    count: found.entry.count,
                    history: found.history,
                    schema: found.schema,
                    deleted: false,
                    dirty: false,
                },
            );
        }
        Ok(WriteTable {
            serial: self.serial,
            id: found.entry.id,
            _store: PhantomData,
        })
    }

    /// The state of the table of `table`. Fails with misuse for a handle of another transaction and with not found
    /// once the table was deleted, without reading anything (A7, E17).
    fn live_table(&self, table: &WriteTable<'_>) -> Result<Ref<'_, TableState>> {
        if table.serial != self.serial {
            return Err(self.folder.foreign_table());
        }
        let state = Ref::map(self.tables.borrow(), |tables| {
            tables.get(table.id).expect("a live handle")
        });
        if state.deleted {
            return Err(self.folder.not_found(&state.name));
        }
        Ok(state)
    }

    /// Whether `given`, valid by E5 with `defaults`, is compatible with the table of `found` (E6).
    fn compatible(
        &self,
        name: &str,
        found: &Found,
        given: &Schema,
        defaults: &[Option<super::value::Value<'static>>],
    ) -> Result<()> {
        let tables = self.tables.borrow();
        let schema = tables
            .get(found.entry.id)
            .map_or(&found.schema, |state| &state.schema);
        schema
            .compare(given, defaults)
            .map_err(|different| self.folder.different_schema(name, different))
    }

    /// Creates the empty table `name` with `schema`, which E5 accepted with `defaults`: its identities are drawn
    /// first, so that a failure of the generator changes nothing; then its rows tree, its history tree with version 1
    /// and the identities it used, and its catalog entry.
    fn create_table(
        &mut self,
        name: &str,
        schema: &Schema,
        defaults: Vec<Option<super::value::Value<'static>>>,
    ) -> Result<WriteTable<'s>> {
        let table_id = self.draw_identity(|tx, id| {
            Ok(catalog::has_identity(&tx.inner, TreeKind::Table, id)?
                || tx.tables.borrow().get(id).is_some())
        })?;
        let mut drawn: Vec<u64> = Vec::with_capacity(schema.columns.len() + 1);
        for _ in 0..=schema.columns.len() {
            let id = self.draw_identity(|_, id| Ok(drawn.contains(&id)))?;
            drawn.push(id);
        }
        let schema_state = drawn.pop().expect("one more than the columns");
        let schema = TableSchema::new(table_id, schema, defaults, &drawn, schema_state);
        let root = krummi_tree::create(&mut self.inner)?;
        let history = krummi_tree::create(&mut self.inner)?;
        let history = self.record_version(history, &schema, schema.identities())?;
        let entry = Entry {
            id: table_id,
            kind: TreeKind::Table,
            root,
            count: 0,
        };
        catalog::put_with_extra(&mut self.inner, name, &entry, &schema.encode(history))?;
        self.join(
            name,
            Found {
                entry,
                schema,
                history,
            },
        )
    }

    /// Empties the table `name` of `found`: it keeps its identity, its schema and its handles, and the pages of its
    /// rows give way to one empty root (E3).
    fn empty_table(&mut self, name: &str, found: Found) -> Result<WriteTable<'s>> {
        let handle = self.join(name, found)?;
        let old = self.tables.get_mut().get(handle.id).expect("joined").root;
        krummi_tree::free_all(&mut self.inner, old)?;
        let root = krummi_tree::create(&mut self.inner)?;
        let state = self.tables.get_mut().get_mut(handle.id).expect("joined");
        state.root = root;
        state.count = 0;
        state.dirty = true;
        Ok(handle)
    }

    /// Deletes the table `name` of `found`, ending its identity: its trees' pages are freed, and its handles fail
    /// from now on (E2, A6).
    fn drop_table(&mut self, name: &str, found: Found) -> Result<()> {
        let id = self.join(name, found)?.id;
        let (root, history) = {
            let state = self.tables.get_mut().get(id).expect("joined");
            (state.root, state.history)
        };
        krummi_tree::free_all(&mut self.inner, root)?;
        krummi_tree::free_all(&mut self.inner, history)?;
        catalog::remove(&mut self.inner, name)?;
        let state = self.tables.get_mut().get_mut(id).expect("joined");
        state.deleted = true;
        state.dirty = false;
        Ok(())
    }

    /// Applies a schema change to the table of `table` (E4): the names' sizes, then the handle, then the rules of E5;
    /// a change that changes nothing writes nothing. Otherwise it draws the new identities, records the new version
    /// and its identities in the history tree and writes the catalog entry, so that every handle and Describe see
    /// the new schema at once (B9).
    fn change_schema(&mut self, table: &WriteTable<'_>, change: Change<'_>) -> Result<()> {
        for name in change.names() {
            self.folder.check_column_name(name)?;
        }
        let (next, history) = {
            let state = self.live_table(table)?;
            let next = state
                .schema
                .plan(&change)
                .map_err(|invalid| self.folder.invalid_schema(&state.name, invalid))?;
            (next, state.history)
        };
        let Some(next) = next else {
            return Ok(());
        };
        let known = |tx: &Self, id: u64| {
            Ok(krummi_tree::get(&tx.inner, history, &identity_key(id))?.is_some())
        };
        let schema_state = self.draw_identity(known)?;
        let column_id = if next.adds_column() {
            Some(self.draw_identity(|tx, id| Ok(id == schema_state || known(tx, id)?))?)
        } else {
            None
        };
        let schema = next.identify(schema_state, column_id);
        let history = self.record_version(
            history,
            &schema,
            column_id.into_iter().chain([schema_state]),
        )?;
        let state = self
            .tables
            .get_mut()
            .get_mut(table.id)
            .expect("a live handle");
        state.schema = schema;
        state.history = history;
        state.dirty = false;
        let (name, entry, extra) = (
            state.name.clone(),
            state.entry(),
            state.schema.encode(history),
        );
        catalog::put_with_extra(&mut self.inner, &name, &entry, &extra)
    }

    /// A random identity that `known` does not know (D6). A failure of the generator is the transaction's (T11).
    fn draw_identity(&self, mut known: impl FnMut(&Self, u64) -> Result<bool>) -> Result<u64> {
        identity::draw(|id| known(self, id)).map_err(|error| match error {
            Error::Io { .. } => self.inner.fail(error),
            other => other,
        })
    }

    /// Records the current version of `schema` and the identities `new` in the history tree at `history`, and returns
    /// its root.
    fn record_version(
        &mut self,
        history: u64,
        schema: &TableSchema,
        new: impl IntoIterator<Item = u64>,
    ) -> Result<u64> {
        let mut root = krummi_tree::set(
            &mut self.inner,
            history,
            &version_key(schema.version()),
            &schema.encode_version(),
        )?
        .root;
        for id in new {
            root = krummi_tree::set(&mut self.inner, root, &identity_key(id), &[])?.root;
        }
        Ok(root)
    }

    /// Writes into the catalog the root and the row count of every table whose rows changed them (see
    /// [`WriteTransaction::finish`]).
    pub(crate) fn finish_tables(&mut self) -> Result<()> {
        let tables = self.tables.get_mut();
        for state in tables.0.iter().filter(|s| s.dirty && !s.deleted) {
            catalog::put_with_extra(
                &mut self.inner,
                &state.name,
                &state.entry(),
                &state.schema.encode(state.history),
            )?;
        }
        Ok(())
    }
}
