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
| T1 | C1–C8; S1, S4, S5 at the engine | inline (no writer trigger: one unit) | pending | |
| T2 | S1–S9 at the public API | inline | pending | |
| T3 | C9, S10: measurement example, Windows and Linux runs, default size | inline | pending | |
| T4 | Docs, closing report, reviews | inline | pending | |

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
