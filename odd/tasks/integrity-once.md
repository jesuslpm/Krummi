# Remembered page integrity (O3)

Objective: O3 of [the performance plan](../../docs/performance/plan.md), from the PRD
[pager-integrity-once.prd.md](../../docs/prd/krummi/pager-integrity-once.prd.md) (decisions I1–I9, authoritative).
Branch `perf/integrity-once` from `prd/integrity-once` `9317233` with `bench/complete-2026-11-05` merged in; code base
`3ec8641`. Delivery: single branch, unmerged. Test runner: `cargo test`.

## Specs

The PRD's decisions I1–I9, *Fuera de alcance* and *Dejado a la implementación* are the specs; each S# quotes its
acceptance criterion verbatim. Every criterion runs on a disk store and a temporary store, except the parts that
reopen, which are disk only.

- **S1** (AC-I1): "Dados un árbol y un valor guardado fuera de su hoja con sus páginas en el archivo de datos, cuando
  una transacción de lectura los lee varias veces y otra transacción de lectura los lee después, entonces todas
  devuelven lo mismo y la integridad de cada página y del valor se comprueba solo en su primera lectura; y tras cerrar
  y reabrir el almacén, la primera lectura vuelve a comprobarla (I1, I2, I5)."
- **S2** (AC-I2): "Dados los nodos de un árbol, recordados por una transacción de lectura anterior, y una página del
  mapa de espacio libre, recordada por una escritura anterior (I3), todos en el archivo de datos, cuando una
  transacción de escritura lee una clave dos veces y después guarda una que modifica una hoja y reserva páginas,
  entonces comprueba la integridad de cada página que lee del archivo de datos en cada lectura, incluidas la hoja y
  la página del mapa que copia; y una transacción de lectura posterior no vuelve a comprobar los nodos que la
  escritura no modificó (I2, I3)."
- **S3** (AC-I3): "Dada una versión confirmada que todavía está en el área temporal, y las copias que modifica una
  escritura, cuando se leen, entonces su integridad no se comprueba, como hoy (I3)."
- **S4** (AC-I4): "Dada una página recordada, cuando una escritura cambia su contenido y se confirma mientras una
  transacción de lectura abierta antes sigue leyéndola, entonces esa transacción sigue viendo el contenido anterior;
  y cuando esa transacción se cierra y la nueva versión se traslada a su sitio, entonces una transacción de lectura
  nueva ve el contenido nuevo y su integridad se comprueba una vez (I4, C8, G3)."
- **S5** (AC-I5): "Dado un valor fuera de su hoja recordado, cuando se borra y una escritura posterior reutiliza sus
  páginas para otro contenido que llega al archivo de datos, entonces la lectura del contenido nuevo lo comprueba una
  vez y devuelve lo escrito (I1, I4)."
- **S6** (AC-I6): "Dada una página del archivo de datos alterada con el almacén cerrado, cuando se abre y se lee dos
  veces en la misma transacción de lectura y una vez en otra, entonces cada lectura falla con la clase de daño y el
  número de esa página (I1, I2, G6)."
- **S7** (AC-I7): "Dada una página que todavía no se leyó en la apertura en curso, cuando la prueba altera uno de sus
  bytes con el almacén abierto y después se lee, entonces la lectura falla con la clase de daño y el número de esa
  página (I2, I8)."
- **S8** (AC-I8): "Dada una página recordada sin versiones pendientes de trasladar, cuando la prueba altera uno de sus
  bytes con el almacén abierto, entonces una transacción de escritura que la lee falla con la clase de daño y el
  número de esa página, en esa operación, y no puede confirmar (T11); y tras cerrar y reabrir el almacén, la primera
  lectura de esa página falla igual (I3, I5, I8). Este criterio sustituye a AC-C7 y a la cuarta parte de AC-X19."
- **S9** (AC-I9): "Dados páginas correctas y una dañada como en AC-I6, cuando cuatro hilos con transacciones de
  lectura las leen a la vez, al menos mil veces cada uno, mientras una escritura cambia las correctas y se trasladan,
  entonces cada lectura devuelve los datos de su instantánea y la página dañada falla en todas sus lecturas con la
  clase de daño (I4, I7)."
- **S10** (AC-I10): "Dado un almacén con páginas recordadas, cuando crece mientras se leen, entonces las páginas
  recordadas antes de crecer que el traslado no reescribió siguen sin comprobarse otra vez, y las páginas nuevas se
  comprueban la primera vez que se leen (I4, I5, I6)."
- **S11** (AC-I11): "Dada una falta de memoria simulada al reservar la memoria de I6, cuando se abre un almacén,
  entonces la apertura falla con la clase de memoria agotada; y cuando un almacén abierto crece, entonces el
  crecimiento falla con esa clase, el tamaño del almacén en su información no cambia y el almacén sigue aceptando
  lecturas y escrituras (I6, P13)."
- **S12** (AC-I12): "Dadas las ejecuciones completas del banco en Windows y en Linux sobre el commit anterior al
  cambio y sobre el cambio, cuando se comparan por plataforma, entonces ninguna carga baja su razón con Voron más de
  0,05 y W7 y W22 la suben más de 0,05; y el informe de cierre incluye esas comparaciones, si W7 y W22 cumplen su
  umbral y las ejecuciones de una sola carga de lectura antes y después (I9)."

## Tasks

| ID | Specs | Route | Status | Evidence |
|---|---|---|---|---|
| T1 | S1–S11 | inline: `Remembered` bitmap in the pager; read transactions use it, the writer checks and remembers; forget before every in-place write, the test byte flip bypasses it; AC-I1–AC-I11, AC-I8 replacing AC-C7 and AC-X19 part 4 | done | `d6ae1c7`, L5 |
| T2 | S12 | inline: single-load runs before and after, Windows and WSL 2 | pending | |
| T3 | all | independent verifier (risk item 4), RDD, Codex, closing | pending | |

## Log

- **L1** (2026-11-06): user's request, verbatim: "Haz O3, luego O4, por último el benchmark."
- **L2** (2026-11-06): estimate for O3, O4 and the complete runs together: 7 h – 11 h, confirmed ("Sí"); start
  07:02:41 UTC. Defaults taken (no question left open): O3 and O4 are two runs with their own branch, execution
  document, reviews and closing report; O4's branch starts from O3's. The user put the benchmark last, so the
  complete runs of S12 run once after O4 and measure O3 and O4 together; O3's own effect comes from the single-load
  runs (T4).
- **L3** (2026-11-06): map (explorer). Every verified store read goes through `Pager::run` (`pager.rs:333`); every
  in-place write through `Pager::write_run_shared` (`pager.rs:491`): transfer (`scratch.rs:678`, a run as one
  range), close (`scratch.rs:758`, page by page), recovery and the test byte flip (`scratch.rs:268`). `Snapshot`
  already knows whether it is the writer's (`scratch.rs:963`). Opening reads the metadata page with `Pager::page`
  (`krummi-tx/src/lib.rs:154`); the store info query reads no page. Growth is `Pager::grow` (`pager.rs:593`): max
  size first (`next_size`), then reserve and map, publishing the mapping before `len` (O1). No counter of integrity
  checks exists. Simulated memory failures go through `sys::alloc_point` (`Op::Alloc`).
- **L4** (2026-11-06): design. The bitmap lives in a `sys::Reservation` (the address range of temporary stores),
  one bit per page, committed as the store grows, so it never moves: readers index it without a lock or a copy, and
  a growth only commits more of it before publishing the new size. It reserves the bitmap of the maximum size, or of
  1 PiB when the store has none (16 GiB of address space, no memory); a temporary store, of its reservation.
  Committed memory is one bit per page rounded up to the system's memory page (I6: 16 KiB per GiB); committing more
  is the fault point of a reservation of memory, so its failure is the memory-exhausted class. Growth commits after
  the maximum-size check and before the file grows (I6). A run (value off its leaf) is remembered by the bit of its
  first page and forgotten with every page of its range (I1, I4). Read snapshots use and set the bit; the writer's
  snapshot always checks and sets it (I3); `Pager::page` (opening, pager-level tests) checks without remembering,
  so the memory is empty when the store has opened (I5). The forget sits in `write_run_shared`, before the bytes
  are written (I4); the test byte flip writes without it, since it stands for a change the engine did not make.
  Why no stale bit survives: a page is written in place only once no snapshot can read its earlier content from the
  data file (C8, T14), so every read that remembered it ended before its forget, and no read of the data file runs
  during its write. Risk: item 4 (concurrency): independent verifier.
- **L5** (2026-11-06): T1. `crates/krummi-pager/src/remembered.rs` (bitmap, unit tests), `Pager::run_with` and
  `page_and_code_with` with `Integrity::{Check, CheckAndRemember, UseRemembered}`, `Snapshot::integrity` (reader or
  writer), the forget in `write_run_shared` and `write_unremembered` for the test byte flip, the bits committed in
  `grow` after `next_size`, the test hooks `integrity_checks`, `is_remembered` and
  `with_remembered_commits_failing` (it fails every growth, also one whose bits fit in the committed words).
  `crates/krummi/src/integrity_tests.rs`: AC-I1 to AC-I11 (AC-I8 for a normal and a fixed-size tree), each on both
  modes, the reopening parts on disk; AC-I9 shares AC-C8's body (`concurrent_reads_while_a_write_moves_nodes`), with
  a page damaged by the test byte flip after the transfer instead of a damaged layout. AC-C7 and AC-X19 part 4
  removed (AC-I8 replaces them). README and the tree's comments no longer say that every read verifies. RED was not
  observed before the code (the code came first); instead, three mutations, not committed, each fail the tests that
  pin them: never using the remembered bit fails AC-I1, AC-I4, AC-I5, AC-I10; no forget before writing fails AC-I4,
  AC-I5; the writer using the remembered bit fails AC-I2 and both AC-I8. Checks: fmt ok, workspace clippy clean,
  `cargo test --workspace` 537 passed, 0 failed, 30 ignored. One commit for the whole behavior (about 1,000 lines,
  over the 400 heuristic): the code is one change whose tests need all of it, and it is the high-risk unit (item 4).
