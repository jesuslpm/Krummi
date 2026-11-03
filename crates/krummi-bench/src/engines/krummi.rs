//! Krummi with its default options (K3, D8). [`krummi::WriteTransaction::commit`] is durable: "it returns once its
//! changes survive a crash of the process or the system and a power cut" (its doc comment, crates/krummi/src/lib.rs).

use std::path::Path;

use krummi::{
    OpenMode, ReadFixedTree, ReadMultiTree, ReadTransaction, ReadTree, StoreOptions,
    WriteFixedTree, WriteMultiTree, WriteTransaction, WriteTree,
};

use super::{
    FIXED_NAME, FIXED_VALUE_LEN, MULTI_NAME, NORMAL_NAME, Result, Tree, byte_sum, cause, wrong_tree,
};

pub struct Store {
    store: krummi::Store,
    tree: Tree,
}

enum ReadHandle<'s> {
    Normal(ReadTree<'s>),
    Multi(ReadMultiTree<'s>),
    Fixed(ReadFixedTree<'s>),
}

pub struct Reader<'t> {
    tx: ReadTransaction<'t>,
    tree: ReadHandle<'t>,
}

enum WriteHandle<'s> {
    Normal(WriteTree<'s>),
    Multi(WriteMultiTree<'s>),
    Fixed(WriteFixedTree<'s>),
}

pub struct Writer<'t> {
    tx: WriteTransaction<'t>,
    tree: WriteHandle<'t>,
}

impl super::Store for Store {
    type Reader<'t> = Reader<'t>;
    type Writer<'t> = Writer<'t>;

    fn open(path: &Path, tree: Tree) -> Result<Self> {
        let store = krummi::Store::open(path, &StoreOptions::default()).map_err(cause)?;
        Ok(Store { store, tree })
    }

    fn close(self) -> Result<()> {
        self.store.close().map_err(cause)
    }

    fn read<R>(&self, f: impl FnOnce(&Reader<'_>) -> Result<R>) -> Result<R> {
        let tx = self.store.read().map_err(cause)?;
        let tree = match self.tree {
            Tree::Normal => ReadHandle::Normal(tx.open_tree(NORMAL_NAME).map_err(cause)?),
            Tree::Multi => ReadHandle::Multi(tx.open_multi_tree(MULTI_NAME).map_err(cause)?),
            Tree::Fixed => ReadHandle::Fixed(
                tx.open_fixed_tree(FIXED_NAME, FIXED_VALUE_LEN)
                    .map_err(cause)?,
            ),
        };
        f(&Reader { tx, tree })
    }

    fn write<R>(&self, f: impl FnOnce(&mut Writer<'_>) -> Result<R>) -> Result<R> {
        let mut tx = self.store.write().map_err(cause)?;
        let mode = OpenMode::OpenOrCreate;
        let tree = match self.tree {
            Tree::Normal => {
                WriteHandle::Normal(tx.open_tree_with(NORMAL_NAME, mode).map_err(cause)?)
            }
            Tree::Multi => {
                WriteHandle::Multi(tx.open_multi_tree_with(MULTI_NAME, mode).map_err(cause)?)
            }
            Tree::Fixed => WriteHandle::Fixed(
                tx.open_fixed_tree_with(FIXED_NAME, FIXED_VALUE_LEN, mode)
                    .map_err(cause)?,
            ),
        };
        let mut writer = Writer { tx, tree };
        let result = f(&mut writer)?;
        writer.tx.commit().map_err(cause)?;
        Ok(result)
    }
}

impl super::Reader for Reader<'_> {
    fn get(&self, key: &[u8]) -> Result<Option<u64>> {
        let ReadHandle::Normal(tree) = &self.tree else {
            return Err(wrong_tree(Tree::Normal));
        };
        Ok(self.tx.get(tree, key).map_err(cause)?.map(byte_sum))
    }

    fn scan(&self, start: Option<&[u8]>, limit: u64) -> Result<(u64, u64)> {
        let ReadHandle::Normal(tree) = &self.tree else {
            return Err(wrong_tree(Tree::Normal));
        };
        let range = match start {
            Some(start) => self.tx.range(tree, start..),
            None => self.tx.range(tree, ..),
        }
        .map_err(cause)?;
        let (mut count, mut sum) = (0, 0);
        for entry in range.take(limit.try_into().unwrap_or(usize::MAX)) {
            let (key, value) = entry.map_err(cause)?;
            count += 1;
            sum += byte_sum(key) + byte_sum(value);
        }
        Ok((count, sum))
    }

    fn values(&self, key: &[u8]) -> Result<(u64, u64)> {
        let ReadHandle::Multi(tree) = &self.tree else {
            return Err(wrong_tree(Tree::Multi));
        };
        let (mut count, mut sum) = (0, 0);
        for value in self.tx.values(tree, key, ..).map_err(cause)? {
            count += 1;
            sum += byte_sum(value.map_err(cause)?);
        }
        Ok((count, sum))
    }

    fn get_fixed(&self, key: u64) -> Result<Option<u64>> {
        let ReadHandle::Fixed(tree) = &self.tree else {
            return Err(wrong_tree(Tree::Fixed));
        };
        Ok(self.tx.get_fixed(tree, key).map_err(cause)?.map(byte_sum))
    }
}

impl super::Writer for Writer<'_> {
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        let WriteHandle::Normal(tree) = &self.tree else {
            return Err(wrong_tree(Tree::Normal));
        };
        self.tx.set(tree, key, value).map_err(cause)
    }

    fn remove(&mut self, key: &[u8]) -> Result<()> {
        let WriteHandle::Normal(tree) = &self.tree else {
            return Err(wrong_tree(Tree::Normal));
        };
        self.tx.remove(tree, key).map(drop).map_err(cause)
    }

    fn add(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        let WriteHandle::Multi(tree) = &self.tree else {
            return Err(wrong_tree(Tree::Multi));
        };
        self.tx.add(tree, key, value).map(drop).map_err(cause)
    }

    fn put_fixed(&mut self, key: u64, value: &[u8]) -> Result<()> {
        let WriteHandle::Fixed(tree) = &self.tree else {
            return Err(wrong_tree(Tree::Fixed));
        };
        self.tx.set_fixed(tree, key, value).map_err(cause)
    }
}
