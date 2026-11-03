//! LMDB through heed (D8): a map of 1 GiB, `DUP_SORT` for the multi-value tree and native-endian `u64` keys with
//! `INTEGER_KEY` for the fixed-size tree, every tree in the unnamed database of its own store. Nothing else changes:
//! the environment is opened without flags, so every commit syncs. lmdb.h, `mdb_env_sync`: "Data is always written
//! to disk when #mdb_txn_commit() is called, but the operating system may keep it buffered. LMDB always flushes the
//! OS buffers upon commit as well, unless the environment was opened with #MDB_NOSYNC or in part #MDB_NOMETASYNC."
//!
//! This is the only file of the benchmark with unsafe code (K14): opening an environment is an unsafe function of
//! heed's public interface.

use std::path::Path;

use heed::byteorder::NativeEndian;
use heed::types::{Bytes, U64};
use heed::{
    CommitOrAbort, Database, DatabaseFlags, Env, EnvOpenOptions, IntegerComparator, RoTxn, RwTxn,
    WithTls,
};

use super::{Result, Tree, byte_sum, cause, wrong_tree};

/// The map size (D8): on Windows the data file takes the whole map and every repetition copies it, so it is kept
/// small; 1 GiB holds the largest store of the benchmark.
const MAP_SIZE: usize = 1 << 30;

type BytesDb = Database<'static, Bytes, Bytes>;
type FixedDb = Database<'static, U64<NativeEndian>, Bytes, IntegerComparator>;

#[derive(Clone, Copy)]
enum Db {
    /// The normal tree, or with `DUP_SORT` the multi-value tree.
    Bytes(BytesDb),
    Fixed(FixedDb),
}

pub struct Store {
    env: Env<WithTls>,
    db: Db,
    tree: Tree,
}

pub struct Reader<'t> {
    txn: RoTxn<'t, WithTls>,
    db: Db,
    tree: Tree,
}

pub struct Writer<'t> {
    txn: RwTxn<'t>,
    db: Db,
    tree: Tree,
}

/// The version of the LMDB library heed compiles, for the report.
pub fn version() -> String {
    let v = heed::lmdb_version();
    format!("{}.{}.{}", v.major, v.minor, v.patch)
}

fn open_env(path: &Path) -> Result<Env<WithTls>> {
    let mut options = EnvOpenOptions::new();
    options.map_size(MAP_SIZE);
    // SAFETY: the folder is a store of this process only: nothing else maps or modifies its files while it is open,
    // it is on a local disk, and the lock file is LMDB's own (the conditions of heed's `EnvOpenOptions::open`).
    #[allow(unsafe_code)]
    let env = unsafe { options.open(path) };
    env.map_err(cause)
}

fn open_db(env: &Env<WithTls>, tree: Tree) -> Result<Db> {
    match tree {
        // The unnamed database always exists and needs no flags.
        Tree::Normal => {
            let rtxn = env.read_txn().map_err(cause)?;
            let db = env
                .open_databases_and_commit(rtxn, |opener, rtxn| {
                    opener
                        .open_database::<Bytes, Bytes>(rtxn, None)
                        .map(CommitOrAbort::<_, ()>::Commit)
                })
                .map_err(cause)?
                .unwrap_commit()
                .ok_or("LMDB no tiene la base de datos sin nombre")?;
            Ok(Db::Bytes(db))
        }
        // The flags of a database are stored when a write transaction creates it, so these open it in one; on a
        // store that has them already, the transaction changes nothing. heed 0.23 deprecates setting
        // `DatabaseFlags::INTEGER_KEY` by hand: its `IntegerComparator` sets `MDB_INTEGERKEY` itself and keeps LMDB's
        // own comparison of native-endian integers.
        Tree::Multi => {
            let wtxn = env.write_txn().map_err(cause)?;
            let db = env
                .create_databases_and_commit(wtxn, |opener, wtxn| {
                    opener
                        .database_options()
                        .types::<Bytes, Bytes>()
                        .flags(DatabaseFlags::DUP_SORT)
                        .create(wtxn)
                        .map(CommitOrAbort::<_, ()>::Commit)
                })
                .map_err(cause)?
                .unwrap_commit();
            Ok(Db::Bytes(db))
        }
        Tree::Fixed => {
            let wtxn = env.write_txn().map_err(cause)?;
            let db = env
                .create_databases_and_commit(wtxn, |opener, wtxn| {
                    opener
                        .database_options()
                        .types::<U64<NativeEndian>, Bytes>()
                        .key_comparator::<IntegerComparator>()
                        .create(wtxn)
                        .map(CommitOrAbort::<_, ()>::Commit)
                })
                .map_err(cause)?
                .unwrap_commit();
            Ok(Db::Fixed(db))
        }
    }
}

impl Db {
    fn bytes(self, tree: Tree) -> Result<BytesDb> {
        match self {
            Db::Bytes(db) => Ok(db),
            Db::Fixed(_) => Err(wrong_tree(tree)),
        }
    }

    fn fixed(self) -> Result<FixedDb> {
        match self {
            Db::Fixed(db) => Ok(db),
            Db::Bytes(_) => Err(wrong_tree(Tree::Fixed)),
        }
    }
}

impl super::Store for Store {
    type Reader<'t> = Reader<'t>;
    type Writer<'t> = Writer<'t>;

    fn open(path: &Path, tree: Tree) -> Result<Self> {
        let env = open_env(path)?;
        let db = open_db(&env, tree)?;
        Ok(Store { env, db, tree })
    }

    fn close(self) -> Result<()> {
        // The environment closes when its last handle goes; this was the only one.
        self.env.prepare_for_closing().wait();
        Ok(())
    }

    fn read<R>(&self, f: impl FnOnce(&Reader<'_>) -> Result<R>) -> Result<R> {
        let txn = self.env.read_txn().map_err(cause)?;
        f(&Reader {
            txn,
            db: self.db,
            tree: self.tree,
        })
    }

    fn write<R>(&self, f: impl FnOnce(&mut Writer<'_>) -> Result<R>) -> Result<R> {
        let txn = self.env.write_txn().map_err(cause)?;
        let mut writer = Writer {
            txn,
            db: self.db,
            tree: self.tree,
        };
        let result = f(&mut writer)?;
        writer.txn.commit().map_err(cause)?;
        Ok(result)
    }
}

fn check(actual: Tree, expected: Tree) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(wrong_tree(expected))
    }
}

impl super::Reader for Reader<'_> {
    fn get(&self, key: &[u8]) -> Result<Option<u64>> {
        check(self.tree, Tree::Normal)?;
        let db = self.db.bytes(Tree::Normal)?;
        Ok(db.get(&self.txn, key).map_err(cause)?.map(byte_sum))
    }

    fn scan(&self, start: Option<&[u8]>, limit: u64) -> Result<(u64, u64)> {
        check(self.tree, Tree::Normal)?;
        let db = self.db.bytes(Tree::Normal)?;
        let limit = limit.try_into().unwrap_or(usize::MAX);
        let (mut count, mut sum) = (0, 0);
        let mut add = |entry: heed::Result<(&[u8], &[u8])>| -> Result<()> {
            let (key, value) = entry.map_err(cause)?;
            count += 1;
            sum += byte_sum(key) + byte_sum(value);
            Ok(())
        };
        match start {
            Some(start) => {
                let bounds = (std::ops::Bound::Included(start), std::ops::Bound::Unbounded);
                for entry in db.range(&self.txn, &bounds).map_err(cause)?.take(limit) {
                    add(entry)?;
                }
            }
            None => {
                for entry in db.iter(&self.txn).map_err(cause)?.take(limit) {
                    add(entry)?;
                }
            }
        }
        Ok((count, sum))
    }

    fn values(&self, key: &[u8]) -> Result<(u64, u64)> {
        check(self.tree, Tree::Multi)?;
        let db = self.db.bytes(Tree::Multi)?;
        let (mut count, mut sum) = (0, 0);
        if let Some(values) = db.get_duplicates(&self.txn, key).map_err(cause)? {
            for entry in values {
                let (_, value) = entry.map_err(cause)?;
                count += 1;
                sum += byte_sum(value);
            }
        }
        Ok((count, sum))
    }

    fn get_fixed(&self, key: u64) -> Result<Option<u64>> {
        let db = self.db.fixed()?;
        Ok(db.get(&self.txn, &key).map_err(cause)?.map(byte_sum))
    }
}

impl super::Writer for Writer<'_> {
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        check(self.tree, Tree::Normal)?;
        let db = self.db.bytes(Tree::Normal)?;
        db.put(&mut self.txn, key, value).map_err(cause)
    }

    fn remove(&mut self, key: &[u8]) -> Result<()> {
        check(self.tree, Tree::Normal)?;
        let db = self.db.bytes(Tree::Normal)?;
        db.delete(&mut self.txn, key).map(drop).map_err(cause)
    }

    fn add(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        // With `DUP_SORT`, putting a key that is there adds the value to its values.
        check(self.tree, Tree::Multi)?;
        let db = self.db.bytes(Tree::Multi)?;
        db.put(&mut self.txn, key, value).map_err(cause)
    }

    fn put_fixed(&mut self, key: u64, value: &[u8]) -> Result<()> {
        let db = self.db.fixed()?;
        db.put(&mut self.txn, &key, value).map_err(cause)
    }
}
