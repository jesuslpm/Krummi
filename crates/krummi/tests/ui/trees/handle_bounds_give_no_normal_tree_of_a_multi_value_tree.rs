// AC-M9 (S9), M4: the bounds shared by both kinds of handle, `ReadHandle` and `WriteHandle`, give no way to reach the
// normal tree inside a multi-value handle, so normal operations cannot reach a multi-value tree through them
// (Codex code review R1). Their method needs a token that only the engine can build: calling it without one or
// with one built here does not compile.
use krummi::{OpenMode, ReadHandle, ReadTree, Store, StoreOptions, WriteHandle, WriteTree};

fn normal_write<'a, 's>(tree: &'a impl WriteHandle) -> &'a WriteTree<'s>
where
    'a: 's,
{
    tree.as_tree()
}

fn normal_read<'a, 's>(tree: &'a impl ReadHandle) -> &'a ReadTree<'s>
where
    'a: 's,
{
    tree.as_tree(krummi::sealed::Token(()))
}

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let tree = write.open_multi_tree_with("m", OpenMode::OpenOrCreate).unwrap();
    let _ = write.set(normal_write(&tree), b"k", b"v");
    write.commit().unwrap();

    let read = store.read().unwrap();
    let tree = read.open_multi_tree("m").unwrap();
    let _ = read.get(normal_read(&tree), b"k");
}
