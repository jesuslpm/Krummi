# Pager — ODD execution document

Objective: implement module 1 of MuninnDB, the Pager, as specified in `docs/prd/muninndb/pager.prd.md` (v1.0) and
its ancestry `docs/prd/muninndb/muninndb.prd.md` (v1.1). Branch: `feat/pager`. Delivery: `ask-on-risk`.
Test runner: `cargo test --workspace` (Windows) and the same in Ubuntu 24.04 on WSL 2 (Linux).
Estimate: 3–5 h. Real start: 2026-10-22 11:30:12 +0100.

## Specs

The PRD is authoritative; every S# quotes or names its PRD items, and workers read the PRD sections it names.

- **S1 — Public surface (P1, P12, P14).** "abrir o crear un almacén (en una carpeta o en modo temporal),
  consultar su información y cerrarlo. Reservar, leer y escribir páginas es interno de la biblioteca". Info
  returns format version, page size, size in bytes, page count (header pages included), effective max size or
  none, temporary flag. Close always releases the lock; a failed flush returns the I/O class.
- **S2 — Format (P2–P6, D10).** 8 KiB pages; single `muninn.data`; identification mark at the start, outside the
  header copies; current format `1`; "Un almacén íntegro con un número menor que el vigente es de formato
  anterior, también con `0`"; two header copies with integrity check, the newest intact copy wins, one damaged
  copy opens silently, both damaged → Damage class.
- **S3 — Folder rules and open order (P7, P8, parent order of checks, Flujo).** Options first ("un tamaño máximo
  menor que 1 MiB falla con la clase de opciones no válidas sin tocar nada"); then the parent order with the
  Pager precisions; foreign files ignored in a store folder; a folder without `muninn.data` and with any entry is
  "No es un almacén", except a folder whose only content is the file `muninn.data.new`, which is treated as empty
  and replaced. Creation via `muninn.data.new` + rename, durable when open returns; failed creation deletes
  `.new`; intermediate folders are kept. Validation table rows of the Pager PRD.
- **S4 — Lock (P9, D6).** Store-level exclusive lock from order step 2 to close, also during creation; released
  by the OS on process death; no extra files; another path to the same store also fails with "Store in use";
  dropping without close releases it.
- **S5 — Max size and growth (P10, P11).** Max ≥ 1 MiB, rounded down to 8192, per open, both modes; growth
  clamps to the max and fails with the Maximum size class only when the pages do not fit; an existing larger
  store opens and cannot grow. New store ≤ 1 MiB; size always a multiple of 8192; steps never smaller than the
  previous one and increasing; real space reserved on growth; failed growth keeps the size; an incomplete final
  page is ignored on open.
- **S6 — Pages, zero copy, unsafe (P15, P16, P17, D11, D12).** Page reads return references into the mapping
  (or temporary memory) without copying, valid while the store is open (compile-enforced); growth does not
  invalidate them; integrity checked on read (G6); `unsafe` only in the Pager crate, with a SAFETY condition per
  use; an automatic check fails on unsafe elsewhere; a physical read fault on the mapping terminates the process.
- **S7 — Errors (P13, Validaciones, parent table, G7, G12).** Classes: Access, Store in use, Not a store, Damage,
  Different format, Disk full, Input/output, Invalid options, Maximum size, Out of memory; each error carries its
  data (folder, versions, page number, cause). Disk full and I/O failures keep the store usable as the PRD says;
  after a write I/O failure, writes are refused until reopen.
- **S8 — Acceptance.** AC-P1…AC-P17 and the adapted parent criteria AC8, AC9, AC10, AC11, AC12, AC14, AC16,
  AC18, AC20, AC21, as written in the Pager PRD's *Criterios de aceptación*.

## Defaults taken (no product impact; listed in the closing report)

- Workspace with two crates: `muninndb-pager` (the only crate with `unsafe`) and `muninndb` (public facade,
  `#![forbid(unsafe_code)]` through workspace lints). Dependencies: `memmap2`, `zerocopy`, `xxhash-rust` (xxh3
  checksums), `rustix` on Linux (space reservation); dev: `tempfile`, `trybuild`.
- New store: 64 KiB (8 pages: page 0 = mark + header copy A, page 1 = header copy B). Growth step = current
  size (doubling), capped at 1 GiB per step, clamped to the max.
- Writes go through positioned file writes (errors are reported, G12); reads go through the mapping.
- Internal page writes take `&mut` (no reference can be alive); growth takes `&self`; old mappings live until
  the store closes.
- Temporary mode stores pages in heap chunks allocated with fallible allocation.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S6 (unsafe check), S7 | inline | pending | |
| T2 | S2 | inline | pending | |
| T3 | S1, S3, S4, S7 | inline | pending | |
| T4 | S5, S6, S7 | inline | pending | |
| T5 | S3, S7, S8 (crash, power cut, fault injection) | inline | pending | |
| T6 | docs | inline | pending | |

Route evidence: the units depend on each other (T2 → T3 → T4 → T5), so no Writer trigger holds; work inline.

## Log

- **L1** (2026-10-22, user, verbatim): "Implementa el Pager"
- **L2** (2026-10-22, user, verbatim, before the PRD was final): "Una cuestión el motor tiene que ser zero copy
  unsafe permitido en el pager." and "ChatGPT recomendaba memmap2 + zerocopy"
- **L3** (2026-10-22): estimate 3–5 h confirmed after a question about compression (deferred to modules 5 and
  7, no effect on the Pager). Start 11:30:12 +0100.
