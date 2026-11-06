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
| T2 | S12 | inline: single-load runs before and after, Windows and WSL 2; the complete runs after O4 | single-load done | L8 |
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
- **L6** (2026-11-06): independent verifier (sonnet/high) over `08ffc84..d6ae1c7`: AC-I1 to AC-I11 and I1 to I8
  PASS, no blockers. It traced every in-place page write to `write_run_shared` (the test byte flip is the only
  bypass) and confirmed L4's argument in the code: the transfer writes a version only when every open snapshot's
  txid is at least that version's, a snapshot registered later reads it from the scratch area until it is written,
  and the writer reads it there too, so no read of the data file overlaps a forget and its write. Visibility:
  `Remembered::ensure` publishes the committed words before `len`, so a reader of page `p < len` finds its word.
  Its checks, in its own target folder: fmt ok, clippy clean, `remembered` 5 passed, `integrity_tests` 12 passed,
  `check_tests` 13 passed, the four engine crates all passed; `integrity_tests` and `check_tests` looped 25 times
  with 4 threads, 0 failures. Advisory: a disk store reserves 16 GiB of address space for its bits; under
  `ulimit -v` or strict overcommit on Linux, opening could fail as memory exhausted (it ran on Windows only).
  Verifier scratch removed.
- **L7** (2026-11-06): RDD of `08ffc84..d6ae1c7` (risk, resilience, reliability; item 4 escalated): approved,
  lineage `review-b598396ed8c88c55`. R4 warning: every store on disk reserved 16 GiB of address space, which can fail
  where address space is capped, and the error reported a few bytes instead of the reservation. R3 suggestion: a
  run remembered by its first page only would skip the check after a rewrite of an interior page alone. Both taken
  in the fix commit: the reservation covers 64 TiB of store (1 GiB of address space, less than a temporary store's
  64 GiB), a failed reservation reports its bytes, and a run is remembered with every page and skipped only while
  all are remembered (unit tests in `remembered.rs` and `pager_tests.rs`). Checks: fmt ok, clippy clean,
  `cargo test --workspace` 539 passed, 0 failed, 30 ignored. The first single-load rounds failed at once (the script
  passed a relative path to the binaries, which Python on Windows did not find), so nothing was measured.
- **L8** (2026-11-06): T2, single-load runs of the read loads at full scale, before (`3ec8641`, the binary of the
  complete runs) against after (`5a9f9f0`), two rounds alternating the binaries, each load in its own `measure.py`
  call, nothing else running; W18, W20 and W22 read the last store of W17, W19 and W21 of the same binary (median
  op/s, before → after, per pair):

  | Load | Windows | WSL 2 |
  |---|---|---|
  | W7 | 650,682 → 1,217,780 (+87 %); 477,129 → 1,078,306 (+126 %) | 617,077 → 1,317,268 (+113 %); 657,847 → 1,339,518 (+104 %) |
  | W8 | 1,124,867 → 2,761,418 (+145 %); 1,402,542 → 2,789,667 (+99 %) | 1,408,256 → 3,352,744 (+138 %); 1,473,006 → 3,524,479 (+139 %) |
  | W9 | 1,240,354 → 4,107,427 (+231 %); 1,164,677 → 3,883,338 (+233 %) | 1,064,473 → 2,801,475 (+163 %); 949,874 → 2,612,879 (+175 %) |
  | W10 | 22,832,458 → 26,986,911 (+18 %); 22,132,806 → 26,621,304 (+20 %); 23,101,090 → 28,033,427 (+21 %) | 21,491,624 → 24,025,789 (+12 %); 22,640,403 → 22,741,384 (0 %); 22,862,666 → 25,910,853 (+13 %) |
  | W11 | 14,818,478 → 18,405,305 (+24 %); 16,642,815 → 19,694,182 (+18 %) | 14,568,778 → 14,978,557 (+3 %); 14,745,826 → 14,621,673 (−1 %) |
  | W12 | 401,568 → 504,402 (+26 %); 271,236 → 454,063 (+67 %) | 913,678 → 1,283,877 (+41 %); 915,983 → 1,200,791 (+31 %) |
  | W13 | 15,108 → 12,831 (−15 %); 9,512 → 11,368 (+20 %) | 8,359 → 8,701 (+4 %); 7,793 → 8,252 (+6 %) |
  | W18 | 62,958 → 74,193 (+18 %); 67,700 → 96,922 (+43 %) | 71,652 → 91,860 (+28 %); 76,447 → 78,177 (+2 %) |
  | W20 | 6,745,071 → 13,307,003 (+97 %); 5,607,975 → 9,619,695 (+72 %) | 7,332,068 → 15,703,699 (+114 %); 6,272,802 → 11,429,247 (+82 %) |
  | W22 | 918,413 → 3,317,313 (+261 %); 1,112,940 → 3,841,695 (+245 %) | 1,114,986 → 2,277,805 (+104 %); 1,066,426 → 3,417,862 (+220 %) |

  W10 shown with its three later pairs: the first two rounds gave +11 % / −21 % on Windows and +89 % / −41 % on WSL
  2, a scan that hashes one page per many entries, so those were noise. W13 (the writer beside W12's readers) moves
  both ways within its noise. Every read load gains or holds; W7 roughly doubles and W22 gains 2–3.6 times. The
  complete runs of S12 run after O4 (L2). The first attempt at W18, W20 and W22 failed because the script gave W17,
  which starts empty, a source store; fixed before these runs.
- **L9** (2026-11-06): RDD of the last slice `d6ae1c7..f247ef0` (risk, reliability): approved, lineage
  `review-c952ab83310012c9`. R1 warning (and R3 suggestion): a store on disk can no longer grow past 64 TiB, the
  out-of-memory class; kept, it is the deliberate answer to L7's address-space warning, and stores that large are
  out of any realistic use. R3 warning: with a run remembered page by page, a bad reference to one of its interior
  pages as a page of its own would skip the check. Fixed in `5482072`: a remembered page is used only when it names its
  own number (bytes 8 to 16), which interior pages of a run do not, so such a read is checked and fails; the test
  `rewriting_an_interior_page_of_a_run_makes_it_checked_again` reads one and gets the damage class (without the
  guard it fails, mutation not committed). R3 warning: the bytes an out-of-memory opening reports were untested;
  `a_failed_reservation_reports_the_bytes_it_asked_for` pins them. Checks: fmt ok, clippy clean,
  `cargo test --workspace` 540 passed, 0 failed, 30 ignored.
- **L10** (2026-11-06): RDD of `f247ef0..362674a` (risk, reliability): approved, lineage `review-14d2b05e2c604d05`.
  Kept as advisories, not changed: R3 warning, a value whose payload at bytes 8 to 16 of an interior page encodes
  that page's own number would still let a bad reference to it skip the check (only an engine bug reaches a bad
  reference, and the chance is one in 2^64 for content not built to match); R3 suggestions, the test does not read
  a run starting at an interior page, and the reservation test assumes a 64-bit target (the engine's targets are
  64-bit).
- **L11** (2026-11-06): Codex code review of `08ffc84..362674a`
  ([report](../../docs/reviews/integrity-once/2026-11-06-code.md),
  [resolutions](../../docs/reviews/integrity-once/2026-11-06-code-resolutions.md)): `findings`. R1 (major): after
  a run was remembered, its first page alone or a shorter run from it skipped the check, though the base failed
  them as damage; R2 (minor): a remembered read of no pages panicked. Both fixed in `e5ea536`: a run of several
  pages is remembered by its interior pages' bits and a mark of its code and length in the table of checked
  contents, never by its first page's bit, and a read of no pages fails as damage. Open product question for the
  user, after the loop: the 64 TiB ceiling of a store on disk (P10 sets no limit but the disk). Checks: fmt ok,
  clippy clean, `cargo test --workspace` 541 passed, 0 failed, 30 ignored.
- **L12** (2026-11-06): RDD of `362674a..95ec765` (risk, reliability): approved, lineage
  `review-ed093f8836c99912`. Its R3 warning (no test rewrites a run's first page alone) and R3 suggestion (no test
  of a longer run or a run from an interior page) taken in `6d9c923`: the extent test now reads 4 pages from the first
  page and 2 from an interior page (checked, damage) and, after the first page is rewritten alone, the run is
  checked again and fails. Kept as an advisory: a run's mark lives in the shared table of checked contents, so with
  that table off (the measurement switch of C9) or the slot taken by another page, the run is checked again on each
  read; that only costs time.
