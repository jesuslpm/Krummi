# Free space — ODD execution document

Objective: implement `docs/prd/krummi/free-space.prd.md` (decisions F1–F14, criteria AC-F1–AC-F20), the authoritative
specification; its ancestry is `krummi.prd.md` (root), with `pager`, `transactions`, `journal` and
`journal-group-commit` as related PRDs.
Branch: `feat/free-space`. Delivery: `ask-on-risk` (single branch, merged by the user). Test runner: `cargo test
--workspace` (Windows; Linux in WSL). Estimate: 2 h – 3 h 30 min. Real start: 2026-10-24 08:28:08 +0100.

## Specs

Each spec quotes an acceptance criterion of the PRD verbatim; the decisions it names (F#, and the inherited ones)
are part of it. The PRD's sections *Validaciones y mensajes*, *Fuera de alcance* and *Dejado a la implementación*
bound every task. S0 covers what has no criterion of its own: the PRD's AC10 repetition ("AC10 del padre raíz se
repite con un almacén pequeño que tiene páginas liberadas y una página del mapa").

- **S1** — **AC-F1** — Dado un almacén cuyas únicas páginas libres por debajo de la frontera son 10 páginas liberadas por una confirmación durable, cuando una escritura posterior reserva 10 páginas de una en una, entonces recibe exactamente esas 10, `krummi.data` no cambia de tamaño, la frontera no avanza y, tras confirmar, las páginas libres de la información bajan en 10; y la reserva siguiente avanza la frontera (F3, F4, F10).
- **S2** — **AC-F2** — Dado un almacén sin páginas libres por debajo de la frontera, cuando una escritura libera una página y después reserva otra, entonces la reservada no es la liberada; y tras confirmar, una escritura posterior que reserva una página recibe la liberada (F4, F6).
- **S3** — **AC-F3** — Dada una lectura abierta que ve el contenido de una página, cuando una escritura la libera y se confirma, de modo que es la única libre por debajo de la frontera, y otra escritura reserva una página, escribe un contenido nuevo y se confirma, entonces la reservada es ese mismo número, la lectura abierta sigue viendo el contenido anterior, una lectura nueva ve el nuevo, y tras terminar la lectura antigua, cerrar y reabrir, está el nuevo; y el mismo resultado se obtiene con las dos confirmaciones en modo diferido, con la lectura nueva abierta después de que son durables. Y repitiendo tres veces sobre el mismo número de página la liberación y la reutilización con un contenido distinto, con una lectura abierta antes de cada liberación y todas abiertas hasta el final, cada lectura ve el contenido de su generación (F6, T14, E3).
- **S4** — **AC-F4** — Dada una escritura que libera páginas y reserva otras entre las libres por debajo de la frontera, cuando se aborta, o se deja de usar sin confirmar, o su confirmación falla por disco lleno, entonces ninguna de sus liberaciones ni reservas se aplica: las páginas que liberó siguen en uso, las que reservó siguen libres, la frontera no avanza y las páginas libres de la información no cambian (F6, T10, T11).
- **S5** — **AC-F5** — Dada una confirmación en modo durable que libera una página, retenida en la cola del diario, y una escritura que reutiliza esa página y se confirma en modo durable después, cuando falla la escritura del grupo de la primera, entonces las dos fallan con la clase de entrada y salida; y al reabrir sin el fallo, el almacén tiene un prefijo de las dos y nunca la segunda sin la primera (F6, E8).
- **S6** — **AC-F6** — Dada una secuencia de confirmaciones, en los dos modos, que liberan páginas y las reutilizan con contenido nuevo, en la que una misma página se libera y se reutiliza al menos tres veces, cuando se simula un corte de corriente en cada punto en que se escribe o se lleva al disco el diario o `krummi.data`, y se mata el proceso en cada punto de la recuperación, entonces al reabrir el almacén tiene un prefijo de la secuencia, cada página reutilizada tiene el contenido de la última confirmación presente que la escribió, ninguna página está a la vez libre en el mapa y en uso según las confirmaciones presentes, `krummi.data` llega al menos hasta la frontera, y la primera parte de las páginas libres de la información coincide con las libres del mapa (F2, F3, F6, F10, G5).
- **S7** — **AC-F7** — Dado un almacén con un rango libre de 20 páginas seguidas por debajo de la frontera, cuando una escritura reserva 16 páginas contiguas, entonces las recibe dentro de ese rango sin que la frontera avance; y dado un almacén con 100 páginas libres sueltas, ningún rango libre de 16 y una frontera a la que le quedan más de 16 páginas en su región, la reserva de 16 contiguas recibe las 16 páginas que empiezan en la frontera, que avanza 16 (F5).
- **S8** — **AC-F8** — Dada una frontera a la que le quedan 10 páginas en su región y ninguna página libre por debajo de ella, cuando una escritura reserva 16 páginas contiguas, entonces el rango empieza en la primera página después de la página del mapa de la región siguiente, las páginas libres de la información, tras confirmar de forma durable, incluyen las 10 saltadas, y la reserva siguiente de una página recibe una de ellas (F3, F5, F10).
- **S9** — **AC-F9** — Dada una escritura, cuando pide un rango de 0 páginas o de una página más que el tope, libera dos veces la misma página, libera una página que el mapa tiene como libre, una del encabezado, de metadatos o del mapa, o una más allá del final del almacén, lee o modifica una página que ella misma liberó, o modifica una página del mapa, entonces cada operación falla con la clase de uso incorrecto sin efecto; y en cada caso, en una escritura que antes de la operación incorrecta modificó una página, reservó otra y liberó una tercera, confirmarla falla con la clase de uso incorrecto, ninguno de esos cambios aparece en una lectura nueva ni al reabrir, y la escritura siguiente se confirma sin error. En un almacén sin tamaño máximo, un rango del tamaño del tope se reserva sin error en una región que no es la primera (F5, F8, T11).
- **S10** — **AC-F10** — Dada una escritura que modifica una página, reserva un rango de 8 páginas, libera 3 de ellas y la página modificada, y libera una página que ella misma reservó suelta, cuando se confirma y se reabre, entonces las 5 páginas restantes del rango siguen en uso y empiezan a cero, las 3 del rango, la suelta y la modificada están libres, y la página modificada conserva en `krummi.data` su contenido anterior; y si antes de esa escritura no había páginas libres por debajo de la frontera, una escritura posterior que reserva 5 páginas de una en una recibe exactamente esas 5 (F4, F8).
- **S11** — **AC-F11** — Dado un almacén con un tamaño máximo que deja sitio para las versiones de una escritura en `krummi.temp` pero no para que `krummi.data` crezca, y con páginas libres por debajo de la frontera y ninguna entre la frontera y el final del archivo, cuando una escritura reserva de una en una tantas páginas como hay libres en el mapa, entonces se confirma sin error; cuando reserva una página más, falla con la clase de tamaño máximo, no aplica nada y el almacén sigue aceptando escrituras; y con una falta de espacio simulada solo al hacer crecer `krummi.data`, una reserva que necesita crecer falla con la clase de disco lleno y una que reutiliza páginas libres se confirma sin error (F5, P10, G7).
- **S12** — **AC-F12** — Dada una lectura abierta durante 1.000 confirmaciones que liberan y reservan el mismo número de páginas, cuando se observa `krummi.data`, entonces su tamaño no cambia desde la segunda (F7).
- **S13** — **AC-F13** — Dado un almacén cerrado sin error cuyas únicas páginas libres están en una región alejada de la frontera, cuando se abre, entonces la apertura no lee ninguna página del mapa; y cuando una escritura reserva una página, recibe una de esas páginas libres y la frontera no avanza (F4, F9).
- **S14** — **AC-F14** — Dado un almacén cerrado cuyas únicas páginas libres por debajo de la frontera están en una región cuya página del mapa tiene un byte alterado, cuando se abre, entonces abre sin error, la información da las mismas páginas libres que antes de alterarla, todas las páginas en uso salvo esa página del mapa se leen sin error, una escritura que solo modifica páginas en uso se confirma sin error, y una que reserva una página por debajo de la frontera falla con la clase de daño y el número de esa página del mapa, no se puede confirmar, y una reserva posterior vuelve a fallar igual (F9, F10).
- **S15** — **AC-F15** — Dado un almacén nuevo, cuando se consulta la información, entonces las páginas libres son el número de páginas menos las páginas propias; y dado un almacén con páginas en uso que reservó una confirmación durable anterior, cuando una escritura abierta las libera, las páginas libres no cambian; cuando una confirmación diferida las libera, no cambian hasta que es durable, y entonces suben en ese número; y tras cerrar y reabrir dan el mismo valor (F10, E3).
- **S16** — **AC-F16** — Dada una secuencia aleatoria y reproducible por semilla de al menos 100.000 operaciones sobre un almacén de hasta 4 regiones, con reservas de páginas sueltas y de rangos de hasta 64 páginas, liberaciones, confirmaciones en los dos modos y abortos, con lecturas abiertas y cerradas en puntos aleatorios y una caída simulada y reapertura en otros, cuando se compara con un modelo de referencia que conoce las páginas en uso y el tamaño real de `krummi.data`, entonces nunca se reserva una página en uso, ningún rango incluye una página propia, cada lectura ve el contenido de su instantánea, y las páginas libres de la información y del mapa coinciden con las del modelo (F2, F4, F5, F6, F10).
- **S17** — **AC-F17** — Dado un almacén válido cuyo encabezado lleva el formato `4`, cuando se abre, entonces falla con la clase de formato distinto como formato anterior y queda idéntico byte a byte; y la información de un almacén nuevo da el formato `5` (F11).
- **S18** — **AC-F18** — Dado un almacén en modo temporal, cuando se repiten AC-F1, AC-F2, AC-F3 sin la parte diferida ni la de reabrir, AC-F4 sin la parte de disco lleno, AC-F7, AC-F8, AC-F9 sin su última parte, AC-F10 sin reabrir, AC-F11 sin la parte de disco lleno, AC-F12 y AC-F15 sin la parte diferida ni la de reabrir, entonces dan el mismo resultado, con la memoria del almacén en lugar de `krummi.data` y de `krummi.temp` (F12).
- **S19** — **AC-F19** — Dado un almacén cerrado sin error, recortado con el almacén cerrado de modo que el archivo termina antes de la frontera y falta la página del mapa de una región con páginas libres, cuando se abre, entonces la segunda parte de las páginas libres de la información es cero, y una reserva que necesita esa región falla con la clase de daño y el número de esa página del mapa (F3, F10).
- **S20** — **AC-F20** — Dado el informe de cierre, cuando se revisa, entonces contiene los tiempos de la medición de F13 en Windows y en Linux (F13).

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S17 (format 5), info field | inline | done | `ccce542` |
| T2 | S1–S4, S7–S10, S13–S15, S18 | inline | done | `b303e7c` |
| T3 | S0, S5, S6, S11, S12, S16, S19 | inline | done | `d342ec0` |
| T4 | S20 (measurement), performance fix | inline | done | `707800f`, `f31070b` |
| T5 | docs, CI sweep, checks, verify, RDD, report | inline | done | `0c0df0c`, `7244491`, `885ef69`, `e9192c0` |

Route evidence: one cohesive unit of behavior (the allocator in the transaction layer plus a format bump), no
independent parallel units, so no writer trigger fires; the parent writes inline. Risk: item 1 (persisted format
change) → one independent verifier at the feature's end.

## Log

- **L1** (2026-10-24, user): "Haz el commit y comienza la implementación con ODD con un buen equilibrio entre
  simplicidad y eficiencia y mantenibilidad. Se valora DRY frente a WET."
- **L2** (2026-10-24 08:28): estimate 2 h – 3 h 30 min confirmed; engineering principles added to `CLAUDE.md`
  first (commit `cf03037`).
- **L3** T1 (`ccce542`): RED observed for the format tests (expected `5`, got `4`), then GREEN. The first commit of
  T1 was amended twice: `ac_p14` and two facade tests still asserted format `4` (the commit had been chained without
  waiting for the test result). AC-E14 of the grouped-journal PRD becomes `ac_f17_...` (format `4` is now older).
- **L4** T2 (`b303e7c`): code written before its tests, so no RED was observed for S1–S4, S7–S10, S13–S15, S18;
  13 tests then passed. Design: `map.rs` (bits; one run scan serves the search and the longest run), `space.rs`
  (layout of regions, frontier, free total, per-region hint, durable history for the information), `transaction.rs`
  (reserve, free, commit). Test-only region size (`set_region_pages`) per the PRD's criteria preamble. The first
  allocated page moves from 3 to 4 (the first region's map page is 3); `recovery.rs` tests adjusted.
- **L5** T3 (`d342ec0`): crash helpers (`Run`, `POWER_CUTS`, `copy_folder`) moved to `tests/common` (DRY). Added
  `ac_f6_the_store_reaches_the_frontier_after_power_cuts`; a mutation that disables the F3 frontier page made it
  fail (cut 15, `Flushed`), so it guards F3. AC10 sample step 397 (97 took over 60 s).
- **L6** T4 (`707800f`, `f31070b`): the first measurement showed 88.7 µs per single-page reservation; a probe showed
  1–2.5 µs inside a write and ~100 µs for the first reservation of each write: the region summary was recomputed by
  scanning the whole map on every search, and on every commit. Fix: keep only an upper bound of each region's
  longest free run, learned from a failed search and forgotten when a commit frees pages there. Result: 1.9 µs.
- **L7** F13 on Windows (release, 1,179,712 pages, half freed in loose holes; built in 28.9 s):

  | Pages | Free pages | Open (ms) |
  |---|---|---|
  | 1179712 | 652332 | 70.28 |

  | Reservation | First (µs) | Next, mean (µs) |
  |---|---|---|
  | 1 page | 9.2 | 2.0 |
  | range of 16 | 2743.5 | 5.0 |
  | range of 256 | 1971.5 | 74.4 |
  | range of 4096 | 13746.5 | 3211.3 |

- **L8** T5 (`0c0df0c`): README, crate docs, CI runs the new full AC10 sweep (Risk item 5). Suite after T2 on
  Windows: all passed. RDD assessment of `cf03037..0c0df0c`: high (escalated item 1, format and persisted pages;
  process signals in tests and CI).
- **L9** Linux (WSL 2 Ubuntu, clone `~/muninn-accept`, `wsl -d Ubuntu`) on `0c0df0c`: `cargo test --workspace`
  all passed. F13 (release, built in 12.9 s):

  | Pages | Free pages | Open (ms) |
  |---|---|---|
  | 1179712 | 652332 | 0.24 |

  | Reservation | First (µs) | Next, mean (µs) |
  |---|---|---|
  | 1 page | 8.6 | 2.8 |
  | range of 16 | 3073.5 | 6.3 |
  | range of 256 | 3288.6 | 97.8 |
  | range of 4096 | 6097.6 | 3002.4 |
- **L10** Independent verifier (Sonnet, high) on `0c0df0c`: fmt, clippy and `cargo test --workspace` clean (226
  passed); S1–S17 and S19 PASS, S18 PARTIAL (temporary-mode repeats of AC-F3, F4, F9, F10, F11, F12 missing; its
  probes passed), S20 pending (this report). Public API differential against `cf03037`: only `format_version` and
  `free_pages` differ. Blocker B1 (low): the published-commits queue was drained only by `info()`, ~24 B per
  commit otherwise. Advisories: AC-F9 never reopened; AC-F3 deferred variant freed durably; the frontier page
  skipped the maximum-size count; full AC10 sweep over 15 min locally. One correction batch, `7244491`: queue
  pruned at each commit (unit test), frontier page reserved, AC-F3/F4/F9/F10/F11/F12 repeated in temporary
  stores, AC-F9 reopens, AC-F3 defers both commits, six probes kept as regression tests. Suites on `7244491`:
  Windows 233 passed, Linux (WSL) 233 passed.
- **L11** RDD review 1, `cf03037..7244491`, lenses risk, resilience, reliability, lineage
  `review-292159d53910b53e`: approved, acknowledged (authority burned). Findings: risk none; resilience none;
  reliability two WARNING, informational: (1) the AC-F16 model test degenerated once the frontier passed three
  regions (reservations stopped, most operations were empty writes): fixed in `885ef69` (single reservations
  from the map when full, frees as long as reservations, a final check that over a quarter of the operations
  reserved and freed pages; skipped syncs restored after each `testing::reset`; 20,000-operation sample in the
  suite, the 100,000 of AC-F16 in release as a CI acceptance step, 13 s); (2) crashes only after making
  everything durable: kept, AC-F6 covers crashes before durability.
- **L12** RDD review 2, `7244491..885ef69` (assessed high: CI shell step), lenses reliability, resilience, lineage
  `review-b07c5edfa310785a`: approved, acknowledged. Findings, all non-blocking: R3-001 WARNING (reliability), when
  the store is full the model may count a page freed by a commit not yet durable as reusable: false premise, F6
  reuses from the point of no return; an assertion that a full store reserves from the map was added in
  `e9192c0` and holds. R3-002 SUGGESTION, say that crashes follow `make_durable`: already the case, no change.
  R4 SUGGESTION (resilience), restore the fault state after the run: the state is per thread; a final
  `testing::reset` was added anyway in `e9192c0`.
- **L13** RDD review 3, `885ef69..e9192c0` (assessed under budget; reviewed as the last slice), lens reliability,
  lineage `review-bb98e2f82312eeec`: approved, acknowledged. Finding: SUGGESTION, a drop guard for the reset if
  the run panics: not taken, the fault state dies with the test's thread.
- **L14** Final checks on `e9192c0`: fmt and clippy clean; `cargo test --workspace` 233 passed (5 ignored acceptance
  tests) on Windows and 233 passed on Linux (WSL). AC-F16 in full (100,000 operations, release): 11.5 s.

## Closing

Closing report: [free-space-report.md](free-space-report.md). Active time 1 h 17 min against an estimate of
2 h – 3 h 30 min.
