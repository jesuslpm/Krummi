# Compression of row values by dictionary: technical design

Part of the [tables design documents](README.md). Module 7 of the root PRD lists "compresión de valores por
diccionario" ([krummi.prd.md](../../prd/krummi/krummi.prd.md) line 216). This document says what Voron does, what Krummi
could do, and whether the first PRD children of module 7 should include it. Reconciled 2026-11-09 with the user's
decisions (README) and the [fact-check](verification.md): the answer is deferred, with the hooks placed in document 1.

## Summary

- Voron compresses each table row with zstd (level 3) and a per-table trained dictionary, only for tables whose schema
  asks for it, and keeps every dictionary forever so old rows stay readable.
- Krummi has no pure-Rust zstd encoder with dictionary support: `ruzstd` decodes, and has a dictionary-less encoder
  usable only at its `Fastest` level, its dictionary encoding is on the roadmap and its `dict_builder` makes only
  raw-content dictionaries. The `zstd` crate binds C, which breaks the C-free library build (R2).
- `lz4_flex`, already a dependency of the pager, supports dictionaries in its block API, and `fsst-rs` is a pure-Rust
  option for strings but is not production-ready.
- **Decided (2026-11-09):** compression by dictionary is deferred out of the first children of module 7, with its hooks
  reserved in the format now: a compressed flag and a dictionary id in the row header, and a tree reserved for
  dictionaries. When built: **LZ4 with a dictionary, opt-in per table, whole row**; before building it, confirm that
  enabling `lz4_flex`'s dictionary support does not switch the journal's encoder to its slower safe implementation
  (below). User: "Sí".
- The hooks are in one place, agreed with [document 1](01-row-format-and-types.md): bit 2 of the row header's `flags`
  byte and, when it is set, a dictionary id right after the 3-byte header.

## Requirements

1. R1: Reads stay zero-copy for uncompressed rows; a compressed row is decompressed once per transaction into memory of
   that transaction and the reference points there (D11, root PRD line 51).
2. R2: No `unsafe` in Krummi's own code outside the pager (D12) and no C toolchain in the library build.
3. R3: A row written with dictionary N is readable as long as any version of it is visible to a snapshot, across
   restarts and after dictionaries change.
4. R4: Compression never changes semantics: index keys, primary keys and the order of keys are never compressed.
5. R5: Compression is opt-in per table; random or already-compressed data must cost almost nothing (a failed attempt is
   stored raw).
6. R6: Crash safety: a dictionary is durable in the same commit as the first row that names it.

## What Voron does

Source: [TableValueCompressor.cs](file:///C:/projects/ravendb/src/Voron/Data/Tables/TableValueCompressor.cs) and
[Table.cs](file:///C:/projects/ravendb/src/Voron/Data/Tables/Table.cs).

- **Opt-in per table.** A schema must call `CompressValues(etagSource, true)`; it needs a fixed-size etag index, used as
  the training order (TableSchema.cs lines 75-82). Insert and update call `TryCompression` only if the schema is
  compressed (Table.cs lines 361, 720).
- **Whole row, zstd level 3.** The entire serialized row (`RawBuffer`) is compressed with `ZstdLib.Compress` and the table's
  current dictionary (TableValueCompressor.cs line 80; level 3 in Table.cs line 872). The dictionary id is appended as a
  variable-length integer written in reverse, so the reader finds it from the end (lines 81, 133-153; Table.cs lines
  322-336). Overhead reserve is 32 bytes (line 27).
- **When a value stays compressed.** If the compressed size is not smaller than the raw size, compression is dropped and the
  row is stored raw (lines 93-102). A `Compressed` flag lives in the raw-data entry (and in the page flags for large
  values), so the reader knows which form it has (Table.cs lines 383, 428-431, 2569-2586).
- **Dictionary id 0 is "no dictionary"**: plain zstd without a trained dictionary (Table.cs line 865).
- **Retraining trigger.** Each dictionary stores the compression ratio it achieved (`CompressionDictionaryInfo`, one byte:
  CompressionDictionaryInfo.cs line 8). If a new row compresses more than 10 percentage points worse than that expected
  ratio (line 84) and the newest etag is a multiple of 1024 (lines 115-126), training runs. So training is rare, amortized,
  and inside a write transaction.
- **Training.** `MaybeTrainCompressionDictionary` walks the etag index from newest to oldest, collecting up to 256 rows or
  1 MiB, skipping rows larger than 32 KiB and giving up after 16 Ki skipped ones; fewer than 16 samples abort (lines
  155-194). It calls zstd's trainer with a dictionary buffer of at most one page (line 209).
- **Accepting a new dictionary.** The new dictionary replaces the current one only if it compresses the current row at least
  10% smaller than the present dictionary does (lines 421-455). Id is `entries + 1`. It is stored in a tree
  `CompressionDictionaries` keyed by big-endian id, value = info byte + dictionary bytes (lines 217-239). The table's
  current id is stored in the table tree (Table.cs lines 77-87).
- **Old dictionaries.** Nothing deletes a dictionary: ids only grow, and any row names the id it was written with, so old rows
  decompress with their own dictionary. Updated rows are recompressed with the current one.
- **Cache and rollback.** A process-wide concurrent map caches loaded dictionaries (Table.cs lines 815-831). On rollback, the
  cached new dictionary is removed, because using one that was never committed would corrupt data (TableValueCompressor.cs
  lines 242-261, RavenDB-17758).
- **Recovery copies.** After commit, two zip files outside the data file keep all dictionaries (encrypted when the store is),
  so a damaged dictionary tree does not make every compressed row unrecoverable (lines 274-419).
- **Read.** `DecompressValue` allocates the decompressed size (stored inside the zstd frame) from the transaction allocator
  and decompresses there; no cache of decompressed rows beyond that (Table.cs lines 322-336).

## Options compared

| Option | Ratio on structured rows | Speed | Fit with D12/R2 | Notes |
|---|---|---|---|---|
| zstd + trained dictionary (C `zstd` crate) | Best (small rows gain 2-5x) | Compress ~hundreds MB/s, decompress ~1 GB/s | Fails R2: it needs a C compiler in the library build (its FFI is also `unsafe` inside the dependency, which the policy of doc 4 question 5 would allow case by case) | Voron's choice; training built in |
| `ruzstd` (pure Rust) | Decode only for dictionaries | Slower decoder | Compatible, but dictionary encoding is roadmap (its encoder is usable at `Fastest` only); it can build raw-content dictionaries behind `dict_builder` only | Not usable to compress with a dictionary today |
| LZ4 block + dictionary (`lz4_flex`) | Good (2-3x on repetitive rows), below zstd | Fastest | `compress_with_dict`, `decompress_with_dict` and their `_into` variants exist in 0.12.0 (the version Krummi pins) and 0.14.0; decode is available with `safe-decode`, already enabled; `decompress_with_dict` needs the uncompressed size from the caller | No trainer: the dictionary is a byte string of sample content, only its last 64 KiB is referenced; Krummi must pick it (concatenate representative rows). Feature unification risk: see below |
| FSST (`fsst-rs`) | Good on strings (comparable to LZ4), random access per string | 1-3 GB/s claimed | Pure Rust; README says not production-ready, little-endian only; version 0.7.0 has 88 uses of `unsafe` and no `forbid(unsafe_code)` | Per-column, for text columns; symbol table of at most 255 symbols (from the FSST paper); needs a column type that is text |
| Per-column vs whole-row | Per-column wins on text and vectors cannot gain; whole-row catches repetition across columns | Per-column lets a read decompress one column | Per-column needs the offset table to point into compressed segments | See below |
| No compression | 1x | Zero cost | Trivially fine | Fully zero-copy |

Training: the zstd trainer (COVER/fastCOVER) has no Rust implementation except `ruzstd`'s raw-content builder. For LZ4,
a "dictionary" of 16-64 KiB of representative rows chosen by a simple sampler is enough for the block format (LZ4 only
references the last 64 KiB), so a custom trainer is a few dozen lines.

## Decided design

**Decision (2026-11-09).** Defer dictionary compression: it is not in the first children of module 7 (it is child 7.4
of the split, deferred, with its hooks in 7.1). Its value depends on workloads the benchmark does not have, it needs a
stable row format and index design first, and it adds a lifecycle (training, dictionaries, compaction) that the first
tables do not need. Reserve the hooks now (below) so that the PRD for it can be written when a user workload asks for it.

**Hooks to reserve in the first children (cheap, no behavior):**

1. **In the row header, one place** (document 1, "Compression hook"): bit 2 of the `flags` byte is "compressed". When
   it is set, the schema version and flags stay readable, and right after the 3-byte header come `dictionary_id`
   (varint; 0 is plain LZ4 with no dictionary), `raw_len` (varint, the uncompressed size of the body, which
   `lz4_flex::decompress_with_dict` needs) and one LZ4 block holding the body: null bitmap, fixed section, end-offset
   table and variable section. The first draft of this document put the id after the offset table, read from the end like
   Voron; that is dropped, because the id sits at a fixed place at the front, the offset table lives inside the
   compressed body, and nothing has to be appended to a row to name its dictionary (contradiction 9 of the fact-check).
   Until compression is built the bit is always clear.
2. A per-table `meta` record able to hold a "compression" setting (none by default) and a "current dictionary id".
3. A reserved tree for dictionaries, `dict`, among the table's trees (document 2), keyed by big-endian id, empty and not
   created until used. Dictionary ids are per table, as in Voron, so the tree is dropped with the table.

**Design when implemented:**

- **Granularity.** Whole row (decided). Compress the row body (everything after the 3-byte header); the offset table is
  readable only after decompression. Per-column compression would keep point lookups of one column cheap but defeats
  zero-copy of the other columns' offsets and complicates the row format; a table can instead keep large text or blob
  columns in a separate column group later. Vector columns and integer columns are not compressed on their own.
- **Algorithm.** LZ4 block with dictionary (`lz4_flex`), decode in safe mode as in the journal (decided). It is pure
  Rust and fast. **Before building it, confirm the feature setting.** The pager enables `lz4_flex` with
  `default-features = false` and only `safe-decode` and `checked-decode` (`crates/krummi-pager/Cargo.toml:17`), so its
  encoder runs in the non-`safe-encode` implementation that the D12 annotation of 2026-11-06 accepts (30 % to 50 %
  faster on pages of a real store). A table-layer encoder outside the pager that needed `safe-encode` would, by Cargo's
  feature unification, switch the journal's encoder to the safe, slower one. Check with `cargo tree -e features`
  whether the dictionary functions need `safe-encode` at all (not run here, no cargo command was allowed in this
  reconciliation). If they do, the alternative is to expose a dictionary compress function from the pager crate, which
  already owns the fast encoder. Dictionary format: id (u32), a short header (ratio achieved, sample count) and up to
  64 KiB of sample content. A zstd backend is a later optional feature behind the same dictionary id, if a safe encoder
  appears.
- **When a row is compressed.** Only for tables that opt in (decided: default off), rows of at least 128 bytes, and only
  if the compressed result (including the id and length) is at least 10% smaller than the raw row; otherwise stored raw
  with the flag clear. Rows above a threshold (for example 64 KiB) use no dictionary (dictionary id 0: plain LZ4), since
  they compress well alone.
- **Reads.** A raw row is returned zero-copy as today. A compressed row is decompressed once per transaction into a
  transaction arena keyed by the row's primary key; a second read of the same row in that transaction returns the same
  reference (D11). Queries that scan a table decompress each row once; if the compressed fraction is high, scans cost
  1-2 GB/s of LZ4 decode, rarely the bottleneck. No cross-transaction cache of decompressed rows in the first version
  (it would require invalidation under snapshot isolation).
- **Keys and indexes.** Never compressed (R4). Keys are compared bytewise in tree pages (the tuple encoding of doc 3);
  compressing them would break order and zero-copy comparisons. A secondary index entry holds `tuple(columns) ‖
  tuple(primary key)` as its key and the included (covering) columns as its value, in document 1's row format: it holds
  no copy of the row, and its included values stay uncompressed, so compressing table rows does not affect index scans.
  Only the table tree's row values are compressed. (Document 3's covering columns made the first draft's phrase "key plus
  row address" incomplete; this is the reconciled statement, contradiction 15.)
- **Journal.** The journal compresses each record with LZ4 (see [journal-page-diffs.prd.md](../../prd/krummi/journal-page-diffs.prd.md)).
  Compressed rows reach it as already-incompressible bytes; it already falls back to raw when LZ4 does not shrink the
  record (`journal.rs:347-356`), so double compression wastes only a failed attempt. A journal that stores page diffs
  gains less from compressed rows whose bytes change completely on update; this is a reason to keep compression opt-in.
- **Training lifecycle.** Follow Voron's shape, simplified: (a) train only when the table has at least 32 rows and no
  dictionary, or when the last N inserted rows compress more than 10 points worse than the ratio recorded with the current
  dictionary, checked every 1024 inserts; (b) sample the newest rows up to 256 rows or 1 MiB, skipping rows over 32 KiB; (c)
  accept the new dictionary only if it improves the current row by at least 10%. Voron orders its samples by an etag
  index; Krummi has no etag, so the sampler reads the tail of the primary key range or a stride over the table tree.
- **Storage.** The `dict` tree: value = header + bytes. The dictionary is written in the same transaction as the first
  row that names it (R6), so the pager's commit atomicity gives crash safety without extra files. The in-memory cache is
  part of the committed state and is discarded on rollback (the Voron bug RavenDB-17758 shows why). Voron's external
  recovery zips are not reproduced: Krummi's journal and checksums already protect pages.
- **Never delete a referenced dictionary.** Rows name their id; dictionaries are never deleted in the first version.
  Because a dictionary is at most 64 KiB and training is rare (a few per table over its life), the space is negligible.
- **Compaction.** A table rewrite (offline or a later online compaction) recompresses rows with the current dictionary and
  may then drop dictionaries no row references, determined by a full scan during that rewrite.
- **Workloads that gain.** Rows with repeated structure: JSON or text columns, repeated field names, enumerations in strings,
  timestamps and sequential ids, similar URLs. Rows of random bytes gain nothing: the attempt fails the 10% rule and the
  row is stored raw, costing one wasted compression per write. The current benchmark values are random bytes (as stated
  in the task), so it would show only cost; a compression benchmark needs a structured corpus (for example generated JSON
  documents or log lines).

## Costs and risks

- **CPU on write.** An LZ4 attempt per insert/update (hundreds of MB/s) on a table that opts in; unavoidable failure cost on
  incompressible data. Mitigation: skip after K consecutive failures (adaptive backoff), re-test on retrain.
- **CPU on read.** Decompression for each compressed row first read in a transaction; no benefit for point reads of small
  rows (LZ4 on 200 bytes saves little). Zero-copy is lost for those rows.
- **Memory.** Decompressed rows live for the whole read transaction: a long scan over a compressed table grows the arena.
  Needs a bound or per-batch release; open question for the design of queries (doc 4).
- **Dictionary quality.** An LZ4 sample dictionary is cruder than a zstd-trained one; ratios will be lower. If they are not
  good enough for the target workloads, the only strong alternative is zstd, which needs a C dependency or a new
  pure-Rust encoder.
- **Format lock-in.** Row header and dictionary id become part of the on-disk format; hence reserving them now.
  Reserving them keeps existing rows readable without a rewrite and the layout unchanged; it does not make enabling
  compression compatible (added 2026-11-10 after an external review). An engine from before compression cannot
  decompress a compressed row, so 7.4 declares under B12 whether its change is incompatible, and with the doubt rule
  of B12 it most likely is. 7.1 makes that case safe: a reader that finds the compressed bit set, or a dictionary id,
  without support for them fails with a typed error instead of reading the body as uncompressed.
- **Corruption blast radius.** A damaged dictionary makes every row that names it unreadable. Mitigation: checksum of the
  dictionary, and verifying round-trip (decompress and compare) when a dictionary is first accepted.
- **Interaction with updates.** Krummi rewrites the row on every update (a changed leaf cell, no in-place write as in
  Voron's `TryWriteDirect`), so a compressed row whose size changes costs nothing extra beyond the recompression; the
  stored size is the compressed size (doc 2).
- **Dependencies.** `fsst-rs` is explicitly not production-ready and little-endian only; do not adopt it before it matures.
  `lz4_flex` is "pure Rust, safe, already audited in this repository" only in part: the pager runs its encoder in the
  `unsafe`-using mode under the D12 annotation of 2026-11-06, and decodes in safe, checked mode.

## Product questions

1. **Include dictionary compression in the first children of module 7?** *Decided 2026-11-09:* defer, reserving the
   format hooks now (README: "Compression of values by dictionary is deferred out of the first children ... User: Sí").
2. **Algorithm.** *Decided:* LZ4 with a dictionary, pure Rust; the feature-unification check above comes first.
3. **Granularity.** *Decided:* whole row.
4. **Default.** *Decided:* opt-in per table, off unless the table asks for it.
5. **Do we want a structured-data benchmark corpus** (JSON, logs) to measure ratio and cost, since the current random
   values cannot? *Still open, for the PRD of 7.4.* Recommendation: yes, as an acceptance criterion rather than a promise.
6. **Compaction.** *Still open, for the PRD of 7.4.* Is a table rewrite that recompresses rows and drops unused
   dictionaries in scope, or are dictionaries kept for the life of the store? Recommendation: keep them for the first
   version; compaction is a later module.

## Sources

- C:\projects\ravendb\src\Voron\Data\Tables\TableValueCompressor.cs: lines 27, 67-113 (TryCompression), 84-91 and 115-126
  (retrain trigger), 133-153 (reverse varint), 155-270 (training), 209 (dictionary size), 217-239 (storage), 242-261
  (rollback), 274-419 (recovery files), 421-455 (replacement rule).
- C:\projects\ravendb\src\Voron\Data\Tables\CompressionDictionaryInfo.cs: line 8.
- C:\projects\ravendb\src\Voron\Data\Tables\Table.cs: lines 77-87, 322-336 (DecompressValue), 361, 720, 815-879
  (dictionary holder; line 865 id 0), 2569-2586.
- C:\projects\ravendb\src\Voron\Data\Tables\TableSchema.cs: lines 21-22, 75-82, 172.
- C:\projects\Krummi\docs\prd\krummi\krummi.prd.md: lines 51 (D11), 52 (D12), 93, 216.
- C:\projects\Krummi\crates\krummi-pager\Cargo.toml line 17 (`lz4_flex`, `safe-decode`, `checked-decode`);
  C:\projects\Krummi\crates\krummi-pager\src\journal.rs lines 277, 347-356. (The first draft cited the root
  `Cargo.toml` line 13 for `lz4_flex`; the root manifest has no `lz4_flex`.)
- https://github.com/KillingSpark/zstd-rs (ruzstd: roadmap lists encoder dictionaries; `dict_builder` makes raw-content
  dictionaries only).
- https://docs.rs/lz4_flex/latest/lz4_flex/block/index.html (`compress_with_dict`, `decompress_with_dict`, and the `_into`
  variants).
- https://github.com/spiraldb/fsst (fsst-rs: pure Rust, not production-ready, little-endian only, Apache-2.0).
