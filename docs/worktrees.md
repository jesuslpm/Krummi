# Worktrees: one change per copy, side by side

Work on several changes at once, each in its own copy of the repository (a git worktree) with its own agent
session, without one copy breaking, slowing down or polluting another. The whole flow can be rehearsed on a
throwaway base and discarded without leaving a trace in `main`. You ask the agent with fixed Spanish phrases;
the project's `worktree` skill runs the matching script in `scripts/worktree/`. Windows only.

- Product contract: the worktrees PRD of the chicago-crime repository (`docs/prd/worktrees/worktrees.prd.md`),
  where this tooling was built. MuninnDB copies have no reference CSV, so its CSV rules do not apply here.
- The Spanish prompts for every operation: [prompts-worktrees.md](methodology/prompts-worktrees.md).

## Quick path

| Step | Session | Say (verbatim) | What the skill runs |
|---|---|---|---|
| Create a copy on `main` | main copy | `Crea un worktree para el cambio <n>` | `New-Worktree.ps1 -Name <n>` |
| Create a copy on another base | main copy | `Crea un worktree para el cambio <n> sobre <base>` | `New-Worktree.ps1 -Name <n> -Base <base>` |
| Work on the change | the copy, in a new session at its root | your usual PRD, ODD or vibe coding prompts | — |
| See every copy | any | `¿Qué worktrees tengo?` | `Get-Worktrees.ps1` |
| Merge the change into its base | the copy | `Fusiona el cambio en la base` | merge steps 1–2, then `Merge-Worktree.ps1` |
| Remove a merged copy | main copy | `Elimina el worktree <n>` | `Remove-Worktree.ps1 -Name <n>` |
| Discard an unmerged copy | main copy | `Descarta el worktree <n>` | `Remove-Worktree.ps1 -Name <n> -Discard` |
| Create a rehearsal base | main copy | `Crea la base de ensayo <base>` | `New-RehearsalBase.ps1 -Base <base>` |
| Discard the whole rehearsal | main copy | `Descarta el ensayo <base>` | `Remove-Rehearsal.ps1 -Base <base>` |

`<n>` is the change name: lowercase letters, digits and single hyphens, for example `startswith-odd`. A phrase
said in the wrong session is not run: the agent tells you where to ask for it and stops.

Without the agent, each script runs the same way from PowerShell at the root of the right copy:
`powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/<Script>.ps1 <parameters>`.

## Two kinds of session

| Session | Folder | `.git` at its root | What it does |
|---|---|---|---|
| Main copy | `C:\projects\MuninnDB` | a folder | Creates, lists and removes copies; creates and discards the rehearsal base. It does not work on changes. |
| A copy | `C:\wt\muninndb-<n>` | a file | Works on one change and merges it into its base. It never touches the main copy or other copies. |

Open one new agent session per copy, at the copy's root: the editor window that the creation opens is already
there. A session opened in a subfolder does not load the guard.

## What a copy gets

Everything derives from the name `<n>`:

| Element | Value |
|---|---|
| Branch | `feat/<n>`, created from the base |
| Folder | `C:\wt\muninndb-<n>` |
| Base | `main`, or the base given at creation; recorded in the shared git config as `branch.feat/<n>.worktreebase` |
| Private files | `.claude/settings.local.json` and `.atl/skill-registry.md`, copied from the main copy when present |
| Work folder | the `work` folder inside the copy, ignored by git: acceptance runs and generated datasets |
| Engram project | `muninndb-<n>`, assigned with `engram init` (its `.engram/` folder is ignored by git) |
| Build output | the copy's own `target` folder |
| Tag when discarded | `exp/<n>` |

The name must also keep every path of the copy within 259 characters; if it does not, the refusal message gives
the maximum length.

## The whole flow

### 1. Create a copy (main copy)

Say `Crea un worktree para el cambio <n>`, or `Crea un worktree para el cambio <n> sobre <base>` for a base other
than `main`.

1. Everything is checked before anything is created: that the session is in the main copy, that the name is
   valid and fits the Windows path limit, that the base exists, and that there is no branch, folder, registered
   copy or Engram project with that name yet. If anything fails, nothing is created.
2. The branch and the copy are created and the base is recorded.
3. The copy gets the private files, the `work` folder and its own Engram project.
4. The copy is checked clean for git, and a new editor window opens at its root.
5. The summary shows the folder, branch, base, Engram project and the phrase to start with.

If a step fails, the message names the failed step and what was already created. Fix the cause and clean up
with `Elimina el worktree <n>` from the main copy: the creation never undoes its own steps.

### 2. Work in the copy

In the new editor window, start a new agent session at the copy's root and work as usual (PRD, ODD or vibe
coding), committing on `feat/<n>`.

- The copy builds in its own `target` folder; its first build is a complete one.
- Acceptance runs and generated datasets use the copy's `work` folder, not the fixed folders of the README
  (such as `C:\acceptance`), which two copies would share. For example, from the copy's root:
  `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/acceptance/run-acceptance.ps1 -WorkDir work\acceptance`.
- The agent reads and writes memory only in the copy's Engram project.
- The project rule "merge into `main` after full verification" becomes "close the run without merging; merge
  into the copy's base only when the developer says `Fusiona el cambio en la base`, through the worktree merge"
  ([CLAUDE.md](../CLAUDE.md), *Working in a worktree*). The developer decides what is merged: in the experiment,
  only the ODD copy.
- The guard stops the agent from writing in the main copy or in other copies, and from moving the base by hand.

### 3. Merge the change into its base (the copy)

With the work committed, say `Fusiona el cambio en la base` in the copy's session. The agent:

1. Merges what is new in the base into the branch (`git merge --no-edit <base>`) and resolves any conflict with
   you.
2. Runs the full checks again: `cargo fmt --all --check`,
   `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`. A failure stops the
   merge here.
3. Runs `Merge-Worktree.ps1`. It checks that the copy is clean, that the branch contains the whole base and that
   it adds something new; then the base advances to the branch, without a merge commit and without the agent
   leaving its copy. If the base is checked out in another copy (usually the main copy), that copy is updated
   too, unless it has uncommitted changes: then the merge is refused and the base does not move.
4. On exit 3, goes back to step 1 once. On any other error, shows the message and stops.
5. Reports the result. The copy and its branch are kept. Nothing is ever pushed to the remote.

| Exit code | Meaning | Next |
|---|---|---|
| 0 | Done: the base advanced to the branch. | Nothing. Remove the copy when you no longer need it. |
| 3 | Refused: the base has commits the branch does not contain (for example, another copy merged first). | Back to step 1, once; the agent does it. |
| 1 | Any other refusal or error. | Read the message and see [Message → what to do](#message--what-to-do). |

The other scripts end with 0 when done and 1 on any refusal or error.

### 4. List the copies (any session)

`¿Qué worktrees tengo?` shows every copy: name, branch, base, whether the branch is already merged into its
base, folder and Engram project.

### 5. Remove or discard a copy (main copy)

Close the copy's editor window and its agent session first.

| Say | When | Removes | Keeps |
|---|---|---|---|
| `Elimina el worktree <n>` | The branch is merged into its base and the copy has no uncommitted changes. | The copy, its folder and its branch. | The Engram project and the Claude Code memory of that folder. |
| `Descarta el worktree <n>` | The branch is not merged and you want to throw it away. | First tags the branch tip as `exp/<n>`; then removes the copy, its folder, its branch, its Engram project and the Claude Code memory of that folder. | The branch history and its closing report, in the tag `exp/<n>`. |

- An unmerged branch is only removed when you ask to discard it.
- Uncommitted changes refuse both phrases, listing what would be lost. Commit them in the copy first so they end
  up in the merge or in the tag.
- Removing never forces anything and never closes a process: if a file is in use, the message names it. Close
  what uses it and say the same phrase again; repeating is safe.
- Only the Claude Code memory folder is deleted, never the session history.
- A plain removal keeps the Engram project `muninndb-<n>`, so a new copy with the same name is refused while
  that project exists.

## Rehearse the flow and discard the rehearsal

The rehearsal and the real run use exactly the same phrases; only the base changes.

1. From the main copy, `Crea la base de ensayo <base>` creates the branch `<base>` from `main`. It plays the part
   of `main` during the rehearsal.
2. Run the whole flow with the same phrases as the real run, adding `sobre <base>` when creating each copy.
   Copies merge into `<base>`, never into `main`.
3. At the end, close the editor windows of the rehearsal copies and, from the main copy, say
   `Descarta el ensayo <base>`. In one step it removes every copy whose base is `<base>`, their branches, their
   `exp/` tags, their Engram projects, the Claude Code memory of their folders, and `<base>` itself. Copies
   already removed during the rehearsal are included, so their kept Engram projects and memory go too. `main`
   does not change.

Discarding the rehearsal refuses `main` and refuses while a rehearsal copy has uncommitted changes, listing them.
It can be repeated safely, also when some copies are already gone.

### The "startswith" experiment

The same PRD is implemented twice at once, with vibe coding and with ODD; only the ODD copy is merged. `<base>`
is `ensayo` in the rehearsal and `main` in the real run; the change names never change.

| Step | Session | Say |
|---|---|---|
| 0 | main copy | `Crea la base de ensayo ensayo` (rehearsal only) |
| 1 | main copy | `Crea un worktree para el cambio startswith-prd sobre <base>` |
| 2 | `startswith-prd` (new session) | the usual prompt that asks for the startswith PRD |
| 3 | `startswith-prd` | `Fusiona el cambio en la base` |
| 4 | main copy | `Crea un worktree para el cambio startswith-vibe sobre <base>` |
| 5 | main copy | `Crea un worktree para el cambio startswith-odd sobre <base>` |
| 6 | `startswith-vibe` (new session) | the usual prompt that implements the PRD with vibe coding |
| 6 | `startswith-odd` (new session) | the usual prompt that implements the PRD with ODD, at the same time |
| 7 | `startswith-odd` | `Fusiona el cambio en la base` |
| 8 | main copy | `Descarta el worktree startswith-vibe` |
| 9 | main copy | `Elimina el worktree startswith-odd` |
| 10 | main copy | `Elimina el worktree startswith-prd` |
| 11 | main copy | `Descarta el ensayo ensayo` (rehearsal only) |

After step 11 nothing of the rehearsal is left, and the real run starts at step 1 with `<base>` = `main`. In the
real run there is no step 11: the vibe coding history stays in the tag `exp/startswith-vibe` and the ODD one in
`main`.

To confirm the rehearsal left no trace, note `git rev-parse main` before step 0 and check after step 11, from the
main copy:

- `¿Qué worktrees tengo?` lists no rehearsal copy.
- `git branch --list ensayo "feat/startswith-*"` and `git tag --list "exp/startswith-*"` print nothing.
- `git rev-parse main` prints the same commit as before step 0.

## The guard

In a copy, a `PreToolUse` hook (`.claude/hooks/worktree_guard.py`, wired in the versioned
`.claude/settings.json`) blocks the agent from:

- writing in the main copy or another copy: file tools on a path inside them, and shell commands that name them
  unless every part of the command is a read-only command;
- moving the base by hand with git: `checkout` or `switch` to it, `branch -f/-d/-D/-m/-M` on it,
  `update-ref refs/heads/<base>`, `push` or `fetch` into it, and `merge` or `reset` while on it. `main` is
  protected the same way even when it is not the copy's base.

In the main copy it blocks nothing; Python does not even start. Input it cannot read is never blocked. It only
acts in sessions opened at a copy's root. It runs with `python`, or with `python3` where there is no `python`
command (most Linux distributions); with neither, it does not run and every tool call shows
`Guarda de worktrees: no se encontró Python (python ni python3); la guarda no se aplicó.` Tests:
`python -m unittest discover -s .claude/hooks`.

## Scripts

| Script | Session | Parameters |
|---|---|---|
| `New-Worktree.ps1` | main copy | `-Name <n>` `[-Base main]` `[-Root C:\wt]` `[-NoOpen]` |
| `Get-Worktrees.ps1` | any | none |
| `Merge-Worktree.ps1` | the copy | none |
| `Remove-Worktree.ps1` | main copy | `-Name <n>` `[-Discard]` `[-Root C:\wt]` |
| `New-RehearsalBase.ps1` | main copy | `-Base <base>` |
| `Remove-Rehearsal.ps1` | main copy | `-Base <base>` `[-Root C:\wt]` |

`WorktreeCommon.psm1` holds the shared helpers. `-NoOpen` (do not open the editor) and `-ClaudeProjectsDir`
(default `~\.claude\projects`, on the scripts that delete Claude Code memory) are seams for the scripts' tests;
daily use leaves them, and `-Root`, at their defaults. Tests:
`powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/tests/Test-Worktree.ps1`.

## Accepted risks

- **The first build of each copy is a complete one.** It takes the same time in every copy, so it does not skew
  a comparison between them, but it counts in each run's time.
- **The guard only acts in sessions opened at a copy's root.** A session opened in a subfolder does not load it.

## Out of scope

Linux and other systems; pushing branches, opening PRs or merging on the remote; copying a discarded branch's
closing report or metrics into the base (they stay in its `exp/<n>` tag); sharing build output between copies
or building when a copy is created; changing the acceptance scripts, the generator or the query app; keeping the
Claude Code session history of a removed copy.

## Message → what to do

Every message the tools show, verbatim. The placeholders (`<n>`, `<base>`, `<carpeta>`, …) are filled in.

| Message | Meaning | What to do |
|---|---|---|
| `Esto se hace desde la copia principal, y esta sesión está en la copia '<carpeta>'. No se ha cambiado nada.` | Creating, removing or managing the rehearsal was asked from a copy. Nothing changed. | Say the phrase again in the main copy's session (`C:\projects\MuninnDB`). |
| `La fusión se hace desde la copia del cambio, no desde la copia principal. No se ha cambiado nada.` | The merge was asked from the main copy. Nothing changed. | Say `Fusiona el cambio en la base` in the session of the copy `C:\wt\muninndb-<n>`. |
| `Nombre '<n>' no válido: solo minúsculas, dígitos y guiones simples (por ejemplo, startswith-odd). No se ha creado nada.` | The name has characters that are not allowed. Nothing was created. | Choose a name with only lowercase letters, digits and single hyphens. |
| `El nombre '<n>' tiene <x> caracteres; el máximo es <m> para que ninguna ruta pase de 259. No se ha creado nada.` | The name is too long for the Windows path limit. Nothing was created. | Choose a name of at most `<m>` characters. |
| `La base '<base>' no existe. Para un ensayo, créala antes con la frase de crear base de ensayo. No se ha creado nada.` | The base does not exist; a base is never created implicitly. Nothing was created. | Check the spelling. For a rehearsal, first say `Crea la base de ensayo <base>`. |
| `Ya existe <qué> '<valor>'. No se ha creado nada.` | A branch, folder, registered copy or Engram project with that name already exists. Nothing was created. | Choose another name. If a failed creation left it, clean up with `Elimina el worktree <n>` from the main copy. An Engram project kept by an earlier removal keeps the name taken. |
| `Falló el paso '<paso>': <causa>. Ya creado: <lista>. Corrige la causa y limpia con la frase de eliminar el worktree '<n>'.` | A creation step failed; the message lists what was already created. | Fix the cause, say `Elimina el worktree <n>` in the main copy, then create the copy again. |
| `La copia tiene cambios sin confirmar: <lista>. Confírmalos o descártalos antes. No se ha cambiado nada.` | The copy has uncommitted changes. Nothing changed. | Commit or discard the listed changes, then ask for the merge again. |
| `La base '<base>' tiene <x> commit(s) que tu rama no contiene. Incorpóralos, vuelve a verificar y repite la fusión.` | Exit 3: the base moved since the branch last took it in. The base did not change. | Nothing: the agent goes back to merge step 1 once. If it comes up again, ask for the merge again later. |
| `La rama 'feat/<n>' no tiene nada que la base '<base>' no tenga. No se ha cambiado nada.` | The branch adds nothing to the base. Nothing changed. | Commit the work first; if the change is already in the base, remove the copy. |
| `La base '<base>' está abierta en '<carpeta>' con cambios sin confirmar. Confírmalos o guárdalos allí y repite la fusión.` | The base is checked out in another copy that has uncommitted changes. The base did not change. | In that copy, commit or stash the changes, then ask for the merge again. |
| `'<base>' avanza hasta 'feat/<n>' (<commit>). La copia y la rama se conservan.` | Exit 0: the base now points at the branch tip, without a merge commit; a copy that had the base checked out is updated. | Nothing. Remove the copy from the main copy when you no longer need it. |
| `No se elimina '<n>': la rama tiene <x> commit(s) que no están en '<base>'. Para descartarla y conservar su historia en exp/<n>, pide descartar el worktree. No se ha eliminado nada.` | The branch is not merged. Nothing was removed. | Merge it first (`Fusiona el cambio en la base` in the copy), or throw it away with `Descarta el worktree <n>`. |
| `No se elimina '<n>'. Se perdería: <lista>. No se ha eliminado nada.` | The copy has uncommitted changes, which would be lost (also when discarding). Nothing was removed. | Commit the listed changes in the copy, or discard them yourself, and ask again. |
| `Esta sesión está dentro de '<carpeta>'. Elimínala desde la copia principal.` | The session runs inside the copy it was asked to remove. | Close that session and ask from the main copy's session, at `C:\projects\MuninnDB`. |
| `No se pudo borrar '<archivo>': está en uso. Cierra el editor, la aplicación o las pruebas que lo usan y repite. No se ha cerrado ningún proceso.` | A file of the copy is in use; nothing was forced and no process was closed. | Close the editor window, the application or the tests that use it, and say the same phrase again. |
| `Ya existe la etiqueta exp/<n>. No se ha eliminado nada.` | An earlier discard of a change with this name left its tag. Nothing was removed. | The tools never overwrite it. Check it with `git log exp/<n>`; if you no longer need it, delete it yourself with `git tag -d exp/<n>` and ask again. |
| `'main' no es una base de ensayo. No se ha cambiado nada.` | `main` cannot be discarded as a rehearsal. Nothing changed. | Use the name of the rehearsal base. |
| `La base '<base>' ya existe. No se ha creado nada.` | A branch with that name already exists. Nothing was created. | Choose another name for the rehearsal base, or discard an earlier rehearsal you no longer need with `Descarta el ensayo <base>`. |
| `No hay worktrees (solo la copia principal).` | There are no copies, only the main copy. | Nothing. |
| `Bloqueado: esta sesión trabaja en '<carpeta>' y no escribe fuera de ella.` | The guard stopped the agent of a copy from writing in the main copy or another copy. | Keep the work inside the copy. Operations on copies are asked in the main copy's session. |
| `Bloqueado: la base '<base>' solo avanza con la fusión del worktree.` | The guard stopped the agent of a copy from moving the base by hand. | Use `Fusiona el cambio en la base`. |

### Other messages from the scripts

The PRD does not list these; the scripts print them in the same style.

| Message | Meaning | What to do |
|---|---|---|
| `No se encuentra '<herramienta>' en el PATH. No se ha creado nada.` (or `… No se ha eliminado nada.`) | A tool the script checks for (`engram`, or `code` when it opens the editor) is not on the PATH. | Install it or fix the PATH, then repeat. |
| `No se pudo consultar Engram: <causa>. No se ha creado nada.` (or `… No se ha eliminado nada.`) | The Engram query that checks or deletes the copy's project failed. | Fix Engram (`engram doctor`) and repeat. |
| `La copia no está en una rama feat/<nombre> (está en '<rama>'). No se ha cambiado nada.` | The merge runs only on the copy's `feat/<n>` branch. | Switch the copy back to its `feat/<n>` branch and repeat. |
| `La rama 'feat/<n>' no tiene base registrada. No se ha cambiado nada.` | The branch was not created by `New-Worktree.ps1`, so it has no base to merge into. | Merge it by hand from the main copy under the usual project rules. |
| `La base '<base>' no existe. No se ha cambiado nada.` | The copy's recorded base was deleted. | Check with `¿Qué worktrees tengo?`; recreate the base or discard the copy. |
| `Falló el paso '<paso>': <causa>. No se ha cambiado nada.` (or `… No se ha creado nada.`) | A step failed before anything was changed or created. | Fix the cause and repeat. |
| `Falló el paso '<paso>': <causa>. Ya eliminado: <lista>. Corrige la causa y repite.` | A removal stopped halfway; the list says what is already gone. | Fix the cause and say the same phrase again: removal can be repeated. |
| `No existe '<n>': ni copia, ni carpeta, ni rama. No se ha eliminado nada.` | There is nothing with that name. | Check the name with `¿Qué worktrees tengo?`. |
| `La carpeta '<carpeta>' no es una copia de este repositorio. No se ha eliminado nada.` | The folder exists but is not a copy of this repository, so it is never deleted. | Check the folder by hand. |
| `La base '<base>' está abierta en '<carpeta>'. Cambia allí a otra rama y repite. No se ha cambiado nada.` | The rehearsal base is checked out in a copy, so it cannot be deleted. | Switch that copy to another branch and repeat. |
| `No queda nada del ensayo '<base>'. No se ha cambiado nada.` | The rehearsal was already discarded. | Nothing. |
| `'<base>' no es una base de ensayo. No se ha cambiado nada.` | The branch was not created with `Crea la base de ensayo`, so the rehearsal discard never deletes it (the PRD message for `main`, applied to any unmarked branch). | Check the name; delete a branch made by hand with git yourself. |
| `Nombre '<n>' no válido: … No se ha eliminado nada.` | The name given to a removal or discard is not a valid change name. | Check the name with `¿Qué worktrees tengo?`. |
