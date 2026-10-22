# MuninnDB

An embedded, transactional storage engine in Rust, inspired by [Voron](https://github.com/ravendb/ravendb),
the storage engine of RavenDB. The name follows Voron's: *voron* is Russian for raven, and Muninn is the raven
of memory in Norse mythology.

Nothing is implemented yet. Every product change starts with a PRD in `docs/prd/muninndb/` (see `CLAUDE.md`).

## Layout

| Path | Contents |
|---|---|
| `CLAUDE.md` | Agent instructions: workflow, checks, closing and worktree rules |
| `.claude/skills/` | Project skills: `prd`, `vibe-coding`, `closing-report`, `worktree` |
| `.claude/hooks/` | Guard that keeps a worktree session inside its own copy |
| `docs/methodology/` | The working method (PRD, vibe coding, ODD, SDD) and its slides |
| `docs/prd/muninndb/` | Product PRDs (Spanish authoritative, `*.prd.en.md` translations); created by the first PRD |
| `docs/worktrees.md` | Guide to working on several changes at once in worktree copies |
| `scripts/worktree/` | Scripts behind the `worktree` skill, with their tests |
| `scripts/slides/` | Speaker-notes audio for the methodology slides |

## Tests of the tooling

```powershell
python -m unittest discover -s .claude/hooks -p "test_*.py"
python -m unittest discover -s .claude/skills/closing-report/scripts -p "test_*.py"
python -m unittest discover -s .claude/skills/prd/scripts -p "test_*.py"
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/tests/Test-Worktree.ps1
```
