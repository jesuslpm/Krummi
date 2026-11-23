# Collation of the Text Columns — PRD

**Version**: 1.0
**Date**: 2026-11-23
**Parent document**: [Krummi Tables — PRD](tables.prd.en.md) (v1.0)

> English translation of [tables-collation.prd.md](tables-collation.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Build child PRD **7.7** of the [split of the tables module](tables.prd.md#reparto-en-prd-hijos): a `string` column
declares its **collation**, which decides when two texts are the same value. With `binary`, today's collation and the
default, two texts are equal only if their bytes are. With `ci`, equality does not distinguish uppercase from
lowercase. With `ci_ai`, it does not distinguish accents either. The collation of the column governs the primary key,
the secondary indexes, uniqueness, ranges and statistics.

**Why**: today the primary key, the indexes and the statistics compare text byte by byte, with no collation (N5).
Searching for `josé` does not find `José`, a unique index admits both `Ana` and `ANA`, and the same text written with
the `é` as one character or as an `e` with the combining accent are two different values. In a database that is of
little use. The design planned to solve it with indexes on expressions (design document 3), but then the application
would have to repeat the expression in every query. With the collation as a property of the column, the rule lives in
the schema and no query can forget it. User, on 2026-11-21: «Me vale con la collation como propiedad de la columna
(binary por defecto, ci y ci_ai). Me vale con que la igualdad, los prefijos y la unicidad no distingan mayúsculas ni
acentos» (the collation as a property of the column is fine with me (binary by default, ci and ci_ai); it is fine
with me that equality, prefixes and uniqueness do not distinguish uppercase or accents).

**Why now**: 7.1, 7.2, 7.3 and 7.5 are closed, and the collation has to exist before the queries (module 17) fix how
they compare text.

The alphabetical order of a language is out of scope: the texts of a `ci` or `ci_ai` column are ordered by the bytes of
their folded form (IC2, IC3), not like a dictionary. That order leaves contiguous all the texts whose folded form
starts with the folded form of the same text, and the text prefix search of module 17 will rely on that: in a `ci`
column, those that start with `jos` are those that start with `JOS`, `Jos` or `jOs`. In this document, "prefix" on its
own is the one of E12 and IX9, the first columns of a key; that of a text is always called **text prefix**.

This PRD inherits everything from its ancestors. It changes N5 and *Out of scope* of its parent and the collation
decision of design document 3, and makes precise rules of [tables-schema.prd.md](tables-schema.prd.md) (7.1),
[tables-indexes.prd.md](tables-indexes.prd.md) (7.2), [tables-statistics.prd.md](tables-statistics.prd.md) (7.3) and
[tables-json.prd.md](tables-json.prd.md) (7.5), which carry its annotation (*Annotations to other PRDs*).

In this document, the **collation** of a `string` column is `binary`, `ci` or `ci_ai`; the **folded form** of a text is
the text that results from applying to it the collation of its column (IC2); and two texts are **equivalent** in a
column when their folded forms are equal byte by byte.

### Product decisions

| ID | Decision |
|---|---|
| IC1 | **Declaration**: a `string` column declares its collation as an attribute of its declaration, next to its type and to whether it admits nulls, when creating the table or when adding the column (E4): `binary`, `ci` or `ci_ai`. If it is not declared, it is `binary`, so today's tables and applications do not change. Only `string` columns have a collation: declaring any, also `binary`, on a column of another type (`bytes`, `json` or any other) fails with no effect with invalid schema and the new rule **collation on a column that is not text** (IC9). A column of the primary key or of an index may have any collation. The collation of a column never changes: renaming it keeps it; modifying a column (E4) also receives its collation, and giving it another, or not declaring it on a column that is not `binary`, fails with no effect with invalid schema and the rule of a change that N3 does not admit. Preparing a column in a projection (PY2) asks for the type `string` without collation, and holds for any. Describe (E2) shows the collation of each `string` column, also `binary`, and shows no collation on the others. |
| IC2 | **Folded form**, with the data of **Unicode 17.0**, fixed. With `binary`, the text as it is. With `ci`, the result of decomposing the text (NFD), folding it with the full case folding of Unicode (the entries with status C and F of `CaseFolding.txt`, without the T ones, which are the Turkish ones), decomposing it again and composing it (NFC). With `ci_ai`, the `ci` folded form, decomposed (NFD), without the marks that take no space (general category Mn) whose **base** has the Latin, Greek or Cyrillic script (Script property of Unicode), and composed (NFC); the base of a mark is the nearest scalar value that precedes it and is not Mn, and only the script of the base counts, not that of the mark. The tilde (U+0303) whose scalar value immediately before it in that decomposed form, before removing anything, is an `n` is kept, so the `ñ` and the `Ñ` are letters of their own: `año` and `ano` are not equivalent, and `Año` and `AÑO` are; a tilde separated from the `n` by another mark is removed (`n` + U+0301 + U+0303 gives `n`). The marks whose base is of another script (Devanagari, Japanese, Thai, Hebrew, Arabic, Armenian, Georgian or Coptic) or has no base are kept, so in those scripts `ci_ai` gives the same as `ci`. Since `ci_ai` starts from the `ci` form, two texts equivalent in `ci` are also equivalent in `ci_ai`. User, between removing the marks of all scripts or only of those three, and keeping the `ñ` or making it equal to the `n`: «Latín/griego/cirílico, con ñ» (Latin/Greek/Cyrillic, with ñ). Folding twice gives the same as folding once. The folded form is not always lowercase: Unicode folds Cherokee to uppercase. A scalar value that Unicode 17.0 does not assign stays as it is. Changing the Unicode version or the folding is an incompatible change of the format (B12), because it changes what is stored in the keys. Examples in *Output*. |
| IC3 | **Comparison and order**: wherever Krummi compares values of a `ci` or `ci_ai` column (the primary key, the keys of the indexes, uniqueness, ranges, estimates and statistics), it compares their folded forms byte by byte, with the rule of N5 for `string`. A descending column reverses that order, and nulls stay where their index puts them (N5). The entries of an index whose key columns are equivalent follow the order of the entries with the same key (IX8). The values of a range (the prefix and the bounds of E12 and IX9) and of an estimate (ST2) are folded before comparing, and the semantics of the bounds is that of E12 over the folded forms: in a `ci` column, `JOSÉ`, `josé` and `José` give the same range; from `ANA` included to `ana` included is the range of one value; with `ana` excluded, empty. A range that is empty or inverted after folding returns nothing and is not an error (E12). |
| IC4 | **Primary key**: in a key column with `ci` or `ci_ai`, two equivalent keys are the same row, with all the rules of E10 to E13: inserting `ANA` when `Ana` exists fails with duplicate key; reading, deleting, updating and inserting or updating with `ana` find the row `Ana`, and inserting or updating reports that it updated; a bulk load with two equivalent keys fails as with two repeated keys (E13); and deleting a range deletes the rows of its folded range. Since the primary key of a row does not change (E10), the row remains `Ana` even if it was requested with `ANA`, and an update that only gives that key is identical. |
| IC5 | **What is read**: reading a row returns each text as it was stored, without copying, also in the key columns (D11, E11); the collation does not change any read of rows or of projections (PY3). Updating a `ci` column that is not part of the key from `Ana` to `ANA` changes the row: it is not an identical update (E10), and reading it gives `ANA`. The entries of an index (IX8) return in their `ci` and `ci_ai` columns, of the index key and of the primary key, the text as it was stored in the row, the same that reading it would give, never its folded form, decoded into a copy owned by the application like the other values of the key (IX8); the included columns return it as always. User, between the original text, which makes the entries grow, and the folded form: «El texto original» (the original text). |
| IC6 | **Uniqueness**: in a unique index, two rows collide when their columns are equivalent and none is null (IX7), and creating a unique index over rows that already repeat equivalent values fails with no effect with duplicate key (IX11). Duplicate key, of the primary key or of a unique index, carries the data of E17 and IX10 with the texts of the row that collides as they would be stored (those given by the application and, when updating, those the row keeps; in a load, those of the row of the load that collides) or, when creating an index, as they are stored in the row that IX10 names; never their folded form. |
| IC7 | **Size**: a primary key, an index entry and a value of the prefix or a bound of a range (E12, IX9) are measured, as E18 and IX10 say, with the folded form of their `ci` and `ci_ai` columns, which may have more bytes than the text (`ŉ`, of 2 bytes, gives `ʼn`, of 3; the maximum factor is 3) or fewer (a `ci_ai` text with many marks). A text that fits in the key of a `binary` column may not fit in that of a `ci` one and fail with invalid size and the data of E18 and IX10, whose size is the one measured with the folded form, and the other way around. A value of a row is still measured as it is stored in the row (E18), and the original text that an index entry stores to return it (IC5) does not count for the maximum of the key: it is limited only by the maximum of the row. In an estimate, the folded form decides whether a value is large (ST2), which never gives invalid size. |
| IC8 | **Statistics** (7.3): those of a `ci` or `ci_ai` column, those of the index prefixes that include it (ST17) and equality on several columns (ST18) are computed over folded forms: `Ana` and `ANA` count as a single distinct value; the fraction of large values (ST1) counts the texts of more than 2,048 bytes, which are not read, as ST1 says, and also those that have a folded form of more than 2,048 bytes; and the frequent values and the histogram bounds that are read (ST16) are folded forms, shortened as ST16 says at a scalar value boundary of the folded form. Estimating equality or a range with `ANA` or with `ana` gives the same. |
| IC9 | **Errors**: the new invalid schema rule, collation on a column that is not text, goes in the list of E5 right after the one of the cosine metric on `bit` elements, with the column as data; like every rule of E5, it beats different schema in the modes that receive a schema (E1). Different schema (E6) also compares the collation: the new difference **collation** goes after the one of type and before the one of nullability, with the expected and the found (`binary`, `ci` or `ci_ai`); a given schema that does not declare the collation of a `string` column expects `binary`; among several columns, the first is reported as in the other differences of E6. Two default values of a `ci` column are the same only if they are equal byte by byte (E6), because rows read them as they were stored: `Ana` and `ANA` are different default values. There are no new invalid value rules: any valid UTF-8 text is valid in the three collations (E7). |
| IC10 | **Format**: an engine of format 14 does not know how to read the collation of a column, so the change is incompatible (B12) and the format version goes up by one, with the rules of JS12: to 15 if no other change has raised it since 14. A store of the older format is of an older format and this engine does not open it (F11), even if it only has `binary` columns. |
| IC11 | **Stable Unicode**: what Krummi stores in the keys cannot change because a dependency is updated. The library gains no new dependencies to fold the text, and uses no unsafe code (D12); the tests may read the Unicode data files and use other libraries only as dependencies of the tests (AC-IC2). |
| IC12 | **Measurement** (N11, with the annotation of 2026-11-20 of the parent): a single pass of one round of the loads W1–W32 before and after the change, and 10 to 20 rounds of those affected by it, which are those of tables, W23 to W32, whose key and schema encoding changes, and of those that in that pass deviate more than 3 % from the base. There are no new loads of the benchmark nor new thresholds: Voron tables have no collation. In addition, the closing report measures, on Windows and on Linux, with the median of 10 to 20 rounds, on a table of 100,000 rows with a `string` key of about 24 bytes of text with uppercase and accents, generated with a fixed seed, and a unique index on another equal `string` column: inserting in writes of 1,000 rows, like W23; reading by the key; and searching by the index. Each measure is compared with the same table with both columns `binary`, which is its floor. |
| IC13 | **Closing the module**: 7.7 is built after 7.5 and before module 17, and N2 closes module 7 when 7.1, 7.2, 7.3, 7.5, 7.6 and 7.7 are closed. |

### Starting state

Verified on 2026-11-23 in the code and in the PRDs of the ancestors and siblings:

- A column has identity, name, type, whether it admits nulls and default value; the `string` type has no parameters.
  Describe returns those five things of each column, and different schema compares the lack of the column, the type,
  the nullability, the default value and the primary key (E2, E6).
- Text is compared byte by byte everywhere: the primary key, the keys of the indexes, the uniqueness check when writing
  and when creating an index, the ranges of E12 and IX9, the statistics and the estimates. Nothing in the library
  normalizes, folds or lowercases the stored text, and the library does not depend on any Unicode library.
- The `string` columns of the primary key are also stored in the row, so reading a row returns the stored text without
  copying. The entries of an index return the columns of their key and of the primary key decoded from the entry
  itself, without reading the row (IX8).
- There is no text prefix search operation. The estimates are equality, range and equality on several columns (ST2).
- Modifying a column only admits allowing nulls on one that is not part of the key (E4).
- The format version is 14 (JS12).
- The benchmark has 32 loads; those of tables, W23 to W32, have `ulid` and `uuid` keys and indexes on an integer and an
  instant, without any `string` column in a key.

## Actors

- **Application developer (Rust)**: declares the collation of their text columns and searches, traverses and
  guarantees uniqueness without worrying about uppercase or accents.
- **Queries (module 17)**: will compare text with the collation of its column and will do the text prefix search over
  the order of IC3.
- **Maintainer**: runs the tests and the measurements of IC12 on Windows and on Linux.

## Flow

1. The application creates a table, or adds a column, with a `string` column `ci` or `ci_ai` (IC1).
2. It stores rows with the text as the user writes it; Krummi stores it as it is and uses its folded form for the
   primary key, the indexes and the statistics (IC2, IC3).
3. It searches, traverses ranges and checks uniqueness with any combination of uppercase and accents, and obtains the
   rows with their original text (IC3 to IC6).

## Validations and messages

Those of the ancestors apply. This PRD adds, only in the library:

| Situation | Error class |
|---|---|
| Declaring a collation, also `binary`, on a column that is not `string`, when creating the table, when adding the column or when opening it with a mode that receives a schema | Invalid schema, rule collation on a column that is not text (IC9) |
| Modifying a `string` column giving it another collation, or not declaring it when it is not `binary` | Invalid schema, rule of a change that N3 does not admit (IC1) |
| Opening a table with a schema that gives a `string` column another collation, or does not declare it and that of the table is not `binary` | Different schema, difference collation (IC9) |
| Inserting, or bulk loading, a row whose primary key is equivalent to that of another | Duplicate key (IC4, IC6) |
| Storing a row whose columns of a unique index are equivalent to those of another, without nulls | Duplicate key, with the name of the index (IC6) |
| Creating a unique index over rows with equivalent values | Duplicate key, without changing anything (IX11, IC6) |
| A primary key, an index entry, a value of the prefix or a bound whose folded form exceeds the maximum of the key | Invalid size (IC7) |
| Opening a store of format 14 | Older format (F11, IC10) |

## Output

The module writes nothing to the screen. Rows and projections return the text as it was stored (IC5). Describe shows
the collation of each `string` column, and its default value as it was declared (IC1). The traversals of a `ci` or
`ci_ai` column return the rows in the order of their folded forms (IC3). The statistics that are read show folded
forms (IC8). Different schema and duplicate key carry the data of IC9 and IC6. Temporary stores behave the same. The
`binary` columns do not change any output.

Examples of folded forms and of equivalence (IC2):

| Text | `ci` folded form | `ci_ai` folded form |
|---|---|---|
| `José` | `josé` | `jose` |
| `JOSÉ` | `josé` | `jose` |
| `Jose` + U+0301 (combining acute accent) | `josé` | `jose` |
| `Straße` | `strasse` | `strasse` |
| `ΣΑΣ` | `σασ` | `σασ` |
| `ᾳ` (alpha with iota subscript) | `αι` | `αι` (the iota subscript folds to a `ι`, which is a letter) |
| `µ` (micro) + U+0301 | `μ́` | `μ` (folds to the Greek `μ`, whose marks are removed) |
| `ﬁn` | `fin` | `fin` |
| `İstanbul` | `i̇stanbul` (an `i` with the combining dot U+0307) | `istanbul` |
| `Istanbul` | `istanbul` | `istanbul` |
| `ılık` (with dotless `ı`) | `ılık` | `ılık` |
| `Ærø` | `ærø` | `ærø` (`æ` and `ø` do not decompose, so they are not `ae` or `o`) |
| `Año` | `año` | `año` (`ñ` is a letter of its own) |
| `Ångström` | `ångström` | `angstrom` |
| `Ёлка` | `ёлка` | `елка` |
| `Йод` | `йод` | `иод` (`й` loses its mark, like `ä` or `å`) |
| `ña` + U+0323 (dot below) | `ñạ` | `ña` |
| `n` + U+0301 + U+0303 | `ń̃` | `n` (the tilde does not directly follow the `n`) |
| U+0301 alone | U+0301 | U+0301 (it follows no letter) |
| `が` | `が` | `が` (Japanese keeps its marks) |
| `कु` | `कु` | `कु` (Devanagari keeps its marks) |

## Reuse

- The key encoding of 7.1 and 7.2, their directions and the order of their nulls, applied to the folded form.
- The uniqueness check (IX7), the creation of indexes over rows that already exist (IX2, IX11) and the duplicate key
  data (E17, IX10).
- Schema compatibility (E6), Describe (E2) and the list of invalid schema rules (E5).
- The statistics and the estimates of 7.3 over the values encoded as in the key.
- The read without copying of the `string` columns (D11).
- The random value generator and the property tests with a reference model of AC-N1 and AC-N7.

## Out of scope

- The alphabetical order of a language (for example, the `ñ` between the `n` and the `o`) and the collations by
  language, like the Turkish rules: in `ci`, `İ` is not equivalent to `i` nor `ı` to `i`; in `ci_ai`, `İ` is
  equivalent to `i`, because it loses its dot like an accent, and `ı` still is not (IC2).
- Removing the accents of the scripts that are not Latin, Greek or Cyrillic, and making the `ñ` equal to the `n` (IC2).
- Distinguishing or equating by width (`Ａ` and `A`), equating hiragana with katakana, and the compatibility forms
  (NFKC); the `æ`, the `œ`, the `ø` or the `ł` are not equivalent to other letters (IC2).
- Normalizing the text of a `binary` column: it keeps being compared byte by byte.
- Changing the collation of an existing column: a new column is added with the desired collation and the values are
  copied (IC1).
- A collation per index or per query, different from that of its column, and the collation clause of a query
  (module 17).
- Comparing default values by their folded form (IC9).
- Indexes on expressions (design document 14).
- A text prefix search operation, `LIKE` or "starts with": module 17, over the order of IC3.
- The collation of the strings inside a JSON document (design document 14) and that of `bytes`.
- The names of tables, columns and indexes, which keep being compared by their bytes (B4).
- Unicode later than 17.0 (IC2).
- The C interface and the C# binding (module 18).

## Left to the implementation

- How the Unicode data used by the folding is generated and stored, as long as they give exactly the folded form of
  IC2 and meet IC11.
- How an index entry stores the original text of IC5, as long as it does not count for the maximum of the key (IC7).
- The English text of the errors (D8); the names of the operations, the types, the collations and the rules, in English
  and with Rust conventions.

## Acceptance criteria

1. **AC-IC1** — Given a table created with `string` columns without a declared collation, `binary`, `ci` and `ci_ai`,
   and a `ci` column added afterwards, when it is described, then each `string` column shows its collation (`binary`
   the first two) and those of other types none; when a `ci` column is renamed and another is modified to allow nulls
   giving `ci`, then both keep their collation; when a `ci` column is modified giving `binary` or without declaring it,
   then it fails with no effect with invalid schema and the rule of a change that N3 does not admit; when a table is
   created, a column is added or it is opened with a mode that receives a schema, with a collation (also `binary`) on a
   `bytes`, `json`, integer and vector column, then it fails with no effect with invalid schema, the rule collation on
   a column that is not text and the column; with a collation on a `bytes` column and the cosine metric on `bit` on
   another, the cosine metric wins, and with a collation on a `bytes` column and an invalid default value on another,
   the collation wins; and when the table is opened with a schema that gives another collation to a `string` column, or
   does not declare it on the `ci` one, then it fails with different schema, the difference collation and the expected
   and the found, and with another type and another collation at once, with the difference type; when it is opened with
   a schema that gives a `ci` column the default value `ANA` where the table has `Ana`, then it fails with different
   schema and the difference default value; and when it is opened with a schema that declares a collation on a `bytes`
   column and another collation on a `string` one, then it fails with invalid schema, not with different schema.
2. **AC-IC2** — Given all the scalar values of Unicode (without U+D800–U+DFFF), when each one is folded in `ci` and in
   `ci_ai`, then the folded form is the one the definition of IC2 gives computed in the test with the data files of
   Unicode 17.0 (`CaseFolding.txt`, `UnicodeData.txt`, with its general category and its decomposition, and
   `Scripts.txt`), folding the folded form gives the same, and an unassigned value stays as it is; when the strings of
   `NormalizationTest.txt` of Unicode 17.0 are folded, then the three canonically equivalent columns of each line (c1,
   c2 and c3: the text, its NFC and its NFD) give the same folded form; when a property test folds random sequences of
   Latin, Greek, Cyrillic letters and of other scripts with marks, including `ŉ` + U+0303, `ẚ` + U+0301, `µ` + U+0301,
   `a` + U+200D + U+0301 and `ñ` + U+0323, then the folded form is that of the definition, folding it again gives the
   same, and two sequences equivalent in `ci` are so in `ci_ai`; and for the examples of *Output*, the folded form is
   that of the table.
3. **AC-IC3** — Given a table with a `ci` primary key and another with a `ci_ai` one, when `Ana` is inserted, then
   inserting `ANA` (and in `ci_ai` also `Àna`) fails with duplicate key with the text `ANA` (or `Àna`) and the table is
   left as it was; reading and deleting with `ana` find the row `Ana`; updating with `ANA` changes the other columns and
   the key is still `Ana` when reading it; inserting or updating with `ANA` reports that it updated; an update that
   only gives `ANA` is identical; deleting `Ana` and inserting `ANA` in the same write leaves the row `ANA`; a bulk
   load with `Bea` and `BEA` fails as with two repeated keys (E13); deleting the range from `ANA` included to `ana`
   included deletes the row `Ana` and returns 1; and updating a `ci` column that is not part of the key of `Eva` to
   `EVA` is not an identical update, and reading the row, by its name and in a projection, gives `EVA` without copying.
4. **AC-IC4** — Given a table with a unique index on a `ci` column and another on a `ci_ai` one, when a row is stored
   with a value equivalent to that of another row, also when updating it, then it fails with duplicate key, with the
   name of the index and the texts of the row that collides as they would be stored (IC6), and the row, the indexes and
   the number of rows are left as they were; when updating the row with key `Ana` requesting it with `ANA` and giving
   the index column a value equivalent to that of another row, the primary key of the error is `Ana`, the one the row
   keeps (IC4); two rows with nulls do not collide; updating a row from `Ana` to `ANA` in the index column does not
   collide with itself; a bulk load with two rows whose values of a unique index are `Bea` and `BEA` fails with
   duplicate key, with the name of the index and the texts of the row of the load that collides; and when each unique
   index is created over rows that already have `Ana` and `ANA`, then it fails with no effect with duplicate key
   (IX11), with the texts as they are stored.
5. **AC-IC5** — Given a table with `ci` and `ci_ai` columns in the primary key and in ascending and descending indexes
   with nulls first and last, when a property test stores random rows with texts that mix uppercase, precomposed and
   combining accents, `ß`, `ŉ`, Greek letters with final sigma, texts made only of marks and the empty text, and
   traverses the table and each index forward and backward, and by ranges with each combination of included, excluded
   and absent bounds, given with random uppercase and accents, and deletes ranges of the primary key, then the order,
   the rows of each range, the deleted rows and the duplicate keys are those of a reference model that compares the
   folded forms of AC-IC2 and does not use the key encoding; and each row is read with the text that was stored; also
   in a temporary store.
6. **AC-IC6** — Given an index with `ci` and `ci_ai` columns in its key and an included `ci` column, over a table with
   a `ci` primary key, when its entries are read, then they return in the columns of the index key, in those of the
   primary key and in the included one the text as it was stored in the row, also after updating a column of the index
   key from `Ana` to `ANA`, and in the order of the folded forms.
7. **AC-IC7** — Given a `ci` column and a text of repeated `ŉ` whose folded form exceeds the maximum of the key but the
   text does not, when it is inserted as primary key, stored in a column of an index, given as a bound of a range or
   as a value of the prefix, then it fails with invalid size and the data of E18 and IX10, even if the range is empty;
   in a `binary` column the same text is stored; and in a `ci_ai` column, a text of letters with combining accents that
   exceeds the maximum of the key but whose folded form does not, is stored as primary key and is read whole.
8. **AC-IC8** — Given a table of 30,000 rows or fewer, so that the statistics read all its rows (ST1), with a `ci`
   column and a `ci_ai` one with values that differ only in uppercase or accents and an index on the `ci` and an
   integer column, when they are analyzed, then the number of distinct values counts each folded form once, the
   frequent values and the histogram bounds are folded forms, a shortened bound (ST16) is so at a scalar value
   boundary of the folded form, the fraction of large values counts a `ci` text of fewer than 2,048 bytes whose folded
   form exceeds them, the statistics of the index prefix (ST17) count the combinations by their folded form, and
   estimating equality, a range or equality on several columns (ST18) with any combination of uppercase and accents
   gives the same as with the folded form.
9. **AC-IC9** — Given a store of the older format, also one with only `binary` columns, when this engine opens it, then
   it fails with older format (F11); and a store created by this engine has the version of IC10.
10. **AC-IC10** — Given the tests of 7.1, 7.2, 7.3 and 7.5, with `string` columns without a declared collation, when
    they are run, then they give the same as before the change, except the format version, which is that of IC10.
11. **AC-IC11** — Given the closing of this PRD, when its report is reviewed, then it has, on Windows and on Linux, the
    measures of the loads of IC12 before and after the change and those of the table of IC12 with `ci` columns against
    `binary`.
12. **AC-IC12** — Given the dependencies of the library before and after the change, without those of the tests, when
    they are compared, then they are the same (IC11).

## Annotations to other PRDs

This PRD changes or makes precise rules of earlier documents, which carry the annotation of 2026-11-23 (in their
translations too):

- Parent, N2: module 7 is closed when 7.1, 7.2, 7.3, 7.5, 7.6 and 7.7 are closed (IC13). N4: a `string` column declares
  its collation (IC1). N5: "`string` and `bytes` byte by byte, without collation" now holds for `bytes` and for the
  `string` columns `binary`; the `ci` and `ci_ai` ones are ordered and compared by their folded form (IC2, IC3), and
  the primary key and the unique indexes compare the folded forms. *Split*: child PRD 7.7, *Collation of the text
  columns*, is added. *Out of scope*: "the comparison of text without distinguishing uppercase" is no longer out of
  scope.
- 7.1, E2: Describe shows the collation of each `string` column (IC1). E4: a `string` column declares its collation as
  part of its type when creating or adding it, and modifying it with another is a change that is not admitted (IC1).
  E5: the new rule of IC9. E6: the collation is part of the compatibility, with the difference collation (IC9). E10 to
  E13 and E17: equivalent keys are the same row, ranges are folded, and duplicate key carries the given text (IC3, IC4,
  IC6). E18: the primary key is measured with the folded form (IC7). E20: the format goes up by one (IC10).
- 7.2, IX7 and IX11: uniqueness compares equivalent values (IC6). IX8: the entries return the stored text, not the
  folded form (IC5). IX9: ranges are folded (IC3). IX10: the entry, the prefix and the bounds are measured with the
  folded form (IC7), and duplicate key carries the given or the stored text (IC6). *Out of scope*: "Collations and
  comparison of text without distinguishing uppercase" is no longer out of scope, except for what this PRD excludes.
- 7.3, ST1, ST2, ST16, ST17 and ST18: the estimates and the statistics of a `ci` or `ci_ai` column use folded forms; a
  text whose folded form exceeds 2,048 bytes is also large, and a bound is shortened at a scalar value boundary of the
  folded form (IC7, IC8).
- Projections, PY2: a `string` column is prepared with the type `string`, whatever its collation (IC1).
- 7.5, AC-JS12: a new store is of the format of IC10, not of 14.
- [Design document 3](../../../design/tables/03-keys-and-indexes.md), *Strings and collation*, risk *No
  collation* and product question 5, and the [design README](../../../design/tables/README.md): the decision of
  2026-11-09 of binary order without collation now holds only for the `binary` columns, and the comparison without
  distinguishing uppercase stops being a future index on expressions (IC1, IC2).

## References

- Parent: [tables.prd.md](tables.prd.md) (N2–N5, N10, N11, split, *Out of scope*)
- Trees: [trees.prd.md](../trees/trees.prd.md) (B4, B5, B12); free space:
  [free-space.prd.md](../free-space/free-space.prd.md) (F11)
- Siblings: [tables-schema.prd.md](tables-schema.prd.md) (E1, E2, E4–E7, E10–E13, E17, E18, E20),
  [tables-indexes.prd.md](tables-indexes.prd.md) (IX1, IX2, IX7–IX11), [tables-statistics.prd.md](tables-statistics.prd.md)
  (ST1, ST2, ST16–ST18), [tables-json.prd.md](tables-json.prd.md) (JS12), [tables-projection.prd.md](tables-projection.prd.md)
  (PY2, PY3)
- Root: [krummi.prd.md](../krummi.prd.md) (D8, D11, D12)
- Design: [document 3, keys and indexes](../../../design/tables/03-keys-and-indexes.md) (*Strings and collation*) and
  [document 14, indexes on JSON](../../../design/tables/14-json-indexes.md)
- Unicode 17.0: `CaseFolding.txt`, `UnicodeData.txt`, `Scripts.txt`, `NormalizationTest.txt` (Unicode Character
  Database)

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| 69c105b1 | 2026-11-21 06:51 UTC+00:00 | 48 h 43 min | 47 h 40 min | 1 h 3 min | 42 min | 21 min | 7 |

<!-- prd-time: [{"session": "69c105b1-9a71-44c2-9e9d-c3c6315b92d5", "start": "2026-11-21T06:51:30.637000+00:00", "wall": 175372, "discarded": 171570, "agent": 2531, "human": 1269, "interventions": 7, "idle_minutes": 10}] -->
