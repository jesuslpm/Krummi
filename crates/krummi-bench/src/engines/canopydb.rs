//! canopydb (D8): its default options, which already have a page cache of 1 GiB and the write-ahead log on, the single-writer transaction
//! `begin_write` (not `begin_write_concurrent`, K3), and every commit with `commit_with(true)`, which "Commits the
//! transaction and optionally performs a durable `fsync` to make the transaction (and all the ones before it)
//! durable in the storage" (canopydb 0.2.5, src/lib.rs). It has no multi-value trees or integer keys (K6).

use std::path::Path;

use canopydb::{DbOptions, EnvOptions};

use super::{NORMAL_NAME, Result, Tree, byte_sum, cause, unsupported};

pub struct Store {
    db: canopydb::Database,
}

pub struct Reader<'t> {
    tree: canopydb::Tree<'t>,
}

pub struct Writer<'t> {
    tree: canopydb::Tree<'t>,
}

impl super::Store for Store {
    type Reader<'t> = Reader<'t>;
    type Writer<'t> = Writer<'t>;

    fn open(path: &Path, tree: Tree) -> Result<Self> {
        if tree != Tree::Normal {
            return Err(unsupported("canopydb", tree));
        }
        // `EnvOptions::new` already has a 1 GiB page cache (canopydb 0.2.5, src/options.rs), the size K3 asks for,
        // so it stays at its default and is not a changed option.
        let env = EnvOptions::new(path);
        // `DbOptions::default` has `use_wal: true`, which durable commits need.
        let db = canopydb::Database::with_options(env, DbOptions::default()).map_err(cause)?;
        Ok(Store { db })
    }

    fn close(self) -> Result<()> {
        // The last handle of the environment waits for its background threads (`wait_bg_threads_on_drop`).
        drop(self.db);
        Ok(())
    }

    fn read<R>(&self, f: impl FnOnce(&Reader<'_>) -> Result<R>) -> Result<R> {
        let txn = self.db.begin_read().map_err(cause)?;
        let tree = txn
            .get_tree(NORMAL_NAME.as_bytes())
            .map_err(cause)?
            .ok_or_else(|| format!("canopydb no tiene el árbol {NORMAL_NAME}"))?;
        f(&Reader { tree })
    }

    fn write<R>(&self, f: impl FnOnce(&mut Writer<'_>) -> Result<R>) -> Result<R> {
        let txn = self.db.begin_write().map_err(cause)?;
        let result = {
            let tree = txn
                .get_or_create_tree(NORMAL_NAME.as_bytes())
                .map_err(cause)?;
            f(&mut Writer { tree })?
        };
        txn.commit_with(true).map_err(cause)?;
        Ok(result)
    }
}

impl super::Reader for Reader<'_> {
    fn get(&self, key: &[u8]) -> Result<Option<u64>> {
        Ok(self
            .tree
            .get(key)
            .map_err(cause)?
            .map(|value| byte_sum(&value)))
    }

    fn scan(&self, start: Option<&[u8]>, limit: u64) -> Result<(u64, u64)> {
        let range = match start {
            Some(start) => self.tree.range(start..),
            None => self.tree.range::<&[u8]>(..),
        }
        .map_err(cause)?;
        let (mut count, mut sum) = (0, 0);
        for entry in range.take(limit.try_into().unwrap_or(usize::MAX)) {
            let (key, value) = entry.map_err(cause)?;
            count += 1;
            sum += byte_sum(&key) + byte_sum(&value);
        }
        Ok((count, sum))
    }

    fn values(&self, _key: &[u8]) -> Result<(u64, u64)> {
        Err(unsupported("canopydb", Tree::Multi))
    }

    fn get_fixed(&self, _key: u64) -> Result<Option<u64>> {
        Err(unsupported("canopydb", Tree::Fixed))
    }
}

impl super::Writer for Writer<'_> {
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        self.tree.insert(key, value).map_err(cause)
    }

    fn remove(&mut self, key: &[u8]) -> Result<()> {
        self.tree.delete(key).map(drop).map_err(cause)
    }

    fn add(&mut self, _key: &[u8], _value: &[u8]) -> Result<()> {
        Err(unsupported("canopydb", Tree::Multi))
    }

    fn put_fixed(&mut self, _key: u64, _value: &[u8]) -> Result<()> {
        Err(unsupported("canopydb", Tree::Fixed))
    }
}
