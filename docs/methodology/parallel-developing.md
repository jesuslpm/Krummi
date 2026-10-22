# Desarrollo en paralelo con worktrees, una base de datos por copia y un agente de IA

Guía general, aplicable a cualquier proyecto con base de datos, para trabajar **varios cambios a la vez** —cada uno
en su propia sesión de programación asistida por IA— sin que se pisen el código, la base de datos ni la
configuración. Describe el modelo, los scripts que hay que escribir, la skill que los expone al agente, la guarda
que evita accidentes y los **prompts** de cada operación.

> Implementación de referencia: LandPro, un proyecto privado (`dev-worktrees.md`, `tools/worktree/`,
> `.claude/skills/worktree/`, `.claude/hooks/block-main-db.mjs`). Esta guía destila de ahí lo que no depende del
> proyecto. Es el anexo de la sección 10 de [Desarrollo práctico de software asistido por
> IA](desarrollo-asistido-por-ia.md#10-desarrollo-en-paralelo-con-worktrees).

## 1. El problema

`git worktree` aísla **el código**: cada carpeta tiene su rama y sus ficheros. Pero casi todo lo demás sigue
compartido, y es ahí donde se rompe el trabajo en paralelo:

| Recurso | ¿Lo aísla `git worktree`? | Qué pasa si se comparte |
|---|---|---|
| Código y rama | Sí | — |
| **Base de datos** | **No** | La rama A publica su esquema y rompe la rama B; las pruebas de una borran los datos de la otra |
| Configuración local / secretos | No (suelen vivir fuera del repo) | Las dos copias apuntan a la misma base sin que nadie lo note |
| Dependencias (`node_modules`, paquetes restaurados) | No se copian | La copia nueva no compila hasta instalarlas |
| Puertos de las apps | No | La segunda app no arranca, o peor: el navegador habla con la de la otra copia |
| Almacenamiento externo (blobs, colas, correo) | No | Borrar un fichero en una copia lo borra en todas |
| Instrucciones privadas del agente (no versionadas) | No se copian | El agente de la copia nueva trabaja sin las reglas del desarrollador |
| Procesos abiertos (IDE, APIs, tests) | — | Bloquean ficheros y sesiones de base al eliminar la copia |

La regla de diseño es una sola: **cada copia del repositorio es un entorno completo y autónomo**, y todo lo que no se
pueda aislar se declara como riesgo aceptado (sección 9).

## 2. El modelo: un nombre, una copia, todo derivado

El desarrollador dice **un solo nombre** (el del cambio: `orders-export`) y de él se deriva todo, sin elecciones
adicionales:

| Elemento | Convención (ejemplo) | Por qué |
|---|---|---|
| Rama | `feature/orders-export` | La convención de ramas del equipo |
| Carpeta | `<raíz-wt>/<proyecto>-orders-export` (p. ej. `C:\wt\shop-orders-export`) | Fuera del repo principal (nunca dentro: confunde herramientas que suben buscando la raíz); el prefijo distingue carpetas y ventanas del IDE |
| Base de datos | `<Proyecto>_wt_orders_export` | Prefijo fijo reconocible: permite que los scripts destructivos se nieguen a tocar cualquier otra base |
| Configuración local | `appsettings.Local.json`, `.env.local`… (ignorada por git) | Una sola fuente que dice qué base usa esta copia |

- Validá el nombre **antes de crear nada**: caracteres permitidos (minúsculas, dígitos, guiones simples) y longitud
  máxima —el límite real lo pone el sistema más estricto (ruta de Windows, nombre de base, nombre de rama)—.
- La **copia principal** no cambia de comportamiento: sin fichero local, usa la base de siempre. Así el modelo se
  adopta sin obligar a nadie.

### Cómo sabe una copia que es un worktree

En un worktree, `.git` es un **fichero** (apunta al repositorio principal); en la copia principal es un
**directorio**. Subiendo desde la carpeta actual, el primer `.git` que se encuentra decide. Esta comprobación barata
la comparten los scripts, la aplicación y la guarda.

```bash
d="$PWD"; until [ -e "$d/.git" ]; do p="${d%/*}"; [ "$p" = "$d" ] && exit 0; d="$p"; done
[ -f "$d/.git" ] && echo "worktree en $d" || echo "copia principal en $d"
```

## 3. La regla de la base de datos

> **La base de una copia es la que dice su fichero de configuración local.** En un worktree ese fichero es
> **obligatorio**: si falta, o le falta la clave, toda operación contra la base **falla nombrando el fichero o la
> clave**. Nunca recurre en silencio a la base principal, a secretos de usuario ni a variables de entorno.

1. **El fichero local es la última fuente de configuración**: gana a los `appsettings*.json`, `.env`, secretos de
   usuario y variables de entorno. Si no fuera el último, una variable de entorno olvidada redirigiría la copia.
2. **Una sola implementación por plataforma** de esa resolución, reutilizada por: la aplicación al arrancar, las
   suites de pruebas, el generador de código del acceso a datos (si genera desde la base), los scripts de
   publicación y el resolvedor de línea de comandos.
3. **Nadie escribe el nombre de la base**: ni el desarrollador ni el agente. Para consultas sueltas existe un script
   que lo imprime:

   ```powershell
   sqlcmd -S "(local)" -E -d (powershell.exe -NoProfile -File tools/Get-LocalDatabase.ps1) -Q "SELECT 1"
   ```
   ```bash
   psql "$(./tools/get-local-database.sh)" -c "SELECT 1"
   ```
4. **Fallar en voz alta antes que adivinar.** Si no se puede saber a qué copia pertenece un binario o un proceso
   (por ejemplo, compilado en una carpeta que ya no existe), se rechaza con un mensaje que dice qué hacer
   («recompilá desde la copia que quieras usar»). Un error claro cuesta un minuto; una prueba que escribió en la
   base equivocada cuesta una tarde.
5. **Las pruebas de integración nombran sus recursos por base** (instantáneas, bloqueos, ficheros temporales), para
   que dos copias puedan correr sus suites a la vez.

## 4. Ciclo de vida de una copia

| Operación | Desde dónde | Qué hace |
|---|---|---|
| Crear | copia principal | rama + carpeta + base + configuración + dependencias + abrir el IDE |
| Listar | cualquiera | worktrees registrados, su rama, su carpeta y si su base existe |
| Trabajar | el worktree | el flujo normal del equipo, en una sesión del agente abierta **en la raíz** del worktree |
| Publicar esquema / regenerar | el worktree | contra **su** base; el agente puede hacerlo solo |
| Fusionar | el worktree | integración prudente (sección 4.3) |
| Eliminar | copia principal | solo si no se pierde nada; nunca mata procesos |

### 4.1 Crear

Pasos en este orden, cada uno con nombre impreso (`[worktree] Copying database...`):

1. **Validar todo antes de tocar nada**: nombre, que no existan ya rama/carpeta/base, que las herramientas estén en el
   PATH en la versión requerida (imprimir `[tools] <exe> <versión> <ruta>` de la elegida).
2. `git worktree add -b feature/<n> <carpeta> <rama-base>` — partiendo de `origin/<integración>` recién traído, no de
   la rama local.
3. **Crear la base**. Dos estrategias según el proyecto:
   - *Clonar la principal* (backup `COPY_ONLY` + restore, `pg_dump | pg_restore`, `CREATE DATABASE … TEMPLATE`…):
     datos realistas al instante. La base principal **no se modifica**.
   - *Crear vacía + migraciones + seed*: reproducible, sin datos de nadie; más lenta si el seed es grande.
4. **Escribir la configuración local** del worktree (sin comentarios si el formato no los admite): la cadena de
   conexión de la principal con el nombre de base cambiado, y lo que haya que apagar o compartir (telemetría remota
   apagada, claves de cifrado compartidas con la principal en lugar de duplicadas, etc.).
5. **Llevar la base al esquema de la rama**: aplicar migraciones o publicar el proyecto de esquema contra la base
   nueva, borrando además lo que la rama no define (solo permitido sobre bases con el prefijo de worktree).
6. **Copiar lo privado no versionado** que el agente necesita (instrucciones personales, comandos locales).
7. **Restaurar dependencias** con el comando reproducible (`npm ci`, `dotnet restore`, `pip install -r`…).
8. **Comprobar que `git status` está limpio**: si un paso dejó ficheros sin ignorar, es un defecto del script.
9. **Abrir una ventana nueva del IDE** en la carpeta.

Si un paso falla, el script **para**, dice qué paso falló y **qué quedó creado**, y la limpieza es el propio script de
eliminación. No intentes deshacer a mano dentro del script de creación: dos caminos de limpieza divergen.

### 4.2 Trabajar

- **Abrí el agente en la raíz del worktree.** Muchos agentes solo leen su configuración (hooks, permisos) de la
  carpeta donde arrancan: una sesión abierta en una subcarpeta no carga la guarda.
- Compilá **sin redirigir la salida fuera de la copia**: los binarios dentro del worktree se borran con él y siempre
  se sabe de qué copia son.
- Puertos: si dos copias tienen que ejecutar la app a la vez, el fichero local también fija el puerto (o se deriva
  del nombre). Si no, la regla es «una app arrancada a la vez».

### 4.3 Fusionar (integración prudente)

Siempre en dos direcciones y en este orden: **primero la integración entra en la rama; solo con la rama resuelta y
probada, la rama entra en la integración.**

1. `git fetch origin`, mostrar `git log --oneline HEAD..origin/<integración>` y `git merge --no-edit origin/<integración>`.
2. Conflictos:
   - **Ficheros generados** (acceso a datos, modelos del cliente, lockfiles): nunca se editan a mano. Se toma
     cualquier lado y se **regeneran**: publicar esquema → regenerar → compilar → regenerar modelos → commit.
   - **Ficheros escritos a mano**: se resuelven **con el desarrollador**, nunca solo el agente.
3. **Todas las suites completas** contra la base del worktree. Si alguna falla, la fusión se detiene.
4. Llevar el resultado a `origin/<integración>` sin necesitar que la rama de integración esté libre en otra copia
   (por ejemplo, construyendo el commit con `git commit-tree` y empujándolo; o con un PR si el equipo trabaja así).
   Si la copia principal tiene la integración abierta, no se mueve por debajo: se avisa de que haga `git pull`.
5. Códigos de salida con significado, porque el agente actúa según ellos:

| Código | Significado | Qué hace el agente |
|---|---|---|
| `0` | Hecho | Informa |
| `3` | `origin/<integración>` avanzó mientras tanto | Vuelve al paso 1 **una vez**; si se repite, para y avisa |
| `1` | Cualquier otro error (precondición, hook, protección de rama, credenciales, red) | Para y transmite el mensaje literal; **nunca reintenta** |

### 4.4 Eliminar

Desde la copia principal. **Primero comprobar, después tocar**:

1. ¿Hay ficheros sin commit en el worktree? ¿Commits de la rama que no están ni en la integración local ni en la
   remota? Si sí: **rechazar** listando qué se perdería. Nunca hay `--force`.
2. ¿La sesión actual corre dentro de ese worktree? Rechazar.
3. Quitar el worktree (`git worktree remove`), la rama, la base y la carpeta residual.
4. Si un fichero está en uso o la base tiene sesiones abiertas: **informar y parar**. Ni el script ni el agente matan
   procesos ni sesiones: pueden ser el IDE o la app del desarrollador con trabajo sin guardar.

## 5. Los scripts

La lógica vive en **scripts versionados**, no en el prompt del agente: un script se prueba, se revisa y se ejecuta
igual con o sin IA.

| Script | Responsabilidad |
|---|---|
| `tools/worktree/New-Worktree` | Crear (4.1) |
| `tools/worktree/Get-Worktrees` | Listar, con el estado de cada base |
| `tools/worktree/Merge-Worktree` | Paso 4 de la fusión (4.3); los pasos 1-3 los orquesta la skill |
| `tools/worktree/Remove-Worktree` | Eliminar (4.4) |
| `tools/Get-LocalDatabase` | Imprimir la base de **esta** copia |
| `tools/Publish-Local` | Publicar esquema / aplicar migraciones contra la base de esta copia, sin reescribir ficheros versionados |
| `tools/DevCopy` (módulo común) | Detección de worktree, lectura del fichero local, búsqueda de herramientas: **una sola vez** |

Principios que hacen que el agente pueda usarlos sin supervisión constante:

- **Idempotentes donde se pueda, y si no, que se nieguen**: un segundo `New` con el mismo nombre dice «ya existe», no
  crea a medias.
- **Mensajes accionables**: cada error empieza por una frase fija y dice la acción («`X` is not on PATH. Add `<carpeta>`
  to PATH and restart the IDE»). Documentá cada mensaje en una tabla *mensaje → qué hacer*.
- **Sin interacción**: ningún prompt de confirmación; el agente no puede responderlos. La seguridad viene de
  rechazar, no de preguntar.
- **Herramientas por el PATH, nunca por rutas de instalación adivinadas**, y exigiendo **una sola versión** cuando
  hay varias instaladas.
- **Las operaciones destructivas comprueban el prefijo** (`<Proyecto>_wt_*`): un error de variable nunca puede
  borrar la base principal.
- **No reescriben ficheros versionados** como efecto secundario (exportaciones, perfiles): un `git status` sucio tras
  publicar es un defecto.
- **Los perfiles de publicación del IDE que apuntan a la base principal se rechazan en un worktree** (un target de
  build o un chequeo en el script), para que el botón de siempre no publique en la base equivocada.

## 6. La skill (cómo se lo expone al agente)

La skill es una **tabla de enrutado frase → script** con las reglas duras. No contiene lógica: si hace falta un `if`
complejo, va al script.

```markdown
---
name: worktree
description: "Trigger: worktree, create a worktree, merge the change, which worktrees do I have, remove the worktree. Create, list, merge and remove per-change worktrees."
---

## Activation Contract
Use when the developer asks to create, list, merge or remove a worktree. A worktree owns branch `feature/<n>`,
folder `<wt-root>/<project>-<n>`, database `<Project>_wt_<n with - as _>` and the local config file.

## Hard Rules
- Everything goes through `tools/worktree/*` scripts; never improvise git or database commands for these operations.
- Never use the agent's built-in worktree feature.
- Create and remove only from the main copy; merge only from the worktree.
- Never write a database name: resolve it with `tools/Get-LocalDatabase`.
- Never hand-resolve a generated file: regenerate it.
- Never kill a process or a database session; report what blocks and stop.
- Refused removal: show what would be lost; never force it.
- Merge exit 3: back to step 1 ONCE. Any other non-zero exit: stop and relay the message verbatim.

## Decision Gates
| The developer says | Session | Run |
|---|---|---|
| "Create a worktree for `<n>`" | main | `New-Worktree -Name <n>` |
| "Which worktrees do I have?" | any | `Get-Worktrees` |
| "Merge the change" | worktree | Merge steps 1-5 |
| "Remove the worktree `<n>`" | main | `Remove-Worktree -Name <n>` |

Wrong session: say where to run it and stop.

## Execution Steps
(Create / Merge 1-5 / Remove, as in section 4 of the guide.)

## Output Contract
In the developer's language: what was created, merged or removed, or why not and the next action.

## References
- docs/general/<project>-worktrees.md — rules, guard, every message and what to do.
```

Claves para que funcione:

- **El `description` lleva las frases reales** que dice el equipo, en su idioma: es lo que el agente compara para
  activarla.
- **«Wrong session: say where and stop»**: la mayoría de errores son ejecutar en la copia equivocada.
- **Reintentos acotados y explícitos** (exit 3 → una vez). Sin esa regla un agente reintenta hasta el infinito o no
  reintenta nunca.
- Registrá la skill donde el agente la descubra (por ejemplo, un índice de skills del proyecto) y referenciala
  desde las instrucciones del proyecto (`CLAUDE.md`, `AGENTS.md`).

## 7. La guarda (hook antes de cada orden)

Una costumbre de meses («`-d MiBase`») sobrevive a cualquier documento. La guarda es un hook `PreToolUse` que, **solo
dentro de un worktree**, bloquea toda orden de shell que nombre la base principal en una posición de base de datos.

- **Qué bloquea**: `-d <Principal>`, `Database=`/`Initial Catalog=`, `USE <Principal>`, nombres de tres partes
  (`<Principal>.dbo.tabla`), destinos de herramientas de publicación, `ALTER/DROP DATABASE`.
- **Qué deja pasar**: el nombre como parte de rutas, proyectos o namespaces (`Shop.Api/…`), las bases con prefijo de
  worktree, y las **búsquedas** (`rg`, `grep`, `git grep`), donde el nombre es un patrón.
- **En la copia principal no arranca ni el intérprete**: la comprobación del `.git` se hace en el propio comando del
  hook, para que un compañero sin `node` no vea errores.
- **El mensaje de bloqueo dice la alternativa exacta** (el comando con `Get-LocalDatabase`): el agente se corrige
  solo.
- **Es una red contra la costumbre, no contra un adversario**: no inspecciona el contenido de los scripts invocados.
  Documentá los falsos positivos conocidos.
- **Tiene sus pruebas** (`node --test .claude/hooks/*.test.mjs`) y **se prueba en vivo con comandos inofensivos**
  (`echo "-d <Principal>"`), nunca con el comando real.

Registro en Claude Code (`.claude/settings.json`, versionado):

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash|PowerShell",
        "hooks": [
          {
            "type": "command",
            "command": "d=\"${CLAUDE_PROJECT_DIR:-$PWD}\"; until [ -e \"$d/.git\" ]; do p=\"${d%/*}\"; [ \"$p\" = \"$d\" ] && exit 0; d=\"$p\"; done; [ -f \"$d/.git\" ] || exit 0; node \"$d/.claude/hooks/block-main-db.mjs\"",
            "timeout": 10
          }
        ]
      }
    ]
  }
}
```

El script lee el JSON de la herramienta por `stdin`, recorre **todas** sus cadenas (los nombres de campo cambian entre
herramientas) y sale con código `2` y el mensaje por `stderr` para bloquear. Ante una entrada que no entiende, **no
bloquea**.

## 8. Instrucciones del proyecto para el agente

Añadí a `CLAUDE.md` / `AGENTS.md` lo mínimo, apuntando a la guía del proyecto:

```markdown
### Database
Each copy of the repository has its own database: resolve it with `tools/Get-LocalDatabase`, never by writing a
name. In a git worktree the agent publishes the schema and regenerates the data layer itself; in the main copy the
database is the developer's — ask them, and run those steps only when asked.
Worktrees: create, list, merge and remove only through the `worktree` skill. See docs/general/<project>-worktrees.md.
```

La asimetría es deliberada: **en un worktree el agente es dueño de la base** (la creó un script y se tira con la
copia); **en la principal, el dueño es el desarrollador**.

## 9. Riesgos aceptados (lo que no se aísla)

Escribilos explícitamente, con la regla de uso que los neutraliza:

| Recurso compartido | Regla |
|---|---|
| Almacenamiento de ficheros en la nube | En un worktree no borres ni reemplaces ficheros existentes: creá nuevos para probar |
| Colas, correo, webhooks externos | Apagados o redirigidos a un buzón/sandbox en el fichero local |
| Claves de cifrado de la app | Compartidas con la principal (no duplicadas): lo cifrado en una copia se lee en la otra |
| Objetos huérfanos en la base | Si la herramienta de publicación no borra lo que el proyecto ya no define, un script de informe los lista |

## 10. Prompts de cada operación

Frases para el agente, listas para copiar. `<n>` es el nombre del cambio.

### Desde la copia principal

| Operación | Prompt |
|---|---|
| Crear | «Creá un worktree para el cambio `<n>`.» |
| Crear con un servicio extra | «Creá un worktree para el cambio `<n>` con `<servicio>`.» (si el script tiene un interruptor `-With<Servicio>`) |
| Listar | «¿Qué worktrees tengo?» |
| Eliminar | «Eliminá el worktree `<n>`.» |
| Ponerse al día tras una fusión | «Hacé `git pull` de la rama de integración y decime si trae cambios de esquema.» (publicar la base principal lo decide el desarrollador) |

### Desde el worktree (sesión abierta en su raíz)

| Operación | Prompt |
|---|---|
| Comprobar el entorno | «Verificá este worktree: qué base resuelve, si existe, si el esquema está al día con la rama y si `git status` está limpio.» |
| Empezar el trabajo | «Empecemos el cambio `<n>`: <descripción>.» (el flujo del equipo: PRD, spec, TDD…) |
| Consultar la base | «Consultá en la base de este worktree <qué>.» |
| Publicar el esquema | «Publicá el esquema en la base de este worktree.» |
| Regenerar el acceso a datos | «Regenerá el acceso a datos y los modelos del cliente desde la base de este worktree.» |
| Reconstruir la base | «Reconstruí la base de este worktree desde el esquema de la rama.» |
| Traer la integración sin fusionar | «Traé `origin/<integración>` a esta rama, resolvé los generados regenerándolos y consultame los conflictos escritos a mano.» |
| Correr todas las suites | «Corré todas las suites completas contra la base de este worktree y listame los fallos.» |
| Fusionar | «Fusioná el cambio a `<integración>`.» |
| Diagnosticar un mensaje | «El script dijo: `<mensaje>`. ¿Qué hago?» (el agente busca en la tabla *mensaje → qué hacer*) |

### Para montar todo esto en un proyecto nuevo

Un prompt de arranque, para una sesión en la copia principal. Conviene tratarlo como un cambio grande: con su
documento de producto y sus pruebas, no de un tirón.

```text
Quiero desarrollar en paralelo con git worktrees en este repositorio, siguiendo
docs/general/parallel-developing.md. Antes de proponer nada:
1. Buscá cómo resuelve hoy la aplicación su base de datos (app, tests, generadores,
   scripts de publicación) y listame cada punto con path:línea.
2. Decime qué recursos compartidos tiene el proyecto (almacenamiento, colas, correo,
   puertos, claves) y cuáles propondrías aislar y cuáles aceptar como riesgo.
3. Proponé la convención de nombres (rama, carpeta, base) y la estrategia de creación
   de la base (clonar la principal o crear + migrar + seed), con sus contras.
Después, por pasos y con pruebas: la resolución única de la base desde un fichero
local ignorado por git; Get-LocalDatabase y Publish-Local; los scripts New/Get/Merge/
Remove; la skill `worktree`; la guarda PreToolUse con sus tests; la guía del proyecto
con la tabla mensaje → qué hacer; y la referencia en las instrucciones del agente.
No toques la base principal en ningún momento.
```

## 11. Comprobación

- [ ] En la copia principal, sin fichero local, la app y las pruebas usan la base de siempre.
- [ ] En un worktree recién creado: `Get-LocalDatabase` devuelve `<Proyecto>_wt_<n>`, la base existe y `git status`
      está limpio.
- [ ] Borrar el fichero local del worktree hace fallar la app, las pruebas y los scripts **nombrando el fichero**.
- [ ] `echo "-d <Principal>"` en una sesión del worktree queda bloqueado; en la principal, no.
- [ ] Dos worktrees corren sus suites a la vez sin interferir.
- [ ] Eliminar un worktree con un fichero sin commit se rechaza listándolo, sin tocar nada.
- [ ] Una fusión con `origin/<integración>` adelantada vuelve al paso 1 una sola vez.
