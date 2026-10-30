# Remembered node layout check — execution (ODD)

Implements [trees-node-check-cache.prd.md](../../docs/prd/krummi/trees-node-check-cache.prd.md) (decisions C1–C9,
authoritative in Spanish). Branch `feat/trees-node-check-cache`. Delivery: local work-unit commits on the branch,
no push or merge (user's decision). Test runner: `cargo test --workspace`.

## Specs

The PRD's decisions C1–C9 and its *Dejado a la implementación* bound every task. Acceptance criteria, verbatim; their
common conditions: "Valen los de los ascendientes, que deben seguir pasando sin cambios, incluidos AC10 del padre raíz, AC-M15 y el resto de pruebas de daño de los árboles. Cada criterio se comprueba en un almacén en disco y en modo temporal, salvo las partes que reabren el almacén, que son solo en disco. Las pruebas usan el conocimiento de la implementación para contar las comprobaciones de organización, para colocar páginas en el archivo de datos o en el área temporal, para alterar bytes y para elegir el tamaño de la memoria de nodos recordados."

- **S1** — **AC-C1** — Dados un árbol normal, el catálogo y el subárbol de una clave de un árbol multivalor, con sus páginas en el archivo de datos y sitio para recordar todos los nodos que lee la prueba, cuando una transacción de lectura lee la misma clave o el mismo valor varias veces y otra transacción de lectura lo lee después, entonces todas devuelven lo mismo, la organización de cada nodo del camino se comprueba solo en la primera lectura de ese nodo y su integridad en todas (C1, C2, C4).
- **S2** — **AC-C2** — Dado un nodo recordado, cuando una escritura cambia su contenido y se confirma mientras una transacción de lectura abierta antes sigue leyéndolo, entonces esa transacción sigue viendo el contenido anterior; y cuando esa transacción se cierra y la nueva versión se traslada al archivo de datos en el mismo número de página, entonces una transacción de lectura nueva ve el contenido nuevo y comprueba su organización una vez (C1, C2, C8, G3).
- **S3** — **AC-C3** — Dada una página de un nodo recordado que se libera y una escritura posterior reutiliza para otro nodo con otro contenido, cuando ese nodo nuevo llega al archivo de datos y se lee, entonces se lee su contenido y su organización se comprueba una vez (C1, C2).
- **S4** — **AC-C4** — Dada una página de un nodo en el archivo de datos con la organización dañada y el código de integridad recalculado para que sea correcto, cuando se lee dos veces en la misma transacción de lectura y una vez en otra, entonces cada lectura falla con la clase de daño y el número de esa página, y una lectura posterior la comprueba otra vez; también cuando antes se había recordado otra versión correcta de esa misma página (C1, C2, A9).
- **S5** — **AC-C5** — Dada la misma página dañada de AC-C4, pero en el área temporal (confirmada y sin trasladar), cuando se lee dos veces, entonces falla igual las dos veces (C3).
- **S6** — **AC-C6** — Dada una hoja de un árbol multivalor en el archivo de datos con la organización correcta y recordada, y una entrada cuyo contenido es incoherente, cuando una operación la usa dos veces, entonces falla las dos veces con la clase de daño y el número de esa página (C1, M11).
- **S7** — **AC-C7** — Dado un nodo recordado, cuando la prueba altera un byte de su página en el archivo de datos sin recalcular su código de integridad, entonces la lectura siguiente falla con la clase de daño y el número de esa página (C2, C7).
- **S8** — **AC-C8** — Dada una memoria de nodos recordados de un tamaño reducido para la prueba, y nodos correctos y uno dañado como en AC-C4 cuyos números de página la obligan a descartar recuerdos, cuando cuatro hilos con transacciones de lectura los leen a la vez, al menos mil veces cada uno, mientras una escritura los cambia y se trasladan, entonces cada lectura devuelve los datos de su instantánea y el nodo dañado falla en todas sus lecturas con la clase de daño (C5, C6).
- **S9** — **AC-C9** — Dado un almacén en disco con nodos recordados, cuando se cierra y se vuelve a abrir, entonces la primera lectura de cada nodo comprueba su organización otra vez (C4).
- **S10** — **AC-C10** — Dada la medición de C9, cuando se ejecuta en Windows y en Linux, entonces informa el coste por lectura de nodos hoja de 1, 100 y 370 entradas y el de Contiene en las dos cargas, sin recordar nodos y recordándolos, y los resultados quedan en el informe de cierre (C9).

## Decisions (left to the implementation, C5, C9)

- **D1 — Where the memory lives.** One table per open store inside the pager, next to the integrity check that
  produces the code; the tree code asks through its page source. No unsafe code.
- **D2 — Table.** A fixed number of slots, one `AtomicU64` each, holding the verified integrity code; the slot is
  the page number modulo the slot count. A hit needs the stored code to equal the code just verified. Code `0` is
  never remembered (an empty slot). The page number is inside the checksummed bytes, so a false hit needs a 64-bit
  coincidence (C5, C7). Default size chosen by the measurement (T3); tests may set their own per store.
- **D3 — Count for tests.** The table counts the contents it remembers (one per successful check of a verified
  page); hits are not counted, so they cost nothing extra.
- **D4 — Measurement switch.** A process-wide hidden switch, used only by the measurement example, turns the
  memory off (C9); tests always use it on.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | C1–C8 (engine; its acceptance tests are T2's) | inline (no writer trigger: one unit) | done | `6a1396b` |
| T2 | S1–S9 at the public API | inline | done | `1ea3d98` |
| T3 | C9, S10: measurement example, Windows and Linux runs, default size | inline | done | `4057ce7` |
| T4 | Docs, closing report, reviews | inline | done, Codex approved (L15) | the closing report commits |

Route evidence: the four tasks depend on each other in order (T2 and T3 need T1), so no two independent units run
in parallel; no writer trigger fires.

## Log

- **L1** (2026-10-30, user, original request): "Entonces empieza con la implementación ODD". Estimate 2 h 30 min to
  4 h, confirmed with "Sí." Start 2026-10-30T09:07:30+00:00 (`date`).
- **L2** (2026-10-30): forecast about 900 authored changed lines (T1 ~250, T2 ~350, T3 ~300), above 400: default
  delivery taken without asking during the run (CLAUDE.md): local work-unit commits on this branch, one per task, the
  high-risk T1 alone; slicing into pull requests is left to the user.
- **L3** (2026-10-30, T1): PRD defect found while testing the table. C5 said a lookup "nunca combina el número de un
  nodo con el código de otro", but the design the user agreed before the PRD (a slot per page number modulo the slot
  count, holding only the code; "otra coincidencia de 64 bits, del mismo orden que la anterior") finds a content of
  another page that shares the slot and has the same code. That needs a 64-bit coincidence, because the code covers
  the page number: the risk C7 already accepts. C5 (Spanish and English) now says each entry is stored and looked up
  whole, and that matching another page's code takes the same coincidence as C7. Default taken without stopping the
  run; listed in the closing report.
- **L4** (2026-10-30, T1): commits `09a9da4` (C5 wording), `6a1396b` (T1) and `64912a9` (the phase log, committed to keep the
  untracked log out of the review inventory). Tests: 411 passed (408 before, plus 3 unit tests of the table); fmt and
  clippy clean. RED was not observed first for T1: the engine change came before its acceptance tests (T2), which
  run against it; the existing damage tests stayed green.
- **L5** (2026-10-30): RDD review of T1 (`09a9da4..` committed only; assessed medium `under_budget`, raised to high
  with item 2: skipping the layout check weakens damage detection), lenses risk, resilience, reliability: approved,
  lineage `review-3073e38929171edc`, acknowledged. Findings: R1-001 (suggestion, collision trust assumption with an
  external writer) → already the accepted residual risk of C7; R4-fixed-table-per-opening and R3-eager-table-allocation
  (suggestions, 512 KiB per store at open) → the size is chosen by the measurement in T3; R3-node-at-cache-untested
  (warning) → T2 adds the acceptance tests through `checked_remembered`; R3-with-checked-slots-not-panic-safe
  (suggestion) → fixed in T2 with a guard. The phase-log background marks of this review failed (wrong arguments), so
  its duration is not in the phase log.
- **L6** (2026-10-30, T2): acceptance tests `crates/krummi/src/check_tests.rs`, AC-C1 to AC-C9 on disk and in
  temporary mode (AC-C9 on disk only), through the public API and two test hooks: `Engine::checked_remembered` and
  `Engine::flip_byte_in_place`. The second takes the engine mutably (no transaction open) and the scratch area's
  `flushing` lock (no transfer writes meanwhile) to change a byte where the store keeps a page, without its checksum.
  `with_checked_slots` restores the previous size on panic too (RDD T1 suggestion). An AC-C2 assertion first failed
  because the test's own helper read the transferred leaf before the count; it now checks the page at the end.
  Mutation check: remembering before the layout check failed AC-C4 and AC-C8; never remembering failed AC-C1, AC-C2,
  AC-C3, AC-C6 and AC-C9. Checks: fmt, clippy clean; `cargo test --workspace` 420 passed, 0 failed.
- **L7** (2026-10-30, T3): example `node_check_timing` and [measurement](trees-node-check-cache-measurements.md),
  Windows 09:34:46–09:37:16 and a rerun 09:38:17–09:40:44 (the first run's absolute times looked high; the rerun, with
  the stores in the system's temporary folder, agreed), Linux in WSL 09:37:17–09:38:00. Remembering: leaf of 370
  entries −86% (Linux) and −88%/−90% (Windows); of 100 entries −64% and −47%/−59%; Contains −66% and −71%/−72% in
  the multi-value tree, −44% and −36%/−55% in the composite tree; a leaf of one entry +13% on Linux (40 ns) and noise
  on Windows. Defaults taken (left to the implementation, C5): every node uses the memory, with no minimum number of
  entries; 65,536 slots per open store (512 KiB). Open observation: on Windows the saving per Get on the dense leaf
  (12.5 and 14.0 µs in the two runs) exceeds the isolated cost of the layout check (2.5–2.9 µs); not explained. `cargo fmt` reformatted the
  example after the runs (formatting only). README: the tree paragraph names the remembered check and the example.
  Checks: fmt, clippy clean; 420 passed.
- **L8** (2026-10-30): RDD review of the last slice `64912a9..4057ce7` (T2, T3; assessed medium `under_budget`),
  lenses risk, reliability, readability: approved, lineage `review-9992850ecbe7818c`, acknowledged. Findings and
  dispositions: R2-001 (warning, the Windows saving "12.6 µs" matched no row) → fixed, 12.5 and 14.0 µs;
  R3-ac-c8-slot-count-unasserted (warning) → fixed, AC-C8 asserts the stores got 4 slots (new hook
  `checked_slots`); R2-003 (the AC-C8 node-read count was overstated) → fixed, 2 + 2 × keys + 1 per round;
  R3-ac-c7-checksum-vs-layout → fixed, AC-C7 also flips a byte of a key in another remembered leaf, which keeps the
  layout valid, and the read fails; R1-flip-byte-unchecked-preconditions and R2-004 → fixed, the hook panics on an
  offset beyond the payload or a page with a scratch version, and the engine hook documents it; R2-002 → fixed, the
  example's documented command has `--`; R3-timing-order-bias (suggestion, the round without remembering always runs
  first) → kept: the fastest of three limits it, and changing the order now would not match the recorded runs.
  Checks: fmt, clippy clean; 420 passed.
- **L9** (2026-10-30): RDD review of `4057ce7..ceab08d` (the review fixes), lenses reliability and risk: approved,
  lineage `review-b82822f29a0a84ec`, acknowledged. R3-ac-c7-key-flip-offset-and-path-unproved (warning) → dismissed
  with evidence: `ReadTransaction::page` returns the payload (`Pager::page` is `&run[PAGE_HEADER_SIZE..]`) and the hook
  adds `PAGE_HEADER_SIZE` to its offset, so the flipped byte is the key's padding; `node::is_valid` reads only the
  kind, the count, the offsets and the lengths (and off-leaf run references), none of which that byte holds, so the
  layout stays valid and only the integrity code can fail.
- **L10** (2026-10-30): Codex code review of `f49919f..c7408c4` failed before reviewing: "You've hit your usage
  limit … try again at 12:44 PM" (wrapper status `error`, thread `01a18c3f-2fe9-7e43-80b2-7f700c88b917`, no report
  saved). Not an approval; waiting for the user's decision.
- **L11** (2026-10-30): closing report [trees-node-check-cache-report.md](trees-node-check-cache-report.md), written
  with the Codex code review pending at the user's choice ("Sí" to writing it now). Active time 52 min 13 s against
  the estimate of 2 h 30 min to 4 h.
- **L12** (2026-10-30): Codex code review R1 of `f49919f..fc57e47`
  ([report](../../docs/reviews/trees-node-check-cache/2026-10-30-code.md),
  [resolutions](../../docs/reviews/trees-node-check-cache/2026-10-30-code-resolutions.md)): status `findings`,
  thread `01a18cd9-9d10-7141-9077-eaeeec8c5414`. R1 (major, the table's allocation aborts the process when the
  system has no memory for it, against P13) → fixed in `161c0e4`, test first (RED: `capacity overflow` panic).
  R2 (major, AC-C8 does not make the readers overlap the writes and transfers) → fixed in `f95c57f`: coordinated
  readers, 1,000 full rounds each, every generation seen, transfers asserted while they read. Checks: fmt, clippy,
  421 tests passed.
- **L13** (2026-10-30): RDD review of `fc57e47..6b095ff` (the R1 and R2 fixes; assessed medium `under_budget`, last
  slice), lenses resilience and reliability: approved, lineage `review-e32dc28366105d7c`, acknowledged. Advisory,
  non-blocking: R3-ac-c8-hangs-on-failure and R4-reader-loop-hangs-on-writer-failure (warnings, fixed in `8cf01cf`
  with Codex R3), R3-oom-test-only-overflow and R4-scratch-assert-while-readers-pin (suggestions, not acted on:
  the table's reservation goes through the same fallible helper that the fault-injection tests already cover, and
  every reader's open snapshot is the newest one when the writer transfers).
- **L14** (2026-10-30): Codex follow-up R2 ([report](../../docs/reviews/trees-node-check-cache/2026-10-30-code-02.md)):
  status `findings`, thread `01a18cf1-e3a6-7c73-a359-9fe773b93063`; R1 and R2 verified resolved; new R3 (minor,
  AC-C8 hangs when a reader or the writer fails) → fixed in `8cf01cf`. Checks: fmt, clippy, 421 tests passed.
- **L15** (2026-10-30): RDD review of `6b095ff..2fa32aa` (the R3 fix), lenses reliability and resilience: approved,
  lineage `review-c534a2c3ec2a4bc1`, acknowledged; R3-001 (suggestion, informational) not acted on. Codex follow-up
  R3 ([report](../../docs/reviews/trees-node-check-cache/2026-10-30-code-03.md)): status `approved`, thread
  `01a18d00-2118-7ce2-b2b4-96a0c9d850ed`, R3 verified resolved, no finding open. The closing report is updated
  with the outcome; following the user's "Lo fusionaremos y subiremos cuando codex lo revise luego.", main is
  fast-forwarded to this branch and pushed.
- **L16** (2026-10-30): RDD review of `2fa32aa..f739caa` (the closing report update, documentation only; assessed
  medium `under_budget`, last slice), lens readability: approved, lineage `review-1d015634a812a639`, acknowledged.
  R2-pending-section-lists-approved-review (suggestion) → fixed in the commit that records this review: the
  approved Codex review left the report's pending section.
