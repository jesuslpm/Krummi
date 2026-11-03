//! fjall (D8): the single-writer transactional database `SingleWriterTxDatabase` (not the optimistic one, K3), a
//! block cache of 1 GiB, and every write transaction with `durability(Some(PersistMode::SyncData))`, which "Flushes
//! data using `fdatasync`" when it commits (fjall 3.1.13, src/journal/writer.rs); without it a write transaction
//! only flushes to the operating system's buffers (`PersistMode::Buffer`). It has no multi-value trees or integer
//! keys (K6).

use std::path::Path;

use fjall::{
    KeyspaceCreateOptions, PersistMode, Readable, SingleWriterTxDatabase, SingleWriterTxKeyspace,
    SingleWriterWriteTx, Snapshot,
};

use super::{NORMAL_NAME, Result, Tree, byte_sum, cause, unsupported};

/// The block cache (K3): 1 GiB.
const BLOCK_CACHE: u64 = 1 << 30;

pub struct Store {
    db: SingleWriterTxDatabase,
    keyspace: SingleWriterTxKeyspace,
}

pub struct Reader<'t> {
    snapshot: Snapshot,
    keyspace: &'t SingleWriterTxKeyspace,
}

pub struct Writer<'t> {
    tx: SingleWriterWriteTx<'t>,
    keyspace: &'t SingleWriterTxKeyspace,
}

impl super::Store for Store {
    type Reader<'t> = Reader<'t>;
    type Writer<'t> = Writer<'t>;

    fn open(path: &Path, tree: Tree) -> Result<Self> {
        if tree != Tree::Normal {
            return Err(unsupported("fjall", tree));
        }
        let db = SingleWriterTxDatabase::builder(path)
            .cache_size(BLOCK_CACHE)
            .open()
            .map_err(cause)?;
        let keyspace = db
            .keyspace(NORMAL_NAME, KeyspaceCreateOptions::default)
            .map_err(cause)?;
        Ok(Store { db, keyspace })
    }

    fn close(self) -> Result<()> {
        // The last handle of the database waits for its background threads.
        drop(self.keyspace);
        drop(self.db);
        Ok(())
    }

    fn read<R>(&self, f: impl FnOnce(&Reader<'_>) -> Result<R>) -> Result<R> {
        f(&Reader {
            snapshot: self.db.read_tx(),
            keyspace: &self.keyspace,
        })
    }

    fn write<R>(&self, f: impl FnOnce(&mut Writer<'_>) -> Result<R>) -> Result<R> {
        let tx = self.db.write_tx().durability(Some(PersistMode::SyncData));
        let mut writer = Writer {
            tx,
            keyspace: &self.keyspace,
        };
        let result = f(&mut writer)?;
        writer.tx.commit().map_err(cause)?;
        Ok(result)
    }
}

impl super::Reader for Reader<'_> {
    fn get(&self, key: &[u8]) -> Result<Option<u64>> {
        Ok(self
            .snapshot
            .get(self.keyspace, key)
            .map_err(cause)?
            .map(|value| byte_sum(&value)))
    }

    fn scan(&self, start: Option<&[u8]>, limit: u64) -> Result<(u64, u64)> {
        let range = match start {
            Some(start) => self.snapshot.range(self.keyspace, start..),
            None => self.snapshot.iter(self.keyspace),
        };
        let (mut count, mut sum) = (0, 0);
        for guard in range.take(limit.try_into().unwrap_or(usize::MAX)) {
            let (key, value) = guard.into_inner().map_err(cause)?;
            count += 1;
            sum += byte_sum(&key) + byte_sum(&value);
        }
        Ok((count, sum))
    }

    fn values(&self, _key: &[u8]) -> Result<(u64, u64)> {
        Err(unsupported("fjall", Tree::Multi))
    }

    fn get_fixed(&self, _key: u64) -> Result<Option<u64>> {
        Err(unsupported("fjall", Tree::Fixed))
    }
}

impl super::Writer for Writer<'_> {
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        self.tx.insert(self.keyspace, key, value);
        Ok(())
    }

    fn remove(&mut self, key: &[u8]) -> Result<()> {
        self.tx.remove(self.keyspace, key);
        Ok(())
    }

    fn add(&mut self, _key: &[u8], _value: &[u8]) -> Result<()> {
        Err(unsupported("fjall", Tree::Multi))
    }

    fn put_fixed(&mut self, _key: u64, _value: &[u8]) -> Result<()> {
        Err(unsupported("fjall", Tree::Fixed))
    }
}
