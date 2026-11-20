//! Unit tests of the statistics memory (ST8): what a commit retires.

use super::{Retired, StatsMemory};

/// RDD follow-up (ST8): the columns and indexes removed from a table are forgotten once the table is deleted, whose
/// floor already keeps every statistic of it out; those of the tables still in the store stay.
#[test]
fn deleting_a_table_prunes_its_removed_targets() {
    let memory = StatsMemory::default();
    let removed = |table, id| Retired {
        columns: vec![(table, id)],
        indexes: vec![(table, id + 1)],
        ..Retired::default()
    };
    memory.retire(&removed(1, 10), 5);
    memory.retire(&removed(2, 10), 6);
    assert_eq!(memory.state().removed.len(), 4);
    let deleted = Retired {
        tables: vec![(1, u64::MAX)],
        ..Retired::default()
    };
    memory.retire(&deleted, 7);
    let state = memory.state();
    assert_eq!(state.removed.len(), 2);
    assert!(state.removed.iter().all(|(table, _)| *table == 2));
    assert_eq!(state.floors.get(&1), Some(&u64::MAX));
}

/// RDD follow-up (ST8): emptying a table keeps its removed targets, since its floor does not cover a build of a later
/// snapshot that still had them.
#[test]
fn emptying_a_table_keeps_its_removed_targets() {
    let memory = StatsMemory::default();
    let removed = Retired {
        columns: vec![(1, 10)],
        ..Retired::default()
    };
    memory.retire(&removed, 5);
    let emptied = Retired {
        tables: vec![(1, 3)],
        ..Retired::default()
    };
    memory.retire(&emptied, 6);
    assert!(memory.state().removed.contains(&(1, 10)));
}
