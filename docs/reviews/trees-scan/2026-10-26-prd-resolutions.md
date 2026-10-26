# Trees scan PRD review resolutions

Resolutions of [2026-10-26-prd.md](2026-10-26-prd.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-10-26

Artifact versions after the fixes (uncommitted working tree, SHA-256 of the bytes):

- `docs/prd/krummi/trees-scan.prd.md`: `9ac49e38b02e58b2f12ec29306fa33da008914133e426ae794c13132bc45c333`
- `docs/prd/krummi/trees.prd.md`: `fc798bd5130df67f0b337ce530d91803252859d97ca527bb21f67e47c31df067`
- `docs/prd/krummi/trees-keys.prd.md`: `3bf17c869dc3fb89ab9817e4eac210bbd3ae1c2e03f83094a4a3788b7b8fbc99`

### R1 — minor: AC-R14 does not demonstrate the full compatibility promised by R10

- Disposition: fixed.
- Fix: AC-R14 now also opens the store whose ranges 5.2 removed with the 5.1 version, reads the tree list, every
  remaining key and the entry count, and stores and removes entries until pages split and merge, then commits and
  reopens, all without error. That part is an execution check with the 5.1 version built separately and its
  result in the closing report, not a committed test. Committed tests do not build or run earlier versions.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — minor: the announced annotations to earlier PRDs are missing

- Disposition: fixed.
- Fix: `trees.prd.md` carries a dated 2026-10-26 annotation block for B5, B7 and B8. `trees-keys.prd.md` carries
  one for A7, A9 and A10. Both link to `trees-scan.prd.md` and name the R decisions behind each note.
- Status: claimed fixed, unverified until a follow-up review.

### R3 — minor: starting state omits the internal traversal used by Listar

- Disposition: fixed.
- Fix: the *Recorrido interno* row of *Estado de partida* now names the catalog traversal used by Listar. It also
  names the traversal that frees the pages of a deleted or emptied tree, and the one used in tests. It adds that
  no traversal covers part of a tree or runs in descending order.
- Status: claimed fixed, unverified until a follow-up review.

### R4 — minor: R3 overstates the difference from Rust's range behavior

- Disposition: fixed.
- Fix: R3 now limits the comparison to Rust's actual panic conditions. Those are a lower bound after the upper
  one, or equal bounds that are both excluded. Krummi's empty-interval rule is unchanged.
- Status: claimed fixed, unverified until a follow-up review.

## 2026-10-26 (verification)

- The follow-up review [2026-10-26-prd-02.md](2026-10-26-prd-02.md) states "R1–R4 are verified resolved. No
  original finding remains open; the PRD is ready for implementation within the reviewed scope."
