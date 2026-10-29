//! Multi-value trees where the public tests cannot see without the engine's test hooks: where the keys and the values
//! that the scans return point (AC-M16).

use krummi_tx::Memory;

use super::*;

/// AC-M16 (M7, AC20, T16): the keys of a key scan and the values of a value scan, of a key with its values in its
/// entry and of one with its values in a subtree, are read where the engine keeps them, never a copy: once
/// transferred, `krummi.data` or the memory of a temporary store; and the same memory comes back on each scan. That
/// a reference cannot outlive its transaction is in `tests/ui/trees`.
#[test]
fn ac_m16_scans_of_a_multi_value_tree_return_the_engine_s_memory() {
    let tmp = tempfile::tempdir().unwrap();
    for store in [
        Store::open(tmp.path(), &StoreOptions::default()).unwrap(),
        Store::open_temporary(&StoreOptions::default()).unwrap(),
    ] {
        let mut write = store.write().unwrap();
        let tree = write
            .open_multi_tree_with("m", OpenMode::CreateNew)
            .unwrap();
        for value in [&b"a"[..], b"b", b"c"] {
            write.add(&tree, b"inline", value).unwrap();
        }
        for i in 0..150 {
            write
                .add(&tree, b"subtree", format!("{i:016}").as_bytes())
                .unwrap();
        }
        assert!(write.subtree_of(&tree, b"inline").unwrap().is_none());
        assert!(write.subtree_of(&tree, b"subtree").unwrap().is_some());
        write.commit().unwrap();
        assert!(store.engine.transfer_now());

        let read = store.read().unwrap();
        let tree = read.open_multi_tree("m").unwrap();
        let keys: Vec<&[u8]> = read
            .keys(&tree, ..)
            .unwrap()
            .map(|item| item.map(|(key, _)| key))
            .collect::<Result<_>>()
            .unwrap();
        assert_eq!(keys, [&b"inline"[..], b"subtree"]);
        let again = read.keys(&tree, ..).unwrap().next().unwrap().unwrap().0;
        assert_eq!(keys[0].as_ptr(), again.as_ptr(), "no copy");
        for key in keys {
            assert_eq!(store.engine.memory_of(key), Some(Memory::Data), "{key:?}");
            let values: Vec<&[u8]> = read
                .values(&tree, key, ..)
                .unwrap()
                .collect::<Result<_>>()
                .unwrap();
            assert!(values.len() >= 3);
            for value in &values {
                assert_eq!(
                    store.engine.memory_of(value),
                    Some(Memory::Data),
                    "{value:?}"
                );
            }
            let last = read.values(&tree, key, ..).unwrap().next_back();
            let last = last.unwrap().unwrap();
            assert_eq!(last.as_ptr(), values.last().unwrap().as_ptr(), "no copy");
        }
        drop(read);
        store.close().unwrap();
    }
}
