# Remembered layout check of temporary-area versions, and reads that do not wait for each other

Objective: implement [pager-version-checks.prd.md](../../docs/prd/krummi/pager-version-checks.prd.md) (U1–U5), item 2
of the investigation of 2026-11-08. Branch `perf/cheap-gaps`, after the cheap gaps run
([cheap-gaps.md](cheap-gaps.md)). Delivery: single branch, unmerged. Test runner: `cargo test`. Phase log and closing
report shared with the cheap gaps run.

## Specs

- **S1** (U1, verbatim): "La primera lectura de un nodo desde una versión confirmada que está en el área temporal
  comprueba su organización, como hoy, la haga una transacción de lectura o de escritura. Si la pasa, las lecturas
  siguientes de esa misma versión, de cualquier transacción, no vuelven a comprobarla mientras la versión siga en el
  área temporal." Failures are never remembered; a copy to modify is a change (S1 of the writer checks); the memory
  reserves nothing and does not count for the maximum size.
- **S2** (U3, verbatim): "encontrar la página que ve una transacción no hace esperar a una lectura por otras lecturas,
  de la misma transacción o de otras."
- **S3** (U4): no visible change; format `9`.
- **S4** (acceptance criteria AC-U1–AC-U7 of the PRD), in a store on disk and in temporary mode, in a normal tree, a
  fixed-size tree and a multivalue subtree.
- **S5** (U5 and AC-U8): single-load pairs before and after, Windows and WSL 2; W12 improves on both; none of W2, W7,
  W10, W12, W13, W14, W19 gets worse.

## Tasks

| ID | Specs | Route | Status | Evidence |
|---|---|---|---|---|
| V1 | S1, S3, S4 | inline: per-version layout memory in the slot, `PageCheck::Version` | pending | |
| V2 | S2, S3 | inline: shared lock for page lookups in the scratch area | pending | |
| V3 | S5 | inline: single-load pairs, with the cheap gaps pairs | pending | |

## Log

- **L1** (2026-11-08): user's request, verbatim: "Cuando termines haz el 1 y 2 sin preguntar y cambia el PRD con tus
  recomendaciones sin preguntar también todo de seguido. Voy a estar fuera todo el día." PRD written with the
  reviewer's findings resolved by the agent's defaults (authorized by that message); Codex's PRD review failed with its
  usage limit until 2026-11-11 06:49 (`codex-review` log `20261108-072807-pager-version-checks-prd`), pending.
  Start: 2026-11-08 07:29:24 UTC.
- **L2** (2026-11-08): routes. Both units inline: sequential and small. V2 touches concurrency (high-risk item 4): it
  gets an independent verifier and its own RDD review.
