# Tree append

Objective: the key-value API's `set` and `try_add` add a key greater than every key of a normal tree at the end of its
remembered last leaf, without walking the tree, as tables already do. Branch `feat/tree-append` from
`feat/night-optimizations` (`504cd84`, code as `d96b9c2`); delivery `ask-on-risk`; tests `cargo test --workspace`. No
PRD: nothing observable changes.

## Specs

- **S1** — The remembered last leaf, as the user put it: «optimicemos try_add, el primer try_add calcula la última clave
  y hoja y se recuerda, los siguientes try_add usan esa información. Los set y remove hacen que se olvide lo
  recordado.» Extended by the user's choice (L2): `set` uses it too, because a `set` of a key greater than every key
  is an insert. Every other change of the tree forgets it: a `set` or `try_add` that does not add at the end, `remove`,
  the range removals, emptying or deleting the tree, and an operation that fails.
- **S2** — No behavior change: the tree's every node byte for byte, its root and its count equal those that walking
  the tree for every operation leaves. `krummi_tree::update` is not changed (the cost of the ordered-write cache,
  `odd/done/trees/ordered-write-cache-report.md`).
- **S3** — Acceptance, the rule of S5 of `odd/done/pager/night-optimizations.md`: «No merece la pena optimizar W1 si W3,
  W5 y W7 empeoran», by Q7's rule in an A/B of Krummi alone against the code before it on Windows (at least ten
  rounds for loads under one second per repetition or with several threads, three for the others; a marked load is
  repeated in twenty rounds), with the exception agreed on 2026-11-18: W3 and W25 may lose up to about 2 % when the
  usual loads gain. Added from that run's L20: W12 and W13 measured in the complete run, not only in the A/B.
- **S4** — Closing: complete runs (Krummi and Voron) on Windows and WSL 2 on the final code if the change stays; the
  Codex review and the closing report.

## Tasks

| ID | Specs | Route | Status | Evidence |
|---|---|---|---|---|
| T1 | S1, S2 | inline | done | RED/GREEN `append_tests` (3 tests); `setting_a_new_key_reports_the_last_leaf_as_adding_it_does`; L4 |
| T2 | S3 | inline (background runs) | pending | |
| T3 | S4 | inline | pending | |

## Log

- **L1** (2026-11-18) — User, after the tables' fast path was explained: «Podríamos tener append_last y try_add_at_last
  en los árboles y usar esto desde la tablas. No sé por qué lo tenemos sólo en las tablas.» Then «¿No tenemos
  operación insert en los árboles?», then «Pues qué tal esto: optimicemos try_add, el primer try_add calcula la última
  clave y hoja y se recuerda, los siguientes try_add usan esa información. Los set y remove hacen que se olvide lo
  recordado.», then «update es otra operación a parte de set, try_add y remove?»
- **L2** (2026-11-18) — User: «¿Qué recomiendas? una comparación más se notaría mucho? compensaría? las inserciones
  ordenadas serían más rápidas, en una app normal suele haber muchas más inserciones que actualizaciones.»
  Recommended: `set` too (the bench writes with `set`, engines/krummi.rs:149; the check costs a few ns against 1–2.5
  µs per operation; the earlier cost of W3 came with changes inside `update`, which this change leaves alone). User:
  «Sí». Estimate «Entre 4 h y 6 h», confirmed «Sí»; start 11:14:46 (`date`).
- **L3** (2026-11-18) — Map, read inline. `update` already fills `last` for `Op::Set` when a new key goes to the end
  of the rightmost leaf (`crates/krummi-tree/src/lib.rs:1127-1131`) and clears it on any split, merge or dropped child
  (`:970-972`, `:1049`, `:1057-1059`, `:1063`), so `set_at_last` is a wrapper like `try_add_at_last` (`:632`).
  `append_last` checks that the key is greater than the leaf's last (`:720`), so the API needs no greatest key of its
  own. The API's per-tree state is `TreeState` (`crates/krummi/src/lib.rs:480`), changed by `apply`,
  `apply_removed`, `empty` and `delete` (`:1306-1395`). Tables keep `last` with the root it belongs to and forget it
  on any other change (`crates/krummi/src/table/rows.rs:1444`, `:1509`). Design: `TreeState` gets
  `last: Option<(u64, LastLeaf)>`; `set` and `try_add` take it out, try `append_last` when the root matches, and
  otherwise call `set_at_last` or `try_add_at_last` and keep what they return; a `try_add` of a key that is there
  changes nothing and keeps it; everything else leaves it out.
- **L4** (2026-11-18) — T1 inline. `krummi_tree::set_at_last` wraps `update` with `Op::Set` as `try_add_at_last` does
  with `Op::Add`; `update` is unchanged. `TreeState.last` holds the last leaf with its root; `put` (shared by `set`
  and `try_add`) takes it out, appends when the root matches and `append_last` accepts, and otherwise walks with
  `set_at_last` or `try_add_at_last` and keeps what they report. `remove`, the range removals, `empty` and `delete`
  forget it. Decision without asking: a `try_add` of a key that is there forgets the leaf too, as tables do, rather
  than keeping it (S1 lists only appends as keeping it). RED observed before the code: the three tests of
  `crates/krummi/src/append_tests.rs` failed with nothing remembered; the tree-level test of `set_at_last` could only
  fail to compile before the wrapper (no behavior of its own). GREEN after. Checks: fmt and clippy clean, `cargo test
  --workspace` 873 passed, 0 failed. Risk: item 6 covered (the equivalence tests pin the tree's bytes, root and count
  against walking).
