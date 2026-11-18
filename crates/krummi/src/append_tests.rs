//! The remembered last leaf of a normal tree: `set` and `try_add` add a key greater than every key of the tree at the
//! end of its rightmost leaf without walking the tree, and every other change of the tree forgets that leaf.

use std::collections::BTreeMap;

use super::*;
use crate::tests::long_key;

fn key(i: usize) -> Vec<u8> {
    format!("{i:08}").into_bytes()
}

/// A value of `i`: most stay in their leaf, some are off it.
fn value(i: usize) -> Vec<u8> {
    let len = match i % 97 {
        0 => 5_000,
        n if n % 3 == 0 => 300,
        _ => 20,
    };
    (0..len).map(|j| (i + j) as u8).collect()
}

fn state<'w>(write: &'w mut WriteTransaction<'_>, tree: &WriteTree<'_>) -> &'w mut TreeState {
    write.trees.get_mut().get_mut(tree.id).unwrap()
}

fn entries(write: &WriteTransaction<'_>, tree: &WriteTree<'_>) -> Vec<(Vec<u8>, Vec<u8>)> {
    write
        .range(tree, ..)
        .unwrap()
        .map(|entry| {
            let (key, value) = entry.unwrap();
            (key.to_vec(), value.to_vec())
        })
        .collect()
}

/// An ordered run of `set` and `try_add` adds at the end of the last leaf, walking the tree only when a leaf is full
/// or a value is off its leaf; and it leaves the entries, the root and the count that walking the tree for every
/// operation leaves (the tree's own tests compare every node byte for byte).
#[test]
fn ordered_sets_and_adds_append_at_the_end_of_the_last_leaf() {
    let run = |fast: bool| {
        let store = Store::open_temporary(&StoreOptions::default()).unwrap();
        let mut write = store.write().unwrap();
        let tree = write.open_tree_with("t", OpenMode::CreateNew).unwrap();
        for i in 0..4_000 {
            if !fast {
                state(&mut write, &tree).last = None;
            }
            if i % 2 == 0 {
                write.set(&tree, &key(i), &value(i)).unwrap();
            } else {
                assert!(write.try_add(&tree, &key(i), &value(i)).unwrap());
            }
        }
        let state = state(&mut write, &tree);
        let (appended, root) = (state.appended, state.root);
        (
            appended,
            root,
            write.len(&tree).unwrap(),
            entries(&write, &tree),
        )
    };
    let (appended, root, count, found) = run(true);
    let (walked, same_root, same_count, same_found) = run(false);
    assert!(appended > 3_800, "{appended}");
    assert_eq!(walked, 0);
    assert_eq!((root, count, &found), (same_root, same_count, &same_found));
    let expected: Vec<_> = (0..4_000).map(|i| (key(i), value(i))).collect();
    assert_eq!(found, expected);
}

/// Every change of the tree other than adding at its end forgets the last leaf: a `set` or `try_add` that does not
/// add at the end, `remove`, a range removal and emptying the tree. Adding at the end after each finds the leaf
/// again, and the tree keeps its order.
#[test]
fn every_other_change_forgets_the_last_leaf() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let tree = write.open_tree_with("t", OpenMode::CreateNew).unwrap();
    let mut model = BTreeMap::new();
    let mut next = 0;
    let append = |write: &mut WriteTransaction<'_>,
                  model: &mut BTreeMap<Vec<u8>, Vec<u8>>,
                  next: &mut usize| {
        for _ in 0..50 {
            write.set(&tree, &key(*next), b"v").unwrap();
            model.insert(key(*next), b"v".to_vec());
            *next += 1;
        }
        assert!(
            state(write, &tree).last.is_some(),
            "an append remembers the leaf"
        );
    };
    let forgot = |write: &mut WriteTransaction<'_>, what: &str| {
        assert!(
            state(write, &tree).last.is_none(),
            "{what} forgets the leaf"
        );
    };

    append(&mut write, &mut model, &mut next);
    write.set(&tree, &key(10), b"changed").unwrap();
    model.insert(key(10), b"changed".to_vec());
    forgot(&mut write, "a set of a key that is there");

    append(&mut write, &mut model, &mut next);
    let middle = format!("{:08}.5", 15).into_bytes();
    write.set(&tree, &middle, b"m").unwrap();
    model.insert(middle, b"m".to_vec());
    forgot(&mut write, "a set of a new key in the middle");

    append(&mut write, &mut model, &mut next);
    assert!(!write.try_add(&tree, &key(20), b"x").unwrap());
    forgot(&mut write, "a try_add of a key that is there");

    append(&mut write, &mut model, &mut next);
    let middle = format!("{:08}.5", 30).into_bytes();
    assert!(write.try_add(&tree, &middle, b"m").unwrap());
    model.insert(middle, b"m".to_vec());
    forgot(&mut write, "a try_add in the middle");

    append(&mut write, &mut model, &mut next);
    assert!(write.remove(&tree, &key(next - 1)).unwrap());
    model.remove(&key(next - 1));
    forgot(&mut write, "remove");

    append(&mut write, &mut model, &mut next);
    let from = key(next - 10);
    assert_eq!(write.remove_range(&tree, &from[..]..).unwrap(), 10);
    model.retain(|k, _| *k < from);
    forgot(&mut write, "a range removal");

    append(&mut write, &mut model, &mut next);
    assert_eq!(
        entries(&write, &tree),
        model.clone().into_iter().collect::<Vec<_>>()
    );
    let tree = write.open_tree_with("t", OpenMode::Empty).unwrap();
    assert!(
        state(&mut write, &tree).last.is_none(),
        "emptying forgets the leaf"
    );
    write.set(&tree, &key(0), b"v").unwrap();
    assert_eq!(entries(&write, &tree), vec![(key(0), b"v".to_vec())]);
    write.commit().unwrap();
}

/// A `set` that fails forgets the last leaf: the tree may have changed before the failure.
#[test]
fn a_failed_set_forgets_the_last_leaf() {
    let small = StoreOptions {
        max_size: Some(krummi_pager::MIN_MAX_SIZE),
    };
    let store = Store::open_temporary(&small).unwrap();
    let mut write = store.write().unwrap();
    let tree = write.open_tree_with("t", OpenMode::CreateNew).unwrap();
    let mut remembered = false;
    let error = (0..2_000)
        .try_for_each(|i| {
            let set = write.set(&tree, &long_key(i), b"g");
            remembered |= state(&mut write, &tree).last.is_some();
            set
        })
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::MaxSize, "{error}");
    assert!(remembered, "the ordered sets remembered the leaf");
    assert!(state(&mut write, &tree).last.is_none());
}
