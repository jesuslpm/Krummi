# Grouped and direct journal writes — ODD execution document

Objective: implement `docs/prd/krummi/journal-group-commit.prd.md` (decisions E1–E16, criteria AC-E1–AC-E18), the
authoritative specification; its ancestry is `journal.prd.md` and `krummi.prd.md`.
Branch: `feat/journal-group-commit`. Delivery: `ask-on-risk`. Test runner: `cargo test --workspace` (Windows; Linux
in WSL). Estimate: 3–5 h. Real start: 2026-10-23 11:05:52 +0100.

## Specs

Each spec quotes the PRD verbatim; the decisions it names (E#) are part of it.

- **S1 — Point of no return and turn hand-off (E2, E3).** AC-E1: "Dada una confirmación en modo durable retenida
  después de su punto sin retorno, cuando otro hilo pide una escritura, entonces la obtiene sin esperar y ve sus
  cambios; una lectura abierta en ese momento no los ve; y cuando la retenida devuelve sin error, una lectura
  abierta después por el mismo hilo los ve (E2, E3)."
- **S2 — Grouped commits (E4, G4).** AC-E2: "Dadas 8 confirmaciones en modo durable de 8 hilos retenidas en la cola
  del diario, que caben en el archivo actual del diario, cuando se reanuda, entonces todas devuelven sin error, el
  diario las escribió en una sola escritura, una lectura abierta antes de reanudar no ve ninguna y una abierta
  después de que la primera devuelve las ve todas; y dados 16 hilos que confirman a la vez 1.000 escrituras en modo
  durable cada uno, todas devuelven sin error, nunca hubo dos escrituras abiertas a la vez y, tras matar el proceso
  y reabrir, están todas (E4, G4)."
- **S3 — Power cuts with groups (E8, E9).** AC-E3: "Dada una secuencia conocida de confirmaciones en los dos modos,
  que la prueba agrupa en grupos de una, dos y tres, cuando se simula un corte de corriente en cada punto en que se
  escribe o se lleva al disco el diario, entonces al reabrir abre sin error y el conjunto de confirmaciones
  presentes es un prefijo de la secuencia que incluye todas las de modo durable que habían devuelto sin error y
  todos los grupos anteriores al que se escribía (E8, E9)."
- **S4 — Disk full at reservation (E2, E7).** AC-E4: "Dada una confirmación que necesita un archivo nuevo del diario,
  sin espacio en disco para reservarlo y con otra petición de escritura esperando, cuando se confirma, entonces
  falla con la clase de disco lleno, la escritura que obtiene el turno no ve sus cambios, y al liberar espacio una
  confirmación siguiente termina sin error sin reabrir; y si se mata el proceso tras el fallo, la apertura siguiente
  abre sin error de daño (E2, E7)."
- **S5 — Group failure (E8).** AC-E5: "Dadas una diferida que ya devolvió y tres confirmaciones en modo durable
  retenidas en la cola del diario, y una escritura abierta que ve sus cambios, cuando falla la escritura de ese
  grupo, entonces las tres fallan con la clase de entrada y salida, la escritura abierta falla al confirmarse con
  cambios con esa clase, las peticiones de escritura siguientes también, y una lectura nueva no ve ninguna de las
  cuatro; y al reabrir sin el fallo, las presentes son un prefijo de las cuatro y todas las anteriores están (E8)."
- **S6 — Panic after the point of no return (E2).** AC-E6: "Dada una confirmación en modo durable que la prueba
  interrumpe con un pánico en cada punto posterior a su punto sin retorno, cuando su grupo es durable, entonces una
  lectura nueva ve sus cambios y, al reabrir, están (E2)."
- **S7 — Deferred commit (E3, E5).** AC-E7: "Dada la escritura del diario detenida, con una confirmación en modo
  durable retenida en la cola del diario, cuando otro hilo obtiene el turno y confirma una diferida, entonces
  devuelve sin esperar; la escritura siguiente ve las dos; una lectura que ese hilo abre después de que la diferida
  devuelve no ve ninguna; y cuando se reanuda y el grupo es durable, una lectura nueva ve las dos (E3, E5)."
- **S8 — Same durability in both modes (E4, E5).** AC-E8: "Dadas una diferida y después una en modo durable retenidas
  en la cola del diario, cuando se reanuda, entonces el diario las escribe en una sola escritura, y un corte de
  corriente simulado después de que la durable devuelve conserva las dos; y dada una diferida sola, un corte de
  corriente simulado después de que su grupo es durable, sin otra operación, la conserva (E4, E5)."
- **S9 — Reserved files (E7).** AC-E9: "Dado un almacén con una confirmación con cambios, cuando se confirman otras
  que caben en el mismo archivo del diario, entonces el tamaño del archivo no cambia; y una confirmación mayor que
  el tamaño habitual de un archivo termina sin error y se recupera tras una caída (E7)."
- **S10 — Close (E12).** AC-E10: "Dadas diferidas retenidas en la cola del diario, cuando se cierra el almacén y la
  prueba reanuda la escritura durante el cierre, y en otra prueba cuando se deja de usar sin cerrarlo, entonces el
  cierre no devuelve hasta que se escriben, la carpeta queda solo con `krummi.data` y al reabrir están todas; y con
  un fallo simulado en la escritura del grupo que deja de producirse antes del cierre, el cierre termina sin error y
  al reabrir están todas las diferidas; y con el fallo persistente, el cierre falla con la clase de entrada y salida
  y la apertura siguiente sin el fallo abre con un prefijo de ellas (E12)."
- **S11 — Commit without changes (E11).** AC-E11: "Dada una escritura sin cambios que vio una confirmación en modo
  durable retenida en la cola del diario, cuando se confirma en modo durable, entonces no devuelve hasta que la
  retenida es durable; si su grupo falla, falla con la clase de entrada y salida; y en modo diferido, o sin
  confirmaciones pendientes, termina al instante sin error (E11)."
- **S12 — Make durable (E10).** AC-E12: "Dada una confirmación en modo durable retenida en la cola del diario, cuando
  otro hilo llama a la operación de J11, entonces no devuelve hasta que se reanuda la escritura y la retenida es
  durable (E10)."
- **S13 — Mechanism failures (E13).** AC-E13: "Dado un fallo simulado al poner en marcha lo que escribe el diario,
  cuando se abre el almacén, entonces falla con la clase de memoria agotada y el almacén queda idéntico byte a byte;
  y dado un fallo simulado de ese mecanismo con confirmaciones esperando, entonces fallan con la clase de entrada y
  salida, las peticiones siguientes también, y al reabrir están todas las que habían devuelto sin error (E13)."
- **S14 — Format 4 (E15).** AC-E14: "Dado un almacén válido cuyo encabezado lleva el formato `3`, cuando se abre,
  entonces falla con la clase de formato distinto como formato anterior y queda idéntico byte a byte; y la
  información de un almacén nuevo da el formato `4` (E15)."
- **S15 — Measurement (E16).** AC-E15: "Dado el informe de cierre, cuando se revisa, entonces contiene las
  confirmaciones por segundo de la medición de E16 con 1, 4 y 16 hilos, con escritura directa y con la alternativa,
  en Windows y en Linux (E16)."
- **S16 — Damage per group (E9).** AC-E16: "Dados dos grupos de tres confirmaciones en modo durable que devolvieron
  sin error, en un almacén cerrado por una caída sin trasladar, cuando se altera un byte de la segunda confirmación
  del último grupo, entonces abre sin error con el primer grupo y la primera confirmación del último, sin las otras
  dos; y cuando se altera un byte de una confirmación del primer grupo, la apertura falla con la clase de daño y los
  archivos del almacén quedan idénticos byte a byte (E9)."
- **S17 — Full journal queue (E2).** AC-E17: "Dada una confirmación que espera sitio en la cola del diario llena, con
  confirmaciones retenidas en ella, cuando otro hilo pide una escritura, entonces no la obtiene mientras la primera
  espera; y cuando un fallo simulado al llevar `krummi.data` al disco (J13) deja el almacén sin aceptar escrituras
  durante esa espera, la confirmación falla con la clase de entrada y salida sin aplicar nada, y al reanudar las
  retenidas se escriben y una lectura nueva las ve (E2)."
- **S18 — Temporary mode (E14).** AC-E18: "Dado un almacén en modo temporal, cuando se confirma una diferida,
  entonces una lectura abierta después de que devuelve la ve (E14)."
- **S19 — Inherited criteria.** "Los criterios del Diario, de Transacciones y del padre raíz siguen valiendo y los
  del Diario se repiten con escritura directa y con la alternativa", with the PRD's listed changes to AC-J7, AC-T19,
  AC-J14, AC6, AC-T3, AC-T5, AC-J3, AC10 and AC-J9. Checks: `cargo fmt --all --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo test --workspace`, on Windows and Linux.

Limits: *Fuera de alcance* of the PRD; the only decisions taken without asking are those in *Dejado a la
implementación*.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S14 | inline | done | `6f419c7` |
| T2 | S9, S19 (direct I/O, fallback, reservation; test hooks shared with the journal thread) | inline | done | `92b6345` |
| T3 | S2, S3, S8, S10, S12, S13, S16, S17 (journal thread and groups; readers see durable commits) | inline | pending | |
| T4 | S1, S4, S5, S6, S7, S11, S18 (turn released at the point of no return, E11, failure listener) | inline | pending | |
| T5 | S15 (measurement on Windows and Linux) | inline | pending | |
| T6 | S19 (docs, both platforms, independent verification) | inline + verifier | pending | |

Route evidence: one behavior spread over dependent deliveries (each needs the previous one); no parallel independent
units and no context backstop, so no Writer trigger. Risk: items 1 (journal format, format 4) and 4 (concurrency):
independent verifier at the end and RDD review per high-risk commit.

## Log

- **L1** (2026-10-23, user, verbatim): "Impleméntalo con ODD". Estimate 3–5 h, answer "Adelante".
- **L2** Starting state checked against the code: the whole commit runs on the committing thread (write, sync,
  publish, hand-off, transfer); no threads, no direct I/O, files grow by appending; fault injection is per thread
  (`sys/faults.rs`), so the journal thread must share the opener's fault state.
- **L3** Defaults taken (left to the implementation): the transfer stays on the committing thread after its commit
  returns; the header's applied commit never passes the durable one, so a transfer no longer waits for the journal;
  journal files are reserved with `set_len`/`fallocate` (no zero fill) and the direct-write probe runs when a file is
  created; the fallback is forced in tests with an environment variable read only by test-hook builds.
- **L4** T1: RED `ac_e14_format_3_is_a_previous_format_and_new_stores_have_format_4` (format was 3); GREEN with
  `FORMAT_VERSION = 4`. RDD assess: medium, `under_budget` (kept in the slice).
- **L5** T2: RED `ac_e9_journal_files_keep_their_reserved_size` against the old journal (file one record long);
  GREEN. Suite 170 passed on Windows with direct writes and with `KRUMMI_JOURNAL_BUFFERED=1`. Default taken: builds
  with test hooks use 256 KiB journal files (`RELEASE_FILE_LIMIT` stays 16 MiB), because the AC10 sweeps and the
  power-cut simulation copy and alter whole files. In this intermediate commit a direct write of a deferred commit
  still counts as pending until J11 (no thread yet), so AC-J8 keeps its meaning.
- **L6** RDD review of T1–T2, recorded when it ended:

  | Commits | Lenses | Result | Findings and resolution | Lineage |
  |---|---|---|---|---|
  | `d88a74a..92b6345` | risk, resilience, reliability | approved | WARNING R1-001, R4-direct-fallback-open-only and R3-001 (same issue): the fallback triggers only when the direct open fails, not when a file system accepts the open and rejects the write, and no test forces it; fixed in T3 with a probe write when the file is created and a test that forces it. SUGGESTION R3-002: nothing checks that `ALIGN` is a multiple of `DIRECT_ALIGN`; fixed in T3 with a compile-time assertion. | `review-a5c8fce6cba72e73` |
