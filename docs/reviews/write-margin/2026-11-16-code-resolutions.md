# Write margin code review resolutions

Resolutions of [2026-11-16-code.md](2026-11-16-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-16

Artifact versions: reviewed range `6b49bea..1ee31df`; after it, `687ef64` (an attempted fix of R1) and `b5b4e29`
(its revert), so the code at `b5b4e29` is the reviewed code; `0`-line diff of `crates/` against `1ee31df`.

### R1 — major: W20 regression remains unresolved against S4

Disposition: pending, a product decision. The finding is right: S4 asks for no load slower, and W20 is. An attempted
fix did not hold. `687ef64` gave `multi::entry` its own descent again instead of calling `located_entry`, after an
A/B of five rounds measured 119.9 ns per value for it against 121.5 on `main` and 124.5 for the reviewed head
(`work/wm/ab4-result.json`). Two later sessions contradicted it: the final head with that change measured 121.5
against 114.9 on `main` (`ab5-result.json`, three rounds), and in one session of five rounds the same code built in
two folders measured 122.7 and 121.3 against 118.7 on `main` (`ab6-result.json`). Measured with `work/wm/ab.py`:
full scale, build order alternating, performance cores, bench power plan, digests equal in every run. So the
duplication bought nothing measurable and was reverted (`b5b4e29`). What holds over five sessions: the branch reads
W20 2 to 5 % slower than `main` in every session but one (L8, L9 of
[write-margin.md](../../../odd/tasks/write-margin.md)); the cause is not shown: W20 reads a store with no
scratch-area versions, so neither the hash nor the version mark runs on its path, and `#[inline]` on
`located_entry` did not change it. The other loads that share the changed code are not slower (L9). Whether to
accept that W20 cost for W19's 10 % or to keep looking (a profile of W20 on both builds) is the user's call;
S4 was not changed.

Status: pending.

### R2 — pre-existing advisory: reused ScratchWriter snapshots are omitted from version GC

Disposition: pending, outside this run's scope, as the review classifies it: the omission of the writer's snapshot
from `collect_garbage` and the reuse of a `ScratchWriter` after `publish` exist at `6b49bea`, and no sequence through
the store's operations reaches it (`WriteTransaction::commit` consumes its writer). It is listed in the closing
report as a separate follow-up of the pager; the run changes nothing there.

Status: pending.

Review record: [2026-11-16-code.md](2026-11-16-code.md), wrapper status `findings` (R1 open; no open question),
thread `01a1e468-b45c-7e03-8df1-1926523834ae`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` (Noether) `gpt-6-astra` / `xhigh`.

## 2026-11-16 (verification)

Follow-up report: [2026-11-16-code-02.md](2026-11-16-code-02.md), over `1ee31df..864d430`, wrapper status `findings`,
thread `01a1e48d-c7ac-78a1-a73d-79cd9982e864`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` (Avicenna) `gpt-6-astra` / `xhigh`. Verdict, quoted: «Changes requested: R1 remains open.
R2 remains a non-blocking pre-existing advisory.»

- R1: «still open; not verified resolved». Codex: «resolve the W20 regression and verify the final-head A/B;
  relaxing S4 requires an explicit user decision.» Status: pending, a product question for the user after the loop.
- R2: pending, pre-existing, outside the run.

## 2026-11-16 (user's decision on R1)

### R1 — major: W20 regression remains unresolved against S4

Disposition: resolved by the user's decision, as the review asked («relaxing S4 requires an explicit user
decision»). The user first chose to profile W20 (L10 of [write-margin.md](../../../odd/tasks/write-margin.md)): in 20
alternated rounds the branch reads W20 2.8 % slower than `main` (130.4 against 126.8 ns per value, slower in 18 of
20, `work/wm/ab7-result.json`), and with both builds compiled with `-C llvm-args=-align-loops=64` the gap is gone
(126.2 against 125.8, slower in 5 of 10, `ab8-result.json`): it is the alignment of a hot loop of the multi-value
read, not work the change adds. The user then chose «Aceptar y cerrar» (L11): S4 now names W20 as accepted 2–3 %
slower in the default build; aligning loops is a separate follow-up.

Status: claimed, unverified until a follow-up review.


## 2026-11-16 (verification, round 3)

Follow-up report: [2026-11-16-code-03.md](2026-11-16-code-03.md), over `864d430..91a1e17`, wrapper status
`findings` (only R2 open), thread `01a1e519-413c-7e31-ac94-102cd8b42905`. Telemetry: coordinator `gpt-6.1-sol` /
`high`; reviewer `krummi-review-critical` (Kant) `gpt-6-astra` / `xhigh`. Verdict, quoted: «R1 is resolved; R2
remains an open pre-existing advisory. No correction-induced defect found.»

- R1: verified resolved.
- R2: pending, pre-existing, outside the run; the loop's three automatic rounds are used, and R2 is listed in the
  closing report as a follow-up of the pager.
