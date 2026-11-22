# Temporal Values in Microseconds — PRD

**Version**: 1.0
**Date**: 2026-11-22
**Parent document**: [Krummi Tables with Schema — PRD](tables-schema.prd.en.md) (v1.0)

> English translation of [tables-temporal-micros.prd.md](tables-temporal-micros.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Make the application give and read the `time`, `timestamp` and `duration` values in the same unit in which Krummi
stores them: **microseconds**. Today it gives and reads them in nanoseconds, but Krummi stores them rounded to the
microsecond, so a value that is read is always a multiple of 1,000 ns, and the published limits of the domains are in
microseconds, a different unit from that of the value.

**Why**: the two units confuse and add nothing. The only reason for the nanosecond was that E8 could round a smaller
fraction to the microsecond; with the microsecond as the unit of the value there is no fraction to round. The user: «No
entiendo que los tipos de datos la unidad del valor de time, timespan y duration sean nanosegundos y lo almacenado sea
microsegundos. Esto despista mucho y no se explica.» (I do not understand why the unit of the value of the data types
time, timespan and duration is nanoseconds and what is stored is microseconds. This is very confusing and is not
explained.) and «La razón me parece absurda, uso nanosegundos para poder redondear a microsegundos y así cumplir la
regla, si son todos microsegundos la regla de redondear a microsegundos consiste en no hacer nada.» (The reason seems
absurd to me: I use nanoseconds in order to round to microseconds and thus meet the rule; if everything is
microseconds, the rule of rounding to microseconds consists of doing nothing.)

This PRD changes E8 of its parent (the rounding of the values given) and makes precise N4 of the tables, IX9 of the
indexes, ST2 of the statistics and JS7 of the JSON columns (the unit of the values); all of them carry its annotation
(*Annotations to other PRDs*). What is stored does not change.

### Product decisions

| ID | Decision |
|---|---|
| TU1 | **A single unit**: the value of a `time` is the number of microseconds since midnight; that of a `timestamp`, the number of microseconds since 1970-01-01T00:00:00Z, in UTC; that of a `duration`, a signed number of microseconds. It is the unit in which Krummi stores it, and it holds in every place through which a temporal value enters or leaves Krummi: the values of a row when inserting, updating, inserting or updating, bulk loading and reading it; the default values of a column; the limits of the ranges of the primary key (E12) and of the indexes (IX9); the keys of the indexes and their entries; the typed reads of a row and of a projection; the values of the statistics (the most frequent ones of a column and of the prefixes of an index, and the limits of the histogram); and the conditions of an estimation (ST2). The published limits of the domains of E8 keep their value, now in microseconds, and their type. |
| TU2 | **Signed 64-bit integers** for the three types, in every place of TU1: the domains of E8 fit in them (the largest, that of `duration`, reaches 922,337,203,685,477,580). `time` already was one; `timestamp` and `duration` stop being 128-bit integers. They are integers without a type of their own, like the days of `date`: the unit is stated by the documentation and the guides (user, between an integer and a type of its own with a constructor that forces naming the unit: «Entero i64 sin más» (a plain i64 integer)). |
| TU3 | **No rounding of the values given** (changes E8 of the parent): a value given is already in microseconds, so it is stored as it is. The domain of E8 is checked on the value given, with the error classes of today in each place (*Validations and messages*). The domains and the rounding of `decimal` do not change. An update with the same stored value remains identical (E10, E13). |
| TU4 | **The instant read from a JSON** (makes precise JS7): reading an instant from a JSON text keeps converting it to UTC, rounding its fraction of a second to the microsecond with ties toward the greater value and checking the domain of E8 after converting and rounding, as today; it returns microseconds (`"2026-11-11T10:23:00.1234567Z"` gives 1,794,392,580,123,457). It is the only temporal rounding that remains: that of a text with more than six fraction digits. A time or a duration stored in a JSON, which the application reads with the nearest type and converts (*Out of scope* of JS7), are also given to Krummi in microseconds. |
| TU5 | **What is stored does not change**: the rows, the keys, the indexes, the statistics and the default values stored are already in microseconds, so their bytes are the same, the format version does not change (D7 of the root) and a store written before this change is opened and read the same, with its values in microseconds. |
| TU6 | **The text form does not change** (that of the data of the errors, such as the key of **duplicate key**, N10 of the tables, and E17): a time is `HH:MM:SS` followed, if it has a fraction, by a point and its fraction digits without the trailing zeros (`09:00:00.5`, `23:59:59.999999`); an instant, `YYYY-MM-DDTHH:MM:SS` with its fraction the same way and a `Z` (`1970-01-01T00:00:00Z`, `2026-11-11T10:23:00.123457Z`); a duration, its sign if it is negative, its whole seconds, its fraction the same way and an `s` (`-1.5s`, `0.000001s`). |
| TU7 | **Measurements** (N11 of the tables and its annotation of 2026-11-20): the affected loads are those of tables, W23–W32, because all of them write or read the `timestamp` column of their rows. The loads give the same data as before the change; their times are what is measured. |
| TU8 | **Documentation**: the usage guides and the API documentation explain a single unit, the microsecond, and stop justifying the nanosecond. |

### Starting state

- The `time`, `timestamp` and `duration` values that the application gives and reads are nanoseconds: `time` in a
  64-bit integer, `timestamp` and `duration` in 128-bit integers. The typed reads of a row and of a projection return
  nanoseconds.
- Krummi stores them in microseconds, in 64-bit integers, in the rows, the default values, the keys of the primary key
  and of the indexes and the statistics; when writing them it rounds to the nearest microsecond (E8) and when reading
  them it multiplies by 1,000.
- The published limits of the domains of E8 are in microseconds, in 64-bit integers.
- Reading an instant from a JSON ([tables-json.prd.en.md](tables-json.prd.en.md), JS7) returns nanoseconds.
- The text form of the temporal values is that of TU6.
- The table loads of the benchmark (W23–W32) write and read a `timestamp` column, and W32 traverses an index on it.
- The usage guides explain both units today.

## Actors

- **Application**: gives and reads temporal values in microseconds.
- **Krummi**: checks them against their domain and stores them as they are.

## Flow

1. The application gives a `time`, a `timestamp` or a `duration` in microseconds: in a row, a default value, a limit of
   a range or a condition of an estimation.
2. Krummi checks the domain of E8 on that value and stores it without changing it.
3. The application reads it in microseconds: the same value it gave.

What was stored before is read in microseconds: the same instant, in another unit. An application that moves from
nanoseconds to microseconds converts its values like this:

- **A value read from Krummi before this change** is a multiple of 1,000 ns, so dividing it by 1,000 is exact and gives
  the stored value.
- **An original value of the application**, the one it gave in nanoseconds, is converted as Krummi stored it: to the
  nearest microsecond, with ties toward the greater value. Dividing it by 1,000 by truncation can give one microsecond
  less than the stored one: a `timestamp` given as 1,500 ns was stored as 2 µs, and 1,500 divided by 1,000 by
  truncation gives 1 µs. A default value converted this way is different from the stored one and, when opening with
  that schema the table that already exists, gives different schema (E6).

A value that the application keeps giving in nanoseconds is interpreted as microseconds: a current instant falls
outside the domain of `timestamp` and fails; a time greater than 86,399,999,999 does too; a smaller time or a duration
inside the domain do not fail and are stored a thousand times greater. A default value that stays in nanoseconds fails
with invalid schema if it falls outside its domain (E5); inside it, when opening with that schema a table that already
exists, it gives different schema (E6) except when it is the same in both units, such as 0.

## Validations and messages

| Situation | Rule | Message |
|---|---|---|
| A `time`, `timestamp` or `duration` inside its domain of E8 | It is stored or used as it is (TU3) | None |
| One outside its domain, in the value of a row | E8, E17 | Invalid value, rule outside the domain, as today |
| One outside its domain, in the default value of a column | E5 | Invalid schema, as today |
| One outside its domain, in a limit of a range of the primary key or of an index | E12, IX9 | Invalid value, as today |
| One outside its domain, in a condition of an estimation | ST2 | Invalid value, as today |
| A value of another type in a temporal column | N4 | Invalid value, rule type different from that of the column, as today |
| A JSON instant with more than six fraction digits | TU4 | None: it is rounded to the microsecond, as today |
| A JSON instant outside the domain after converting and rounding | JS7 | None: the read gives null, as today |
| The other types (`date`, `decimal` and the rest) | No changes | Those of today |

The ends of the range of the 64-bit integers are values like the others: outside each domain, they fail as their place
indicates. There are no new messages and no existing message changes.

## Output

- The typed reads of `time`, `timestamp` and `duration` return signed 64-bit integers in microseconds: the time
  09:00:00.5 is read as 32,400,500,000; the instant 1970-01-01T00:00:01Z, 1,000,000; the duration of −1.5 s,
  −1,500,000.
- Reading an instant from a JSON returns a signed 64-bit integer in microseconds (TU4).
- The text form does not change (TU6).
- The other values and outputs keep their form.

## Reuse

- The domains of E8, their check and the error classes of each place.
- The stored format of 7.1, which is already in microseconds (TU5).
- The reading of an instant from a JSON text of JS7, with its rounding to the microsecond (TU4).
- The measurements of N11 with its annotation of 2026-11-20.

## Out of scope

- Precision below the microsecond, in the value or in what is stored.
- Changing the unit of `date`, the domains of E8 or the published limits of the domains.
- Conversions from or to the time types of the Rust standard library or of other libraries.
- Stored time zones (N4).
- Any change of the stored format (TU5), of the computation of the statistics or of the estimations.
- The design documents of the tables, which follow their own convention.

## Left to the implementation

- Nothing observable: the rules of this PRD fix the unit, the type, the rounding, the errors, what is stored and the
  text form.

## Acceptance criteria

1. **AC-TU1** — Given a table with `time`, `timestamp` and `duration` columns and an index on each, when values in
   microseconds are written at the ends of each domain of E8 and at an intermediate point, by inserting, updating,
   inserting or updating and with a bulk load, and are read by the key, by a range of the primary key, by each index,
   with the typed reads of the row and with a projection, then each read gives exactly the value given; and a value one
   unit beyond each end, and the ends of the range of the 64-bit integers, fail with invalid value and the rule outside
   the domain, without effect.
2. **AC-TU2** — Given the values of AC-TU1, when they are used as the default value of an added column, then a row
   written before adding it reads it in microseconds, and one outside its domain fails with invalid schema; when they
   are used as limits of a range of the primary key and of an index, then they give the rows of those microseconds, and
   outside their domain they fail with invalid value; when they are used as conditions of an estimation on a table that
   is read whole, then the estimation and the statistics of the column and of the prefixes of the index are the same as
   before the change on the same data, with their values in microseconds.
3. **AC-TU3** — Given the places of TU1, when an application gives and reads a `time`, a `timestamp` and a `duration`
   (the values of a row, the typed reads of a row and of a projection, the entries of an index, the values of the
   statistics and the instant read from a JSON), then they are signed 64-bit integers (TU2).
4. **AC-TU4** — Given the bytes of a row with temporal values, of its primary key, of the entry of an index on a
   temporal column, of a temporal default value and of the statistics of that column, stored by Krummi before this
   change, when the same instants are written in microseconds after the change, then the stored bytes are the same and
   the format version is the same; and a store written before this change is opened and gives its values in
   microseconds (TU5).
5. **AC-TU5** — Given a JSON document, when `"2026-11-11T10:23:00.1234567Z"`, `"2026-11-11T00:30:00+01:00"`,
   `"1969-12-31T23:59:59.9999995Z"` and `"0001-01-01T00:59:59.9999996+01:00"` are read as instants, then they give
   1,794,392,580,123,457, 1,794,353,400,000,000, 0 and −62,135,596,800,000,000 microseconds (the last one, the first
   instant of the domain, to which the rounding takes it); `"0001-01-01T00:59:59.9999994+01:00"` and
   `"9999-12-31T23:59:59.9999995Z"`, which after converting and rounding fall outside the domain, give null; and the
   cases that give null today keep giving null (TU4).
6. **AC-TU6** — Given the time 09:00:00.5, the time 23:59:59.999999, the instant 0, the instant
   2026-11-11T10:23:00.123457Z and the durations of −1.5 s and of 1 µs, in microseconds, when they are requested in
   their text form, then they give `09:00:00.5`, `23:59:59.999999`, `1970-01-01T00:00:00Z`,
   `2026-11-11T10:23:00.123457Z`, `-1.5s` and `0.000001s`; and when a row with each value is inserted twice in a table
   whose primary key is a column of the same type (`time` for the times, `timestamp` for the instants, `duration` for
   the durations), then the **duplicate key** shows the key with that same text in parentheses (`(09:00:00.5)`) (TU6).
7. **AC-TU7** — Given the closing of this PRD, when its report is reviewed, then it includes the measurements of N11
   with its annotation of 2026-11-20 on Windows and on Linux, with W23–W32 as the affected loads, and the checks of the
   data of the benchmark loads (K4) give the same as before the change (TU7).
8. **AC-TU8** — Given the usage guides, when they are read, then they explain a single unit, the microsecond, and their
   examples pass (TU8).

## Annotations to other PRDs

This PRD changes or makes precise rules of earlier documents, which carry the annotations of 2026-11-22 (in their
translations too):

- Tables, N4: the values of `time`, `timestamp` and `duration` that the application gives and reads are in
  microseconds, the resolution with which they are stored, in signed 64-bit integers (TU1, TU2).
- Tables with schema, E8: a temporal value given is already in microseconds and is stored without rounding; the phrase
  on the rounding of an instant, a time or a duration with a fraction below the microsecond remains only for the
  instant read from a JSON text (TU3, TU4), and the note on the time of −500 ns no longer applies. AC6: its temporal
  values below the microsecond (0001-01-01T00:00:00.0000005Z and 9999-12-31T23:59:59.9999995Z) can no longer be given;
  the `decimal` part does not change.
- Secondary indexes, IX9, and statistics, ST2: a temporal value of a limit or of a condition is already in
  microseconds and is not rounded (TU3).
- JSON columns, JS7: the instant read is returned in microseconds; a time or a duration converted by the application is
  given in microseconds (TU4).

## References

- Parent: [Krummi Tables with Schema — PRD](tables-schema.prd.en.md) (7.1).
- Ancestry: [Krummi Tables — PRD](tables.prd.en.md) and the root,
  [Krummi, an embedded storage engine inspired by Voron — PRD](../krummi.prd.en.md).
- Related: [JSON Columns of the Tables — PRD](tables-json.prd.en.md) (JS7),
  [Secondary Indexes of Tables — PRD](tables-indexes.prd.en.md) (IX9),
  [Table Statistics — PRD](tables-statistics.prd.en.md) (ST2).
