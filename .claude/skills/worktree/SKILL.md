---
name: worktree
description: "Trigger: worktree, Crea un worktree para el cambio, ¿Qué worktrees tengo?, Fusiona el cambio en la base, Elimina el worktree, Descarta el worktree, Crea la base de ensayo, Descarta el ensayo. Route each phrase to its script."
license: MIT
metadata:
  author: inercya
  version: "1.0"
---

## Activation Contract

Load when the developer says one of the phrases below, or asks to create, list, merge, remove or discard a
worktree (a copy of this repository for one change) or a rehearsal base. A copy named `<n>` owns branch
`feat/<n>`, folder `C:\wt\muninndb-<n>`, Engram project `muninndb-<n>`, its git-ignored `work`
folder and, once discarded, tag `exp/<n>`. The scripts hold all the logic; this skill only routes.

## Hard Rules

- Only the scripts in `scripts/worktree/`, run from the session's root exactly as in the table, with no other
  parameters (`-Root`, `-NoOpen` and `-ClaudeProjectsDir` keep their defaults). Never improvise git commands
  for these operations.
- Never use Claude Code's built-in worktrees (`EnterWorktree`, `--worktree`, worktree isolation).
- Find the session first: `.git` at the session root is a folder in the main copy and a file in a copy.
- Relay every script message verbatim, in Spanish; never paraphrase or retry it, except the merge's exit 3.
- Never force a removal, never stop or kill a process, never delete files, branches or tags by hand.
- Never move a base by hand (checkout, merge, reset, update-ref or push on it): only `Merge-Worktree.ps1`
  advances it. Never push.
- In a copy, acceptance runs and generated datasets go in the copy's `work` folder.

## Decision Gates

| The developer says | Session | Run |
|---|---|---|
| `Crea un worktree para el cambio <n>` | main | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/New-Worktree.ps1 -Name <n>` |
| `Crea un worktree para el cambio <n> sobre <base>` | main | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/New-Worktree.ps1 -Name <n> -Base <base>` |
| `¿Qué worktrees tengo?` | any | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/Get-Worktrees.ps1` |
| `Fusiona el cambio en la base` | copy | Merge steps 1-5 below |
| `Elimina el worktree <n>` | main | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/Remove-Worktree.ps1 -Name <n>` |
| `Descarta el worktree <n>` | main | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/Remove-Worktree.ps1 -Name <n> -Discard` |
| `Crea la base de ensayo <base>` | main | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/New-RehearsalBase.ps1 -Base <base>` |
| `Descarta el ensayo <base>` | main | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/Remove-Rehearsal.ps1 -Base <base>` |

Wrong session: say where to ask for it (the main copy's session, or the session of `C:\wt\muninndb-<n>`)
and stop. Change nothing.

## Execution Steps

**Every operation except the merge**: run its command and relay the output verbatim. After a creation, tell
the developer to start a new agent session in the editor window it opened, at the copy's root.

**Merge**, in the copy, with the work committed:

1. Read the base with `git config --get branch.feat/<n>.worktreebase`, then `git merge --no-edit <base>` into
   `feat/<n>`. Resolve conflicts with the developer and commit the resolution.
2. Run the full checks: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`. Any failure: stop, list what failed and skip step 3.
3. `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/Merge-Worktree.ps1`
4. Exit 3 (the base has commits the branch lacks): back to step 1 once; a second exit 3, relay it and stop.
   Any other non-zero exit: relay the message verbatim and stop.
5. Report the script's success message: the base advanced, the copy and its branch are kept, nothing pushed.

## Output Contract

In the developer's language: what was created, listed, merged, removed or discarded, or the script's message
verbatim and the next action from `docs/worktrees.md`.

## References

- `docs/worktrees.md` — the whole flow, the rehearsal and what to do for every message.
- `docs/prd/worktrees/worktrees.prd.md` — product contract (Spanish, authoritative).
- `docs/methodology/prompts-worktrees.md` — the Spanish phrases and the "startswith" experiment order.
