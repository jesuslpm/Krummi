//! The filter of a nearest-neighbour search (VI7 of the vector indexes PRD): conditions on the table's columns, joined
//! with AND, that a row must meet to be a result.
//!
//! A filter's values have the type and the domain of their column, as an estimate's (ST2), and compare after the
//! normalization of N4 and the collation of their column, as in an index's key (N5): each value, and each row's value,
//! is encoded as the element of its column (`element_column`), whose bytes order and equal as the values do, so a NaN
//! in a list finds the rows with a NaN. Nulls follow SQL, unlike ST2: a row whose column is null never meets a
//! condition on it; an equality with a null, a range with a null bound, and a NOT IN whose list has a null let no row
//! pass; a null in an IN list passes nothing by itself.
//!
//! The filter is evaluated first, before any distance (VI6): when a secondary index leads with a column that an
//! equality, an IN list or a range constrains, its intervals give the candidate rows; otherwise every row of the table
//! is read. Either way each candidate row is checked against the whole filter.

use std::ops::Bound;

use krummi_pager::{Error, Result, ValueRule};
use krummi_tree::PageSource;

use super::index::EntryLayout;
use super::index_key::{index_range, split_entry};
use super::key::{KeyColumn, KeyRange};
use super::ranges::interval;
use super::schema::{SchemaColumn, TableSchema};
use super::statistics::stats_column;
use super::stats::element_column;
use super::value::{Value, normalize};
use crate::Folder;

/// A condition of the filter of a nearest-neighbour search (VI7): on a column that is neither a vector nor JSON.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterCondition<'a> {
    /// The column equals the value; a null lets no row pass.
    Equal { column: &'a str, value: Value<'a> },
    /// The column is within the bounds, in the order of N5 of its type; a null bound lets no row pass.
    Range {
        column: &'a str,
        lower: Bound<Value<'a>>,
        upper: Bound<Value<'a>>,
    },
    /// Each column equals its value, as many equalities: 1 or more distinct columns.
    EqualColumns(Vec<(&'a str, Value<'a>)>),
    /// The column is one of the values, a nonempty list (IN); a null in it passes nothing by itself.
    In {
        column: &'a str,
        values: Vec<Value<'a>>,
    },
    /// The column is none of the values, a nonempty list (NOT IN); a list with a null lets no row pass.
    NotIn {
        column: &'a str,
        values: Vec<Value<'a>>,
    },
}

/// A condition checked against its column: its values as the column's elements.
#[derive(Debug)]
enum Test {
    Equal(Vec<u8>),
    Range(Bound<Vec<u8>>, Bound<Vec<u8>>),
    /// Sorted and without repeats.
    In(Vec<Vec<u8>>),
    NotIn(Vec<Vec<u8>>),
}

/// A condition on one column, checked.
#[derive(Debug)]
struct Predicate {
    /// The column's identity and its position among the live columns.
    id: u64,
    place: usize,
    element: KeyColumn,
    test: Test,
    /// The values given, normalized, for the interval of an index (the In list without its nulls).
    values: Vec<Value<'static>>,
    bounds: Option<(Bound<Value<'static>>, Bound<Value<'static>>)>,
}

/// A filter checked against a table's schema (VI7).
#[derive(Debug, Default)]
pub(crate) struct Filter {
    predicates: Vec<Predicate>,
    /// Whether a condition lets no row pass whatever the rows hold (a null where SQL makes it unknown).
    none: bool,
}

/// Where a filter's candidate rows come from.
pub(crate) enum Candidates {
    /// No row can pass.
    None,
    /// Every row of the table.
    Table,
    /// The rows of the entries of these intervals of the index at `at` among the table's indexes, whose key columns
    /// are `key`.
    Index {
        at: usize,
        key: Vec<KeyColumn>,
        ranges: Vec<KeyRange>,
    },
}

impl Filter {
    /// Checks `conditions` on the table `table` of `schema` (VI7, *Validaciones*): the sizes of the column names, then
    /// a column the table does not have (a different schema), then a vector or JSON column (an invalid schema), then an
    /// empty list or a repeated column of an equality on several columns, then each value, all in the order given.
    pub(crate) fn check(
        folder: Folder<'_>,
        table: &str,
        schema: &TableSchema,
        conditions: &[FilterCondition<'_>],
    ) -> Result<Filter> {
        for condition in conditions {
            for name in condition.columns() {
                folder.check_column_name(name)?;
            }
        }
        let mut columns: Vec<Vec<&SchemaColumn>> = Vec::with_capacity(conditions.len());
        for condition in conditions {
            let found = (condition.columns().into_iter())
                .map(|name| stats_column(folder, table, schema, name))
                .collect::<Result<Vec<_>>>()?;
            columns.push(found);
        }
        let invalid = |column: Option<&str>, rule| Error::InvalidValue {
            json: None,
            folder: folder.owned(),
            table: Some(table.to_owned()),
            column: column.map(str::to_owned),
            sequence: None,
            rule,
        };
        for condition in conditions {
            match condition {
                FilterCondition::In { values, column }
                | FilterCondition::NotIn { values, column }
                    if values.is_empty() =>
                {
                    return Err(invalid(Some(column), ValueRule::InvalidRange));
                }
                FilterCondition::EqualColumns(pairs) if pairs.is_empty() => {
                    return Err(invalid(None, ValueRule::InvalidRange));
                }
                FilterCondition::EqualColumns(pairs) => {
                    for (at, (name, _)) in pairs.iter().enumerate() {
                        if pairs[..at].iter().any(|(other, _)| other == name) {
                            return Err(invalid(Some(name), ValueRule::RepeatedColumn));
                        }
                    }
                }
                _ => {}
            }
        }
        let mut filter = Filter::default();
        for (condition, columns) in conditions.iter().zip(columns) {
            let place =
                |column: &SchemaColumn| schema.column_position(column.id).expect("a live column");
            let normal = |column: &SchemaColumn, value: &Value<'_>| {
                normalize(&column.ty, false, value.clone().into_owned())
                    .map_err(|rule| invalid(Some(&column.name), rule))
            };
            let encode = |column: &SchemaColumn, value: &Value<'_>| -> Result<Vec<u8>> {
                let mut element = Vec::new();
                element_column(column)
                    .encode(value, &mut element)
                    .map_err(|rule| invalid(Some(&column.name), rule))?;
                Ok(element)
            };
            let mut push = |column: &SchemaColumn, test, values, bounds| {
                filter.predicates.push(Predicate {
                    id: column.id,
                    place: place(column),
                    element: element_column(column),
                    test,
                    values,
                    bounds,
                })
            };
            match condition {
                FilterCondition::Equal { value, .. } => {
                    let column = columns[0];
                    if matches!(value, Value::Null) {
                        filter.none = true;
                        continue;
                    }
                    let value = normal(column, value)?;
                    push(
                        column,
                        Test::Equal(encode(column, &value)?),
                        vec![value],
                        None,
                    );
                }
                FilterCondition::EqualColumns(pairs) => {
                    for (column, (_, value)) in columns.into_iter().zip(pairs) {
                        if matches!(value, Value::Null) {
                            filter.none = true;
                            continue;
                        }
                        let value = normal(column, value)?;
                        push(
                            column,
                            Test::Equal(encode(column, &value)?),
                            vec![value],
                            None,
                        );
                    }
                }
                FilterCondition::Range { lower, upper, .. } => {
                    let column = columns[0];
                    let mut null_bound = false;
                    let mut bound = |bound: &Bound<Value<'_>>| -> Result<Bound<Value<'static>>> {
                        Ok(match bound {
                            Bound::Unbounded => Bound::Unbounded,
                            Bound::Included(Value::Null) | Bound::Excluded(Value::Null) => {
                                null_bound = true;
                                Bound::Unbounded
                            }
                            Bound::Included(value) => Bound::Included(normal(column, value)?),
                            Bound::Excluded(value) => Bound::Excluded(normal(column, value)?),
                        })
                    };
                    let (lower, upper) = (bound(lower)?, bound(upper)?);
                    if null_bound {
                        filter.none = true;
                        continue;
                    }
                    let element = |bound: &Bound<Value<'static>>| -> Result<Bound<Vec<u8>>> {
                        Ok(match bound {
                            Bound::Unbounded => Bound::Unbounded,
                            Bound::Included(value) => Bound::Included(encode(column, value)?),
                            Bound::Excluded(value) => Bound::Excluded(encode(column, value)?),
                        })
                    };
                    let test = Test::Range(element(&lower)?, element(&upper)?);
                    push(column, test, Vec::new(), Some((lower, upper)));
                }
                FilterCondition::In { values, .. } | FilterCondition::NotIn { values, .. } => {
                    let column = columns[0];
                    let not = matches!(condition, FilterCondition::NotIn { .. });
                    let mut kept: Vec<(Vec<u8>, Value<'static>)> = Vec::with_capacity(values.len());
                    let mut null = false;
                    for value in values {
                        if matches!(value, Value::Null) {
                            null = true;
                            continue;
                        }
                        let value = normal(column, value)?;
                        kept.push((encode(column, &value)?, value));
                    }
                    kept.sort_by(|a, b| a.0.cmp(&b.0));
                    kept.dedup_by(|a, b| a.0 == b.0);
                    if (not && null) || (!not && kept.is_empty()) {
                        filter.none = true;
                        continue;
                    }
                    let (elements, values): (Vec<_>, Vec<_>) = kept.into_iter().unzip();
                    let test = if not {
                        Test::NotIn(elements)
                    } else {
                        Test::In(elements)
                    };
                    push(column, test, if not { Vec::new() } else { values }, None);
                }
            }
        }
        Ok(filter)
    }

    /// Whether the row whose live column at place `i` has `value(i)` meets the filter. `scratch` is a buffer it
    /// reuses.
    pub(crate) fn meets<'v>(
        &self,
        value: impl Fn(usize) -> Value<'v>,
        scratch: &mut Vec<u8>,
    ) -> bool {
        if self.none {
            return false;
        }
        self.predicates.iter().all(|predicate| {
            let value = value(predicate.place);
            if matches!(value, Value::Null) {
                return false;
            }
            scratch.clear();
            if predicate.element.encode(&value, scratch).is_err() {
                // A stored value its column cannot encode is a malformed row: it meets nothing (L2).
                return false;
            }
            let element = scratch.as_slice();
            match &predicate.test {
                Test::Equal(expected) => element == expected.as_slice(),
                Test::Range(lower, upper) => {
                    let above = match lower {
                        Bound::Unbounded => true,
                        Bound::Included(b) => element >= b.as_slice(),
                        Bound::Excluded(b) => element > b.as_slice(),
                    };
                    let below = match upper {
                        Bound::Unbounded => true,
                        Bound::Included(b) => element <= b.as_slice(),
                        Bound::Excluded(b) => element < b.as_slice(),
                    };
                    above && below
                }
                Test::In(list) => list.binary_search_by(|e| e.as_slice().cmp(element)).is_ok(),
                Test::NotIn(list) => list
                    .binary_search_by(|e| e.as_slice().cmp(element))
                    .is_err(),
            }
        })
    }

    /// Where the candidate rows come from: none when a condition lets no row pass; the intervals of the first index, in
    /// the order of the names, whose first key column has an equality or an IN list, or else a range; or the table.
    pub(crate) fn candidates(&self, schema: &TableSchema) -> Candidates {
        if self.none {
            return Candidates::None;
        }
        for ranges_only in [false, true] {
            for (at, index) in schema.indexes().iter().enumerate() {
                let first = index.key[0].id;
                let Some(predicate) = self.predicates.iter().find(|p| {
                    p.id == first
                        && match p.test {
                            Test::Equal(_) | Test::In(_) => !ranges_only,
                            Test::Range(..) => ranges_only,
                            Test::NotIn(_) => false,
                        }
                }) else {
                    continue;
                };
                let key = EntryLayout::key_columns(schema, index);
                let ranges: Option<Vec<KeyRange>> = match &predicate.bounds {
                    Some((lower, upper)) => index_range(&key, &[], lower.as_ref(), upper.as_ref())
                        .ok()
                        .map(|r| vec![r]),
                    None => (predicate.values.iter())
                        .map(|value| {
                            index_range(
                                &key,
                                std::slice::from_ref(value),
                                Bound::Unbounded,
                                Bound::Unbounded,
                            )
                            .ok()
                        })
                        .collect(),
                };
                // A value too large for an interval of the index leaves the table to be read.
                if let Some(ranges) = ranges {
                    return Candidates::Index { at, key, ranges };
                }
            }
        }
        Candidates::Table
    }
}

impl FilterCondition<'_> {
    /// The column names it gives, in order.
    fn columns(&self) -> Vec<&str> {
        match self {
            FilterCondition::Equal { column, .. }
            | FilterCondition::Range { column, .. }
            | FilterCondition::In { column, .. }
            | FilterCondition::NotIn { column, .. } => vec![column],
            FilterCondition::EqualColumns(pairs) => pairs.iter().map(|(name, _)| *name).collect(),
        }
    }
}

/// Calls `visit` with the primary key of each entry of `ranges` in the index tree at `root`, whose key columns are
/// `key`, in the order of the ranges and of the index; stops when `visit` returns false. An entry that does not split
/// into the index's elements and a primary key is damage of the page that holds it (IX12).
pub(crate) fn index_keys<'t, S: PageSource + ?Sized>(
    tx: &'t S,
    root: u64,
    key: &[KeyColumn],
    ranges: &[KeyRange],
    mut visit: impl FnMut(&'t [u8]) -> Result<bool>,
) -> Result<()> {
    for range in ranges {
        for entry in krummi_tree::Range::new(tx, root, interval(range))? {
            let (entry, _) = entry?;
            let Some((_, primary)) = split_entry(key, entry) else {
                return Err(match krummi_tree::leaf_of(tx, root, entry) {
                    Ok(page) => tx.damaged(page),
                    Err(error) => error,
                });
            };
            if !visit(primary)? {
                return Ok(());
            }
        }
    }
    Ok(())
}
