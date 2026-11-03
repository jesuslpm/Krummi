//! redb with its defaults (D8): a cache of 1 GiB (`Builder::new`: "`cache_size_bytes`: 1GiB") and
//! `Durability::Immediate`, the default durability of a write transaction: "Commits with this durability level are
//! guaranteed to be persistent as soon as [`WriteTransaction::commit`] returns" (redb 4.4.0, src/transactions.rs).
//! The multi-value tree is a multimap table and the fixed-size tree a table with `u64` keys (K6).

use std::marker::PhantomData;
use std::path::Path;

use redb::{
    MultimapTable, MultimapTableDefinition, ReadOnlyMultimapTable, ReadOnlyTable, ReadableDatabase,
    Table, TableDefinition,
};

use super::{FIXED_NAME, MULTI_NAME, NORMAL_NAME, Result, Tree, byte_sum, cause, wrong_tree};

const NORMAL: TableDefinition<&[u8], &[u8]> = TableDefinition::new(NORMAL_NAME);
const MULTI: MultimapTableDefinition<&[u8], &[u8]> = MultimapTableDefinition::new(MULTI_NAME);
const FIXED: TableDefinition<u64, &[u8]> = TableDefinition::new(FIXED_NAME);

/// The file of the store inside its folder.
const FILE: &str = "data.redb";

pub struct Store {
    db: redb::Database,
    tree: Tree,
}

enum ReadTable {
    Normal(ReadOnlyTable<&'static [u8], &'static [u8]>),
    Multi(ReadOnlyMultimapTable<&'static [u8], &'static [u8]>),
    Fixed(ReadOnlyTable<u64, &'static [u8]>),
}

pub struct Reader<'t> {
    table: ReadTable,
    _txn: redb::ReadTransaction,
    _store: PhantomData<&'t Store>,
}

pub enum Writer<'t> {
    Normal(Table<'t, &'static [u8], &'static [u8]>),
    Multi(MultimapTable<'t, &'static [u8], &'static [u8]>),
    Fixed(Table<'t, u64, &'static [u8]>),
}

impl super::Store for Store {
    type Reader<'t> = Reader<'t>;
    type Writer<'t> = Writer<'t>;

    fn open(path: &Path, tree: Tree) -> Result<Self> {
        let db = redb::Database::create(path.join(FILE)).map_err(cause)?;
        Ok(Store { db, tree })
    }

    fn close(self) -> Result<()> {
        drop(self.db);
        Ok(())
    }

    fn read<R>(&self, f: impl FnOnce(&Reader<'_>) -> Result<R>) -> Result<R> {
        let txn = self.db.begin_read().map_err(cause)?;
        let table = match self.tree {
            Tree::Normal => ReadTable::Normal(txn.open_table(NORMAL).map_err(cause)?),
            Tree::Multi => ReadTable::Multi(txn.open_multimap_table(MULTI).map_err(cause)?),
            Tree::Fixed => ReadTable::Fixed(txn.open_table(FIXED).map_err(cause)?),
        };
        f(&Reader {
            table,
            _txn: txn,
            _store: PhantomData,
        })
    }

    fn write<R>(&self, f: impl FnOnce(&mut Writer<'_>) -> Result<R>) -> Result<R> {
        let txn = self.db.begin_write().map_err(cause)?;
        let result = {
            let mut writer = match self.tree {
                Tree::Normal => Writer::Normal(txn.open_table(NORMAL).map_err(cause)?),
                Tree::Multi => Writer::Multi(txn.open_multimap_table(MULTI).map_err(cause)?),
                Tree::Fixed => Writer::Fixed(txn.open_table(FIXED).map_err(cause)?),
            };
            f(&mut writer)?
        };
        txn.commit().map_err(cause)?;
        Ok(result)
    }
}

impl super::Reader for Reader<'_> {
    fn get(&self, key: &[u8]) -> Result<Option<u64>> {
        let ReadTable::Normal(table) = &self.table else {
            return Err(wrong_tree(Tree::Normal));
        };
        Ok(table
            .get(key)
            .map_err(cause)?
            .map(|value| byte_sum(value.value())))
    }

    fn scan(&self, start: Option<&[u8]>, limit: u64) -> Result<(u64, u64)> {
        let ReadTable::Normal(table) = &self.table else {
            return Err(wrong_tree(Tree::Normal));
        };
        let range = match start {
            Some(start) => table.range::<&[u8]>(start..),
            None => table.range::<&[u8]>(..),
        }
        .map_err(cause)?;
        let (mut count, mut sum) = (0, 0);
        for entry in range.take(limit.try_into().unwrap_or(usize::MAX)) {
            let (key, value) = entry.map_err(cause)?;
            count += 1;
            sum += byte_sum(key.value()) + byte_sum(value.value());
        }
        Ok((count, sum))
    }

    fn values(&self, key: &[u8]) -> Result<(u64, u64)> {
        let ReadTable::Multi(table) = &self.table else {
            return Err(wrong_tree(Tree::Multi));
        };
        let (mut count, mut sum) = (0, 0);
        for value in table.get(key).map_err(cause)? {
            count += 1;
            sum += byte_sum(value.map_err(cause)?.value());
        }
        Ok((count, sum))
    }

    fn get_fixed(&self, key: u64) -> Result<Option<u64>> {
        let ReadTable::Fixed(table) = &self.table else {
            return Err(wrong_tree(Tree::Fixed));
        };
        Ok(table
            .get(key)
            .map_err(cause)?
            .map(|value| byte_sum(value.value())))
    }
}

impl super::Writer for Writer<'_> {
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        let Writer::Normal(table) = self else {
            return Err(wrong_tree(Tree::Normal));
        };
        table.insert(key, value).map(drop).map_err(cause)
    }

    fn remove(&mut self, key: &[u8]) -> Result<()> {
        let Writer::Normal(table) = self else {
            return Err(wrong_tree(Tree::Normal));
        };
        table.remove(key).map(drop).map_err(cause)
    }

    fn add(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        let Writer::Multi(table) = self else {
            return Err(wrong_tree(Tree::Multi));
        };
        table.insert(key, value).map(drop).map_err(cause)
    }

    fn put_fixed(&mut self, key: u64, value: &[u8]) -> Result<()> {
        let Writer::Fixed(table) = self else {
            return Err(wrong_tree(Tree::Fixed));
        };
        table.insert(key, value).map(drop).map_err(cause)
    }
}
