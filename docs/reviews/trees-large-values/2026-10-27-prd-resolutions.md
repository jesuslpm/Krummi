# Trees large values PRD review resolutions

Resolutions of [2026-10-27-prd.md](2026-10-27-prd.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-10-27

Artifact versions after the fixes (uncommitted working tree, SHA-256 of the bytes):

- `docs/prd/krummi/trees-large-values.prd.md`: `5d61fd7c429f4cd1bc8c948fa37271e05ae05f091f1479540b9fbc13cf08f2ae`
- `docs/prd/krummi/trees-large-values.prd.en.md`: `d1c302b6e6020b669e43640887393f20dac75d6e2f5ba57326e0774823cfe794`

No product rule changed: every fix tightens an acceptance criterion or adds the generated section.

### R1 — major: AC-V5 requires the pages in use from before the tree was created without AC-A15's condition

- Disposition: fixed.
- Evidence: AC-A15 (`docs/prd/krummi/trees-keys.prd.md`, criterion 15) returns to the pages in use from before the
  first cycle only "cuando la entrada de `x` no hizo crecer el catálogo (las pruebas lo saben por la
  implementación)". AC-V5 required it whenever the tree is deleted or overwritten with Renombrar.
- Fix: AC-V5 now reads "el valor final es el de antes de crearlo cuando su entrada no hizo crecer el catálogo (las
  pruebas lo saben por la implementación), como en AC-A15".
- Status: claimed fixed, unverified until a follow-up review.

### R2 — major: AC-V3 does not cover a value whose range would cross two growth chunks in temporary mode

- Disposition: fixed.
- Evidence: AC-V3 only covered a value stored with the store about to grow, "de modo que sus páginas serían las
  primeras de un trozo nuevo". That is a value that starts a new chunk, not one that would cross the boundary.
- Fix: AC-V3 now names the case "en modo temporal, un valor guardado cuando el almacén tiene que crecer y su rango,
  con páginas consecutivas, atravesaría el límite entre la memoria que el almacén ya tenía y la que añade al crecer".
  It requires, for every case, that "el valor se devuelve como una sola secuencia contigua". How contiguity is
  achieved stays in *Dejado a la implementación*, so no implementation constraint is added.
- Status: claimed fixed, unverified until a follow-up review.

### R3 — minor: AC-V3 does not tell `krummi.data` apart from committed versions still in `krummi.temp`

- Disposition: fixed.
- Evidence: AC-V3 said "en una lectura en disco, dentro de la proyección de `krummi.data`". It only mentioned the
  versions area for an overwritten value, not for a committed version not yet written to `krummi.data`.
- Fix: AC-V3 now lists three disk cases. A version already written in place in `krummi.data` is inside its
  projection. A committed version not yet written to `krummi.data`, and an earlier version a read still sees after
  a later overwrite, are both inside the `krummi.temp` versions area (J7, T14, T16). Temporary mode has the same
  cases in the store's memory or in the memory that keeps the versions.
- Status: claimed fixed, unverified until a follow-up review.

### R4 — minor: the PRD has no elaboration time section

- Disposition: fixed.
- Evidence: by the `prd` skill, `scripts/prd_time.py` writes that section as the last step of the session, never
  by hand. The review arrived before that step.
- Fix: `python .claude/skills/prd/scripts/prd_time.py docs/prd/krummi/trees-large-values.prd.md
  docs/prd/krummi/trees-large-values.prd.en.md` (exit 0) wrote *Tiempo de elaboración* and *Elaboration time*.
  Session `1927218b`: 2 h 4 min wall clock, 46 min discarded, 1 h 18 min active (30 min agent, 48 min person), 14
  interventions.
- Status: claimed fixed, unverified until a follow-up review.

**Review record**: [2026-10-27-prd.md](2026-10-27-prd.md), independent review pasted by the user; there is no
wrapper status or `threadId`. The reviewer's model and effort were not stated. The Codex round 1 run earlier the
same day ended with wrapper status `error` (Codex usage limit, retry after 11:36), `threadId`
`01a17c68-188d-7a33-bb80-63e40d09cd78`, with no report and no changes. It is not an approval. Its telemetry
showed coordinator `gpt-6.1-sol`/`high` and reviewer `krummi-review-critical` `gpt-6-astra`/`xhigh`.

## 2026-10-28 (verification)

Report: [2026-10-28-prd.md](2026-10-28-prd.md), follow-up of the fixes. The PRD bytes reviewed match the SHA-256
recorded above, and they are the bytes committed in `27a5e5b`. Its verdict, quoted: "Ready for implementation within
the requested follow-up scope."

- R1: verified resolved.
- R2: verified resolved.
- R3: verified resolved.
- R4: verified resolved. The report checked each section against `render_section`.
- No new findings and no open product questions.

Review record: wrapper status `approved`, `threadId` `01a182ee-7b66-7171-887e-dea4d26e99f5`. The telemetry reports
coordinator `gpt-6.1-sol`/`high` and reviewer `krummi-review-critical` `gpt-6-astra`/`xhigh`.
