# JSON Columns of the Tables — PRD

**Version**: 1.0
**Date**: 2026-11-21
**Parent document**: [Krummi Tables — PRD](tables.prd.en.md) (v1.0)

> English translation of [tables-json.prd.md](tables-json.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Build child PRD **7.5** of the [split of the tables module](tables.prd.en.md#split-into-child-prds): a **JSON** column
type that stores one JSON document per row, validates it when it is written, returns it whole without copying it and
allows navigating it and reading its properties as Krummi types.

**Why now**: 7.1, 7.2 and 7.3 are closed, and the annotation of 2026-11-10 of the parent builds 7.5 after 7.3 and
before module 17, so that queries can filter by JSON paths from the start.

The technical design is already decided in [design document 13](../../../design/tables/13-json-column-storage.md),
which is the technical starting point of this PRD, and almost everything that 7.5 had to fix was fixed in the
annotation of 2026-11-12 of the parent (*Split into child PRDs*): the normalized text as the stored form, what a valid
JSON value is, the null of the column versus the JSON `null`, numbers, repeated keys, the order of the properties,
equality when storing, depth, the error classes, the own reader and the typed read. This PRD does not decide them
again: it makes them precise until they leave no observable gaps and fixes what the annotation left to 7.5 (whether
the change is incompatible, how the column is declared, the data of its errors and their precedence, its benchmark
loads), besides what the sibling PRDs left to it: the statistics of JSON columns (ST1 of
[tables-statistics.prd.en.md](tables-statistics.prd.en.md)) and their prepared column in projections
([tables-projection.prd.en.md](tables-projection.prd.en.md), *Out of scope*).

Decisions of the user of 2026-11-21, on the proposals of the draft: the text is validated when it is written, with the
table and the column in the error («Al escribir» (when writing), JS3); JSON columns have no statistics («Ninguna, como
vector» (none, like vector), JS10); there is no document constructor without text («No, fuera de 7.5» (no, out of
7.5), JS11); and there are no new loads of the benchmark, but an informative measurement in the closing report
(«Medición informativa» (informative measurement), JS13).

This PRD inherits everything from its ancestors. It makes precise or extends rules of its parent and of
[tables-schema.prd.en.md](tables-schema.prd.en.md) (7.1), [tables-projection.prd.en.md](tables-projection.prd.en.md) and
[tables-statistics.prd.en.md](tables-statistics.prd.en.md) (7.3), which carry its annotation (*Annotations to other
PRDs*).

In this document, a **document** is the value of a JSON column in a row; a **JSON value** is the document or any value
it contains (an object, an array, a string, a number, `true`, `false` or `null`); a **property** is a pair of a name and
a value inside an object; the **normalized text** is the form in which Krummi stores and returns a document (JS4); and
**absent** is the result of navigating to a value that the document does not have, different from a `null` value.

### Product decisions

| ID | Decision |
|---|---|
| JS1 | **The column type**: N4 gains the type `json`, with no parameters, which is declared like any other column type (E4) and which Describe shows as `json`. A JSON column admits nulls unless it is declared otherwise, may have a default value (JS9) and follows the schema changes of N3 and the rules of E5 like any column: one that admits nulls or that has a default value can be added, removed (unless it is in an index, also as an included column, E5), renamed and allowed to admit nulls; changing the type of a column to `json` or from `json` is not admitted (N3). It cannot be a column of the primary key: the rule of E5 «a column of the key that admits nulls, is a vector or has a default value» includes the JSON column. It cannot be a column of the key of a secondary index (IX3); it can be an included column (IX1), like any other type. JSON columns work the same in a temporary store (G9). |
| JS2 | **What a valid document is** (makes precise the annotation of 2026-11-12 of the parent): a valid UTF-8 text that is exactly one RFC 8259 value, also a bare value at the root (`42`, `"x"`, `true`, `null`), with optional whitespace before, after and between its elements; the whitespace characters are the four of RFC 8259 (space, tab, line feed and carriage return) and no other. These are not valid: an empty text or one made only of spaces; data after the value; a BOM mark; a comment; `NaN`, `Infinity`, `+1`, `01`, `1.`, `.5`, `0x10` or any number outside the grammar of RFC 8259; a single quote as a delimiter; a trailing comma (`[1,]`, `{"a":1,}`); a control character U+0000 to U+001F unescaped inside a string; an escape that RFC 8259 does not define (`"\x"`, `"\u12"`); a lone UTF-16 surrogate, escaped (`"\ud800"`, `"\udc00"`, or a high one followed by something that is not a low one); an object with two properties of the same name, the names compared by their characters after decoding their escapes and without any Unicode normalization (`{"a":1,"a":2}` is not valid; `"é"` written as one character and as `e` followed by the combining accent are different names); and more than 128 nested objects or arrays (the document `[[…]]` with 128 opening brackets is valid and with 129 it is not). An escaped surrogate pair is its character; U+0000, escaped as `\u0000`, and the Unicode noncharacters are valid, as in a `string` column (E7). Numbers have no limit of digits or exponent beyond the size of the row (`1e400`, `123456789012345678901234567890`). |
| JS3 | **Writing a document**: the application gives the value of a JSON column as a value of the JSON class, made of a text or of some bytes, which may not be valid UTF-8; to store a document that was read, it gives its text (JS6), while reading is valid (B9). A `string` or `bytes` value in a JSON column, or a JSON value in a column of another type, fails with invalid value and the rule type different from that of the column (E17). Insert, update, insert or update and bulk load validate and normalize the document before changing anything, with the rules of E17 (JS8); a text that is not a valid document fails with invalid value, and in a bulk load the load aborts entirely (E13). Making the JSON value validates nothing: validation happens only when writing, so its error carries the table and the column (user, between validating when writing and validating when building the value: «Al escribir» (when writing)). |
| JS4 | **The normalized text**: Krummi stores and returns each document in this form, which keeps everything the document means (user, in the annotation of 2026-11-12: «Sí seguimos con el texto normalizado.» (Yes, we continue with the normalized text.)): (1) no whitespace outside strings; (2) `true`, `false` and `null` as they are; (3) each number with the same characters with which it was written (`1.50e+3` stays `1.50e+3`, `-0` stays `-0`); (4) each string and each property name with its escapes decoded and written again like this: all characters as they are in UTF-8, except `"` as `\"`, `\` as `\\`, U+0008 as `\b`, U+000C as `\f`, U+000A as `\n`, U+000D as `\r`, U+0009 as `\t`, and the others from U+0000 to U+001F as `\u00` followed by two lowercase hexadecimal digits (`\u0000`, `\u001f`); so `\/` is written `/`, `é` is written `é` and `😀` is written `😀`; (5) the properties of each object in the order in which they arrived. Normalizing a normalized text gives the same text, and the normalized text is never longer than the given one. For example, ` { "b" : 1.50 , "a" : [ true , "é\/" ] } ` is stored and read as `{"b":1.50,"a":[true,"é/"]}`. The normalized text is not canonical: `{"a":1,"b":2}` and `{"b":2,"a":1}`, or `{"n":1}` and `{"n":1.0}`, are different documents (annotation of the parent). The size of the row that E18 counts includes the normalized text, not the given one: an indented document whose text exceeds the maximum of the row is admitted if its normalized text fits. |
| JS5 | **Reading a document**: a JSON column is read like any column, by its name or by its position (E11), in a projection with a JSON prepared column (PY2, PY3), and as an included column of an index entry (IX8). It returns null if the column is null or, otherwise, the document as a JSON value (JS6), whose whole normalized text is obtained without copying it (D11), with the life of the other references of the transaction (B9, E16): of the row, when reading it by its key, when traversing it or with a projection, and of the index entry, without reading the row, when reading it as an included column (IX8). From a copied row (PY3) it is returned in a copy owned by the application, like its other values, which is navigated and read as a type like a document that was read (JS6, JS7), and whose values and texts are valid while that copy lives. A JSON column is prepared only as JSON, and one of another type is not prepared as JSON: both cases fail with different schema, with the requested type and that of the column (PY2). Reading does not check the stored document (L1): navigating or reading as a type a document of a malformed row follows L2, so it may return a value that is not the one that was written or end with a normal panic of the thread, never with a stack overflow or without ending. Equality when storing is that of the parent: an update with a document whose normalized text is the same as the stored one is identical (E10), so another document that differs only in whitespace or in the form of its escapes does not change the row; `{"n":1}` versus `{"n":1.0}` does change it. |
| JS6 | **Navigating a JSON value**: from any JSON value its **class** is obtained, one of object, array, string, number, boolean or `null`, and its **text**, the fragment of the normalized text of the document that forms it, without copying it, which is in turn a valid normalized text: from `{"a":[1,2],"s":"x"}`, the value of `a` has the text `[1,2]` and that of `s`, `"x"`, with its quotes. From an object are obtained its number of properties, its properties in the stored order, each one with its name and its value, and the value of a property by its name, compared as in JS2, or absent; from an array, its number of elements, its elements in order and the element at a position, counting from 0, or absent if the position is the number of elements or greater. Asking a value of another class for its properties, its number of properties or a property, or its elements, its number of elements or an element, gives absent; an empty object or array give zero and no property or element. A property name is obtained as text, without quotes, with the copy rules of a string read as text (JS7). From any JSON value the value of a **JSON pointer** (RFC 6901) is also obtained, relative to that value. An empty pointer (`""`) gives the value itself; otherwise, it is a sequence of steps, each one `/` followed by a name in which `~1` is `/` and `~0` is `~`; each step on an object gives its property of that name; on an array, the element at that position if the step is `0` or a number of decimal digits without leading zeros (`10`, not `010` or `+1`), and absent otherwise, also `-`, which in RFC 6901 names the position after the last element; and on a string, a number, a boolean or `null`, absent. So, in `{"a/b":{"~":[10,20]}}`, the pointer `/a~1b/~0/1` gives `20`. A malformed pointer, one that is not empty and does not start with `/` or with a `~` that is not followed by `0` or `1`, fails with invalid value and the rule malformed JSON pointer, with no table, no column and no more data, before navigating, also if an earlier step already gave absent. Navigating reads no pages other than those of the row, is not a change for T10 and does not end references (E16). |
| JS7 | **Reading a JSON value as a Krummi type** (makes precise the annotation of 2026-11-12 of the parent): from any JSON value its reading is requested as one of these types, and it gives the value or null, never an error because of the value. **Text**, from a string, without its quotes, without copying it if the string has no escapes in the normalized text, and decoded in a copy owned by the application if it has them (`"a\nb"`). **Boolean**, from `true` or `false`. **`i64`** and **`u64`**, from a number whose exact value is an integer in its range, in any notation: `100`, `100.0`, `1e2` and `1.00e2` give 100, `-0` and `0e999999999` give 0, and `100.6`, `1e-1`, `9223372036854775808` (in `i64`) or `-1` (in `u64`) give null. **`decimal(p, s)`**, from a number rounded to the scale `s` as in E8 (`1.235` gives `1.24` in a `decimal(10, 2)`), and null if, once rounded, it has more than `p` digits (`99.995` in a `decimal(4, 2)`). **`f64`**, from a number rounded to the nearest, with `-0` as `+0` (N4): `0.1` and `0.10000000000000001` give the same `f64`, `1e-400` and `-1e-400` give `+0`, and outside the finite range it gives null (`1e400`). A number is read without expanding its exponent: the time and the memory of the read grow with the length of its text, not with its value (`1e999999999999` gives null in `i64` and in `decimal` without building its digits). **Date**, from a string exactly `YYYY-MM-DD`, with a four-digit year, a month from 01 to 12 and a day that exists in that month, within the domain of E8 (`"2026-11-11"`). **Instant**, from a string `YYYY-MM-DDTHH:MM:SS`, with the date as in the read of a date, the hour from 00 to 23, the minutes and the seconds from 00 to 59, an optional fraction of a second, a point followed by one or more digits, and an offset `Z`, `+HH:MM` or `-HH:MM` with the hour from 00 to 23 and the minutes from 00 to 59 (`-00:00` is UTC), with `T` and `Z` in uppercase; it is converted to UTC, the fraction is rounded to the microsecond as in E8 and the domain of E8 is checked after converting and rounding (`"2026-11-11T10:23:00.1234567Z"` gives `2026-11-11T10:23:00.123457Z`; `"2026-11-11T00:30:00+01:00"` gives `2026-11-10T23:30:00Z`). These give null: a value of another class (a number where a text, a date or an instant is requested; a string where a number is requested), `null`, a date that does not exist (`"2026-02-30"`), a year that does not have four digits or is `0000`, an instant without an offset, in lowercase, with a space instead of `T`, with a leap second (`:60`), with a point without digits or outside the domain. A `p` or an `s` outside the limits of N4 (1 ≤ p ≤ 38, 0 ≤ s ≤ p) fails with invalid value and the new rule invalid read scale, with no table, no column and no more data, before looking at the value. What is absent has no value to read: the typed read is requested on a JSON value obtained by navigating (JS6). |
| JS8 | **Errors: data and precedence** (extends E17): an invalid document fails with **invalid value**, with the table, the column and the new rule **invalid JSON**, which goes in the list of E17 after malformed text; since a JSON column can only break null not admitted, type different from that of the column or this one, the order of E17 does not change for the others. It also carries: the **cause**, one of invalid UTF-8, syntax, lone surrogate, repeated property (with the repeated name, decoded) or more than 128 levels; the **position**, the number of bytes of the given text before the failure, counting from 0; and the **JSON pointer** (RFC 6901) of the innermost object or array open at that position, with each name written with `~0` and `~1` and its other characters as they are, `""` if it is the root or there is none. Each cause has its position: **invalid UTF-8**, the first byte of the first sequence that is not valid UTF-8, also an incomplete one at the end; **syntax**, only the grammar of RFC 8259 (without the rules of the other causes), at the first character with which the text stops being the beginning of some text of that grammar, or the length of the text if it ends before completing one (an empty text gives position 0); **lone surrogate**, the `\` of the escape that does not form a pair; **repeated property**, the opening quote of the name that repeats an earlier one of the same object, when that name is a string without failures; and **more than 128 levels**, the `{` or the `[` that opens level 129. If the text has several failures, the one with the lowest position is reported and, at the same position, in the order of the list of causes. For example, `{"a":1,"b":{"a":2,"a":3}}` fails with repeated property, the name `a`, position 18 and pointer `/b`. A valid document that makes the row larger than its maximum fails with invalid size (E18), after invalid value according to E17, with the size of the row that JS4 counts. In a bulk load, the error is that of the first row that fails, as in E13. The data of the errors are given apart from their text (D8). |
| JS9 | **Default value** (makes precise the annotation of the parent): the default value of a JSON column is given as in JS3, is stored normalized and Describe returns it normalized; the document `null` is a default value, not the lack of one, so in a JSON column the phrase of E4 «declaring null as the default value is the same as having none» does not hold for the document `null` (the null of the column does still equal having none). One that is not a valid document makes the creation, the schema change or the opening with a schema fail with invalid schema and the rule of E5 of a default value that does not meet the rules of its column, with the table and the column, without the cause, the position or the pointer of JS8, with the precedence of E17 (before different schema). The too-large schema of E5 counts the normalized default value. When opening a table with a schema (E6), two JSON default values are the same when their normalized text is the same: `{ "a" : 1 }` is compatible with `{"a":1}`, and the default value `null` is not with the lack of a default value, which gives different schema. |
| JS10 | **Statistics** (what ST1 leaves to 7.5): like vector columns, JSON columns have no statistics (user, between none and only the null fraction: «Ninguna, como vector» (none, like vector)); estimating, reading the statistics or analyzing a JSON column fails with invalid schema and a new rule, JSON column, with the table and the column, like the rule vector column of ST12 and in the same place of the precedence, and an equality on several columns (ST18) that includes a JSON column fails the same way. Analyzing the whole table builds those of the other columns, which are the same as in an identical table without the JSON column, and the sampling does not read the documents. The statistics of the paths inside a JSON arrive with the indexes on expressions (design document 14). |
| JS11 | **Building a document**: «building it», from the split of the parent, is storing it from its text (JS3). There is no constructor of objects and arrays without text: the application builds the text with its own JSON library and Krummi validates it when writing (user, between leaving it out of 7.5 and a minimal one: «No, fuera de 7.5» (no, out of 7.5)). Adding it later is a compatible change. |
| JS12 | **Format**: an engine of format 13 does not know how to read a store with JSON columns, so the change is incompatible (B12) and the format version becomes 14: a store of format 13 is of an earlier format and this engine does not open it (F11). The information of the store (P12) does not change its form. |
| JS13 | **Measurement** (N11, with the annotation of 2026-11-20 of the parent): a single pass of one round of the loads W1–W32 before and after the change, and 10 to 20 rounds of those affected by it, which are those of tables without indexes, W23 to W29 (insert, load, read, traverse, update and delete rows, whose encoding gains a column type), and of those that in that pass deviate more than 3 % from the base. There are no new loads of the benchmark nor new thresholds: Voron has no JSON type (user, between an informative measurement, new loads against Voron with the blittable of RavenDB and no new measurement: «Medición informativa» (informative measurement)). In addition, the closing report measures, on Windows and on Linux, with the median of 10 to 20 rounds, on a table with an integer key and a JSON column and with documents generated with a fixed seed, of about 200 bytes (100,000 rows), about 4 KB (100,000 rows) and about 64 KB (10,000 rows), each one with properties before and after a large array: inserting them in writes of 1,000 rows, like W23; reading the whole document by the key; and reading by the key the first property of the root and the last. Each measure is compared with the same table with the same normalized text in a `string` column, read whole, which is its floor. |
| JS14 | **The reader, «muy bien probado»** (very well tested) (user, in the annotation of the parent: «El propio, pero muy bien probado.» (Our own, but very well tested.)): Krummi parses the text with its own reader, without new dependencies of the library and without unsafe code (D12); the tests may use `serde_json` with its default options, only as a dependency of the tests, which does not reach the library. It is tested in layers: a public conformance corpus, a reference model, a comparison with another library, numbers outside `f64` and `u64`, exhaustive small inputs, property tests, limits, mutants and fuzzing (AC-JS11). |

### Starting state

Verified on 2026-11-21 in the code, in the PRDs of the ancestors and siblings and in the design documents:

- Tables have 21 column types (N4) and none is JSON; in the library there is no JSON code, neither a reader nor a
  dependency that parses JSON, and the code of the queries (module 17) does not exist yet.
- The catalog stores the type of each column and rejects as damage a type it does not know. The format version is 13.
- `string`, `bytes` and vector columns are read without copying (D11), by their name, by their position, in projections
  with prepared columns of text, bytes and vector, and as included columns of an index; the others, in a copy owned by
  the application. Reading does not check the content of the row (L1, L2).
- Vector columns are the precedent of a type that does not sort: they cannot be of the primary key (invalid schema,
  E5) nor of the key of an index (invalid schema, IX3), they can be included columns, and they have no statistics
  (invalid schema with the rule vector column, ST12).
- 7.2 already excludes JSON from the key of an index (IX1, IX3) and returns without copying the JSON of an included
  column (IX8); 7.3 leaves to 7.5 the statistics of JSON columns (ST1); the projections leave to 7.5 their prepared
  column.
- Invalid value carries the table, the column and a rule of the list of E17; invalid schema, the table, the column and a
  rule of E5.
- The benchmark has 32 loads; those of tables, W23 to W32, are run only by Krummi and Voron, and none has a JSON
  column.

## Actors

- **Application developer (Rust)**: declares JSON columns, stores documents and reads them whole, navigates them and
  reads their properties as Krummi types.
- **Queries (module 17)**: will use the navigation and the typed read of this PRD to filter by paths.
- **Maintainer**: runs the tests of the reader and the measurements of JS13 on Windows and on Linux.

## Flow

1. The application creates a table with a JSON column, or adds it to an existing table (JS1).
2. It stores rows with the document as text; Krummi validates it and stores it normalized, or fails without changing
   anything (JS3, JS8).
3. It reads the row and obtains the whole document as text, without copying (JS5).
4. It navigates to a property by its name, its position or a JSON pointer (JS6), and reads it as a Krummi type (JS7).

## Validations and messages

Those of the ancestors apply. This PRD adds, only in the library:

| Situation | Error class |
|---|---|
| Creating a table with a JSON column in the primary key | Invalid schema, rule of E5 of a column of the key that admits nulls, is a vector or has a default value (JS1) |
| Creating an index with a JSON column in the key | Invalid schema, rule of IX3 of a vector or JSON column in the key |
| Changing the type of a column to `json` or from `json` | Invalid schema, rule of E5 of a change that N3 does not admit |
| A default value of a JSON column that is not a valid document, when creating, when changing the schema or when opening | Invalid schema, rule of E5 of a default value that does not meet the rules of its column (JS9) |
| Opening a table with a schema whose JSON default value has another normalized text, or has it where the table does not | Different schema (E6, JS9) |
| Storing in a JSON column a text or some bytes that are not a valid document (JS2) | Invalid value, rule invalid JSON, with the cause, the position and the pointer (JS8) |
| Storing a `string` or `bytes` value in a JSON column, or a JSON value in one of another type | Invalid value, rule type different from that of the column (E17) |
| Storing a null in a JSON column that does not admit them | Invalid value, rule null not admitted (E17) |
| A valid document that makes the row larger than its maximum | Invalid size (E18, JS4) |
| Preparing a JSON column as another type, or one of another type as JSON | Different schema (PY2, JS5) |
| Navigating with a malformed JSON pointer | Invalid value, rule malformed JSON pointer (JS6) |
| Reading as `decimal(p, s)` with `p` or `s` outside the limits of N4 | Invalid value, rule invalid read scale (JS7) |
| Opening a store of format 13 | Earlier format (F11, JS12) |
| Estimating, reading the statistics or analyzing a JSON column, or estimating an equality on several columns that includes it | Invalid schema, rule JSON column (JS10) |

Navigating to something that is absent, or reading as a type a value of another class, is not an error (JS6, JS7).

## Output

The module writes nothing on screen. Reading a JSON column returns null or the document, whose whole text is the
normalized text of JS4, without copying (JS5). Navigating returns JSON values, whose text is a fragment of the
normalized one, without copying, or absent (JS6). The typed read returns the value or null (JS7); the text of a string
or of a property name without escapes, without copying, and with escapes, in a copy owned by the application. Describe
shows the type `json` and the normalized default value (JS1, JS9). The errors carry the data of JS8. The reads of the
other columns, the traversals, the estimates of the other columns and the information of the store do not change their
output.

## Reuse

- The prior validation of the values and the precedence of the errors of 7.1 (E17), with new rules.
- The reads without copying of the variable columns (D11), the projections (PY2, PY3), the included columns (IX8) and
  the rules of a malformed row (L1, L2).
- The rules of vector columns for a type that does not sort: outside the keys (E5, IX3) and without statistics (ST12,
  JS10).
- The rounding and the domains of E8 for the typed read as `decimal`, date and instant.
- The equality of the identical update (E10), the compatibility of schemas (E6) and the maximum size of a row (E18).
- The generator of random values of the tests of the tables for the property tests of the reader (AC-JS11).

## Out of scope

- Updating or removing a property by its path inside the engine: the application reads the document, changes it and
  stores it (annotation of the parent; user: «no en 7.5» (not in 7.5)).
- Reading a property without traversing the text of the document: module 17 decides it with its benchmark load that
  filters by a path, and the stored form allows it without rewriting the rows (annotation of the parent; user:
  «Después, pero que quede anotado» (Later, but let it be noted)).
- Paths as lists of steps in queries, equality by meaning (properties in any order, numbers by their value), filters
  and projections by paths, conditions on the elements of an array and the typed read inside queries: module 17.
- The typed read as the other types of N4 (`i8` to `i32`, `u8` to `u32`, `f32`, `uuid`, `ulid`, `time`, `duration`,
  `bytes`): the application reads the closest type of JS7 and converts it.
- Indexes on JSON (on expressions, multi-entry and inverted): design document 14.
- A binary or canonical stored form, and the compression of documents (7.4, postponed).
- Sorting documents: a JSON is neither a primary key nor a column of the key of an index.
- JSONPath (RFC 9535), JSON5, comments and validating documents against a JSON schema.
- Storing the exact text that arrived, with its whitespace and its escapes: a `string` or `bytes` column is for that.
- Converting a `string` column into `json`, or the other way around (N3).
- Building a document without text, with a constructor of objects and arrays (JS11).
- The C interface and the C# binding, including raising to 128 the depth of .NET when reading the documents (module
  18).
- Importing and exporting documents by lines (module 19) and showing or validating documents from the command-line
  tool (modules 12 and following).

## Left to the implementation

- The exact bytes that are stored for each document, with the starting point of *Stored form v1* of design document 13,
  as long as their text is that of JS4 and the form allows adding later the read without traversing the text without
  rewriting the rows.
- How the reader validates and normalizes, as long as it meets JS2, JS4 and JS8 and neither overflows the stack nor
  takes quadratic time with any text (AC-JS11).
- The English text of the errors (D8); the names of the operations, the types and the rules, in English and with the
  conventions of Rust.
- How the test hooks of the criteria are built: they are test mechanisms, not public contracts.

## Acceptance criteria

Those of the ancestors apply. Each decision is checked by: JS1, AC-JS1 and AC-JS8; JS2, AC-JS1, AC-JS3 and AC-JS11;
JS3, AC-JS3; JS4, AC-JS1, AC-JS2 and AC-JS10; JS5, AC-JS4, AC-JS7 and AC-JS9; JS6, AC-JS4 and AC-JS5; JS7, AC-JS6; JS8,
AC-JS3 and AC-JS10; JS9, AC-JS8; JS10, AC-JS13; JS11, none (it adds no operations); JS12, AC-JS12; JS13, AC-JS14; JS14,
AC-JS11.

1. **AC-JS1** — Given a corpus of documents with each class of value, Unicode of 1 to 4 bytes, all the escapes, the
   numbers `-0`, `0`, `1E400`, `-1e-400`, `123456789012345678901234567890`, `18446744073709551616`, `0.1`,
   `0.10000000000000000000001`, `1.0` and `1`, documents of 128 levels (only objects, only arrays, alternating
   starting with each one, and ten random combinations with a fixed seed), an empty object and array and objects of
   thousands of properties, when each one is stored in a JSON column and read, then the text read is its normalized
   text (JS4), represents the same value as the given one according to the reference model of AC-JS11, with the order
   of the properties and the text of each number, and normalized again gives the same text; and when the store is
   closed and reopened, and in a temporary store, they are read the same.
2. **AC-JS2** — Given the texts ` { "b" : 1.50 , "a" : [ true , "é\/" ] } `, `"A\u0000\u001F\t\""`,
   `"😀"` and an indented document of about 4 KB, when they are stored, then they are read as
   `{"b":1.50,"a":[true,"é/"]}`, `"A\u0000\u001f\t\""`, `"😀"` and the same text as the document without indenting,
   byte by byte.
3. **AC-JS3** — Given each text of this table, when it is inserted, updated and inserted or updated in a write, then
   each operation fails with invalid value, the rule invalid JSON and the cause, the position and the pointer of the
   table; the table, the modification counter (ST4) and the transaction remain as before and the transaction can
   commit its other changes; and when a sequence whose last row brings one of them is bulk loaded, then the load aborts
   entirely with that error (E13). A `string` or some `bytes` in a JSON column fail with type different from that of the
   column, and a null in one that does not admit them, with null not admitted.

   | Text (a byte that is not text, in hexadecimal between `<` and `>`; `\` is always the JSON escape) | Cause | Position | Pointer |
   |---|---|---|---|
   | (empty) | syntax | 0 | `""` |
   | three spaces | syntax | 3 | `""` |
   | `1 2` | syntax | 2 | `""` |
   | `<EF><BB><BF>1` (BOM) | syntax | 0 | `""` |
   | `NaN`, `+1`, `.5`, `'a'` | syntax | 0 | `""` |
   | `01` | syntax | 1 | `""` |
   | `1.` | syntax | 2 | `""` |
   | `[1,]` | syntax | 3 | `""` |
   | `{"a":1,}` | syntax | 7 | `""` |
   | `{"a":1}/*c*/` | syntax | 7 | `""` |
   | `["a` + tab + `"]` | syntax | 3 | `""` |
   | `"\x"` | syntax | 2 | `""` |
   | `"\u12"` | syntax | 5 | `""` |
   | `"\ud800"`, `"\udc00"`, `"\ud800A"` | lone surrogate | 1 | `""` |
   | `<FF>` | invalid UTF-8 | 0 | `""` |
   | `["<E2><28>"]` | invalid UTF-8 | 2 | `""` |
   | `"<E2><82>` | invalid UTF-8 | 1 | `""` |
   | 129 `[` and 129 `]` | more than 128 levels | 128 | `/0` repeated 127 times |
   | `{"a":1,"a":2}` | repeated property, `a` | 7 | `""` |
   | `{"a":1,"b":{"a":2,"a":3}}` | repeated property, `a` | 18 | `/b` |
   | `{"x":{"a/~":[{"k":1,"k":2}]}}` | repeated property, `k` | 20 | `/x/a~1~0/0` |
   | `{"\ud800":1,"\ud800":2}` | lone surrogate | 2 | `""` |

4. **AC-JS4** — Given a table with a JSON column that admits nulls, when a row with the column null and another with the
   document `null` are stored, then the first is read as null and the second as a value of class `null`; and in the
   document `{"a":null}`, navigating to `a` gives a `null` value and navigating to `b` gives absent.
5. **AC-JS5** — Given the document `{"a/b":{"~":[10,20]},"x":[1,2,3],"s":"t","o":{},"e":[]}`, when it is navigated,
   then its properties come back in that order with their name and their value; `/a~1b/~0/1` gives `20`, `""` gives
   the document, `/x/0` gives `1`, and `/x/3`, `/x/-`, `/x/01`, `/x/+1`, `/s/0` and `/nada` give absent; the text of
   the value of `x` is `[1,2,3]` and that of `s`, `"t"`; `x` has 3 elements, `o` zero properties, `e` zero elements and
   the document 5 properties; asking `x` for properties or a property, or `o` or `s` for elements or an element, gives
   absent; and the pointers `a`, `/~2`, `/a~` and `/s/0/~2` fail with invalid value and the rule malformed JSON pointer.
6. **AC-JS6** — Given numbers and strings at the limits of each typed read of JS7 and one beyond (`i64::MIN`,
   `i64::MAX`, `u64::MAX` and the next one in each notation; `decimal(38, s)` with 38 digits and with 39 after
   rounding; the largest finite `f64`, `1e400` and `1e-400`; `1e999999999999` as `i64` and as `decimal`, which takes, in
   the median of five runs, at most 10 times what reading `1e9` takes; dates and instants at the extremes of the domain
   of E8, also when the rounding or the offset take them out; the 29th of February of a leap year and of one that is
   not; the offsets `+23:59`, `-00:00`, `+24:00` and `+00:60`), when they are read, then each one gives the value or the
   null of JS7, and each example of JS7 gives what it says; and reading as `decimal(0, 0)` and `decimal(39, 0)` fails
   with the rule invalid read scale, also on a string.
7. **AC-JS7** — Given a row with a JSON column, when it is read by position, by name and with a projection, then the
   text of the document and that of a string without escapes point to the memory of the row that was read, and when it
   is read as an included column of an index entry, to the memory of that entry, without reading the row, checked as the
   tests of 7.1 and 7.2 check that the text of a `string` column is not copied; a string with escapes and a name with
   escapes are returned in a copy, and a copied row returns the document in a copy owned by the application; and
   preparing the JSON column as `string` or as `bytes`, or a `string` column as JSON, fails with different schema.
8. **AC-JS8** — Given a table definition with a JSON column in the primary key, and an index one with one in the key,
   when they are created, then they fail without effect with invalid schema and their rules (JS1); an index with an
   included JSON column is created and returns it (IX8), and removing that column fails with invalid schema (E5);
   changing the type of a column to `json` or from `json` fails with invalid schema; when a JSON column that admits
   nulls, another with the default value `{"a": 1}` and another with the default value `null` are added to a table with
   rows, then the earlier rows read null, `{"a":1}` and a `null` value, and Describe shows `json` and `{"a":1}`; a
   default value that is not a valid document fails with invalid schema and the rule of E5 of a default value that does not meet the rules of its column (JS9); and
   opening the table with `{ "a" : 1 }` as the default value is compatible, and without a default value in the column
   of `null`, gives different schema.
9. **AC-JS9** — Given a row with `{"a":1,"s":"é"}`, when it is updated with `{ "a" : 1 , "s" : "é" }`, then the
   update is identical (E10) and does not add to the modification counter (ST4); and when it is updated with
   `{"a":1.0,"s":"é"}` or with `{"s":"é","a":1}`, then the row changes and adds.
10. **AC-JS10** — Given a table with an integer key and a JSON column, when the length N of the longest normalized
    text that the column admits is searched, then a document of N bytes is stored and read the same, one of N + 1 fails
    with invalid size, and N is at most 8 bytes smaller than the longest text that a `string` column admits in an
    identical table; and the indented document of N bytes, whose given text exceeds the maximum of the row, is stored
    (JS4).
11. **AC-JS11** — The tests of the reader (JS14), on Windows and on Linux except fuzzing: (1) the JSONTestSuite corpus
    of Nicolas Seriot, where the `y_` are accepted and the `n_` are rejected, each `i_` has a result fixed by a test,
    and the `y_` that Krummi rejects because of a stricter rule are named exceptions with their rule and their error
    (today `y_object_duplicated_key.json` and `y_object_duplicated_key_and_value.json`, repeated property), so that an
    unlisted difference makes the test fail; (2) a reference model of the tests only, with the numbers as text and the
    properties in order, which generates the documents without going through any JSON parser, against what Krummi reads
    from them; (3) a comparison with `serde_json` with its default options on each document of the corpus and generated
    ones, which coincides in accepting or rejecting except the listed differences (repeated keys, depth and numbers that
    `serde_json` does not represent), and in the value of the documents whose numbers it represents exactly; (4) the
    numbers of AC-JS1 outside `f64` and `u64`, stored and read with their text, different from each other and null in
    the typed reads that do not admit them; (5) all the texts up to a fixed length over the characters of numbers
    (`0-9 . e E + -`) against the grammar of RFC 8259, and each `\uXXXX` escape alone and in pairs (paired, lone and
    inverted surrogates); (6) property tests with a fixed seed, with random documents, rewritten with random legal
    whitespace and escapes, which give the same normalized text, and a random byte changed in a valid document, which
    gives an error or another valid document, never a panic; (7) the levels of AC-JS1, accepted with 128 and rejected
    with 129, and validating an object of 1,000,000 distinct properties, and another with one repeated at the end,
    takes, in the median of five runs, at most 20 times what one of 100,000 takes; (8) hand-made mutants of the table
    of escapes, the check of repeated keys, the depth counter and the grammar of numbers, each one detected by some
    test; and (9) fuzzing on Linux, at least one hour per target, of the reader on arbitrary bytes, without panics or
    stack overflows, and of the navigation and the typed read on valid documents, without panics; the closing report
    gives the duration and the number of runs of each target, and the inputs that make something fail are fixed and
    added to the tests of (6).
12. **AC-JS12** — Given a store of format 13, when this engine opens it, then it fails with earlier format (F11); and a
    new store is of format 14.
13. **AC-JS13** — Given a table with a JSON column and other columns, when the JSON column is estimated, its statistics
    are read or it is analyzed, or an equality on several columns that includes it is estimated, then it fails with
    invalid schema and the rule JSON column; and when the table is analyzed, the other columns have the same statistics
    as in an identical table without the JSON column, and the JSON column has none.
14. **AC-JS14** — Given the closing of 7.5, when its report is reviewed, then it includes the measurements of JS13 on
    Windows and on Linux: the pass of W1–W32 before and after, the rounds of the affected loads, and each measure of the
    documents with that of its `string` floor and the ratio between the two.

## Annotations to other PRDs

This PRD changes or makes precise rules of earlier documents, which carry the annotations of 2026-11-21 (in their
translations too):

- Tables, N3 («ninguna de tipo vector» (none of type vector) in the primary key) and N5 («Los vectores no se ordenan:
  no pueden ser clave primaria ni columnas de un índice» (Vectors are not sorted: they cannot be a primary key or
  columns of an index)): they also hold for JSON columns, which can be included columns (JS1). N4: gains the type
  `json` (JS1). *Split*, annotation of 2026-11-12 of 7.5: what «Sigue fijando 7.5» (7.5 still fixes) is fixed in this
  PRD (JS1, JS8, JS12, JS13).
- Tables with schema, E4 and E5: the rule of a column of the key that is a vector includes the JSON column (JS1). E4:
  in a JSON column, the document `null` as a default value is not the same as having none (JS9). E6: two JSON default
  values are the same when their normalized text is the same (JS9). E17: the list of rules of invalid value gains,
  after malformed text, invalid JSON, malformed JSON pointer and invalid read scale (JS6, JS7, JS8); the last two are of
  the navigation and the typed read, never of a row, so they do not compete with the others. E20: the format version
  becomes 14 (JS12).
- Projections, PY2: there are JSON prepared columns, and a JSON column is prepared only as JSON (JS5). PY3: the JSON is
  returned without copying, like text, bytes and vectors, except from a copied row. *Out of scope*: the JSON prepared
  column is fixed by this PRD.
- Statistics, ST1 and *Out of scope*: JSON columns have no statistics, like vector columns (JS10). ST12: the rule JSON
  column goes with the rule vector column. ST18: «the columns that are not vector» also excludes JSON ones.

## References

- Parent: [tables.prd.en.md](tables.prd.en.md) (N3, N4, N5, N11, *Split into child PRDs* with the annotations of 7.5 of
  2026-11-10 and of 2026-11-12, *Output*); root: [krummi.prd.en.md](../krummi.prd.en.md) (D8, D11, D12, G9).
- Siblings: [tables-schema.prd.en.md](tables-schema.prd.en.md) (E4, E5, E6, E7, E8, E10, E11, E13, E17, E18, E20),
  [tables-row-reads.prd.en.md](tables-row-reads.prd.en.md) (L1, L2), [tables-indexes.prd.en.md](tables-indexes.prd.en.md)
  (IX1, IX3, IX8), [tables-statistics.prd.en.md](tables-statistics.prd.en.md) (ST1, ST4, ST12, ST18),
  [tables-projection.prd.en.md](tables-projection.prd.en.md) (PY2, PY3).
- Trees and transactions: [trees.prd.en.md](../trees/trees.prd.en.md) (B9, B12),
  [transactions.prd.en.md](../transactions/transactions.prd.en.md) (T10).
- Design: [document 13](../../../design/tables/13-json-column-storage.md) (technical starting point) and
  [document 14](../../../design/tables/14-json-indexes.md) (indexes on JSON).
- Standards: RFC 8259 (JSON), RFC 6901 (JSON pointer), RFC 3339 (dates and instants).

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| 6ce1b30c | 2026-11-21 06:26 UTC+00:00 | 48 min | 0 min | 48 min | 45 min | 3 min | 4 |

<!-- prd-time: [{"session": "6ce1b30c-0c7f-475e-a6c5-b31016b08203", "start": "2026-11-21T06:26:46.585000+00:00", "wall": 2881, "discarded": 0, "agent": 2692, "human": 189, "interventions": 4, "idle_minutes": 10}] -->
