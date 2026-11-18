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
| T2 | S3 | inline (background runs) | done (marks accepted by the user, L7) | A/B L5; repeats stopped L6 |
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
- **L5** (2026-11-18) — S3 A/B on Windows 11:27:44–12:12:17 (`work/ta/ab.sh`; `not2` = `5a00edc`, the base's code,
  `app` = `bf4b998`; power plan on). Ratios `app`/`not2` in op/s, with the rounds where `app` was slower: W1 1.482
  (0/3), W14 1.013, W23 1.014, W29 1.012, W25 1.009 (0/3), W2 1.004, W15 1.008, W19 1.007, W5 1.001, W22 1.005, W27
  1.008. Marked (median lower, slower in most rounds): W3 0.978 (3/3), W4 0.973 (3/3), W17 0.903 (2/3), W21 0.993
  (3/3), W24 0.981 (2/3), W26 0.985 (3/3), W28 0.999 (2/3), W6 0.998 (7/10), W7 0.978 (9/10), W12 0.946 (6/10); W13
  0.970 (5/10) not marked. W7 and W26 only read, and W17's values are off their leaf, so `append_last` declines
  before reading a page. The marked loads are repeated in 20 rounds (`work/ta/rep.sh`).
- **L6** (2026-11-18) — The 20-round repeats started at 12:12:37 and were stopped at the user's request at 12:26: «No
  hagas más rondas.» No repeat finished; the bench workers were killed, the power plan restored and the leftover run
  stores removed. The run waits for the user's decision on the marks of L5 (wait from 12:26:12).
- **L7** (2026-11-18) — Recommended keeping the change (W1 +48 %; the marks are small, W7 and W26 only read, W17's
  `append_last` declines before reading a page; W12 is checked by the complete run). User: «Sí» (the wait's end is in the phase log). T2 done with the marks of L5 accepted by the user; complete runs next (T3).
- **L8** (2026-11-18) — S4 complete runs on `605d0ff` (code `bf4b998`; script `work/ta/full.sh`). Windows
  12:28:39–12:50:43: **Krummi cumple 29 de 29 umbrales**; carga-en-orden 1,295,570 against Voron's 677,189 op/s,
  1.91 (1.38 and 1.45 in the night's runs of the base's code); lecturas-con-escritor 0.99 (Krummi 1,433,150, Voron
  1,441,159: both engines low in this run), escritor-con-lectores 1.11. WSL 2 12:52:01–13:37:55: **Krummi cumple 29 de
  29 umbrales**; carga-en-orden 150,562 op/s, 1.08 (the night's run on the base's code: 154,783, 1.09), so on Linux,
  where the load runs at about a tenth of Windows' rate, the walk it saves does not show; W12 1.20, W13 1.16. Reports
  in `work/ta/full/` (ignored by git).
