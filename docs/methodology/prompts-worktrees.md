# Prompts para trabajar con worktrees

Frases exactas que el desarrollador escribe al agente para cada operación con copias de trabajo (worktrees),
en qué sesión se escriben y qué ocurre. Salen del [PRD de copias de trabajo en paralelo](../prd/worktrees/worktrees.prd.md);
si una frase de este documento y el PRD no coinciden, manda el PRD.

La skill `worktree` lleva cada frase a su script de `scripts/worktree/`. La [guía de uso](../worktrees.md)
(en inglés) explica el flujo completo, incluido el ensayo, y qué hacer ante cada mensaje.

## Antes de empezar

- **Copia principal**: `C:\projects\Krummi`. **Copia de un cambio**: `C:\wt\krummi-<n>`.
- **Nombre del cambio** `<n>`: minúsculas, dígitos y guiones simples (por ejemplo, `startswith-odd`).
- **Una sesión nueva del agente por copia**, abierta en la raíz de la copia. Una sesión abierta en una
  subcarpeta no carga el guardián.
- **Sesión equivocada**: si una frase se escribe donde no toca, el agente dice dónde hay que escribirla y se
  detiene sin cambiar nada.

## Operaciones

| # | Operación | Sesión | Prompt |
|---|---|---|---|
| 1 | Crear una copia sobre `main` | principal | `Crea un worktree para el cambio <n>` |
| 2 | Crear una copia sobre otra base | principal | `Crea un worktree para el cambio <n> sobre <base>` |
| 3 | Listar las copias | cualquiera | `¿Qué worktrees tengo?` |
| 4 | Fusionar una copia en su base | la copia | `Fusiona el cambio en la base` |
| 5 | Eliminar una copia ya fusionada | principal | `Elimina el worktree <n>` |
| 6 | Descartar una copia sin fusionar | principal | `Descarta el worktree <n>` |
| 7 | Crear la base de ensayo | principal | `Crea la base de ensayo <base>` |
| 8 | Descartar el ensayo completo | principal | `Descarta el ensayo <base>` |

### 1. Crear una copia sobre `main`

Sesión: copia principal.

```text
Crea un worktree para el cambio <n>
```

Crea la rama `feat/<n>` desde `main` y la carpeta `C:\wt\krummi-<n>`. Copia los permisos locales
y el registro de skills. También crea la carpeta de trabajo `work`, asigna el proyecto Engram `krummi-<n>` y abre una ventana nueva del editor en la raíz
de la copia. Si algo no es válido, no crea nada.

### 2. Crear una copia sobre otra base

Sesión: copia principal.

```text
Crea un worktree para el cambio <n> sobre <base>
```

Igual que la anterior, pero la rama parte de `<base>`, y la copia queda registrada con esa base para fusionar
después. La base tiene que existir: este prompt nunca la crea. `sobre main` equivale a la operación 1.

### 3. Listar las copias

Sesión: cualquiera.

```text
¿Qué worktrees tengo?
```

Muestra de cada copia su nombre, rama, base, si ya está fusionada en su base, su carpeta y su proyecto Engram.

### 4. Fusionar una copia en su base

Sesión: la copia del cambio, con el trabajo terminado y verificado.

```text
Fusiona el cambio en la base
```

El agente incorpora a la rama lo nuevo de la base, resuelve los conflictos contigo y vuelve a pasar las
comprobaciones completas. Después hace avanzar la base hasta la rama. La copia y la rama se conservan, y no
se publica nada en el remoto. Si la base tiene commits que la rama no contiene, el agente los incorpora y
repite una sola vez.

### 5. Eliminar una copia ya fusionada

Sesión: copia principal. Antes, cierra la ventana del editor de esa copia.

```text
Elimina el worktree <n>
```

Elimina la copia, su carpeta y su rama. Conserva su proyecto Engram y la memoria de Claude Code. Si la rama
no está fusionada o hay cambios sin confirmar, se niega sin borrar nada.

### 6. Descartar una copia sin fusionar

Sesión: copia principal. Antes, cierra la ventana del editor de esa copia.

```text
Descarta el worktree <n>
```

Crea la etiqueta `exp/<n>` en la punta de la rama, para que su historia y su informe de cierre sigan
accesibles. Después elimina la copia, la rama, el proyecto Engram y la memoria de Claude Code de esa carpeta.
Si hay cambios sin confirmar, se niega: confírmalos primero en la copia para que queden en la etiqueta.

### 7. Crear la base de ensayo

Sesión: copia principal.

```text
Crea la base de ensayo <base>
```

Crea la rama `<base>` desde `main`. Esa rama hace el papel de `main` durante el ensayo.

### 8. Descartar el ensayo completo

Sesión: copia principal. Antes, cierra las ventanas del editor de las copias del ensayo.

```text
Descarta el ensayo <base>
```

En un solo paso, elimina todas las copias cuya base es `<base>` junto con sus ramas, sus etiquetas `exp/`,
sus proyectos Engram y la memoria de Claude Code de sus carpetas. Al final elimina también la propia `<base>`.
`main` no cambia. Con `main` se niega.

## Flujo del experimento "startswith"

El ensayo y la demo usan exactamente los mismos prompts: solo cambia `<base>`, que es `ensayo` en el ensayo
y `main` en la demo. Los nombres de los cambios son siempre los mismos.

| Paso | Sesión | Prompt |
|---|---|---|
| 0 | principal | `Crea la base de ensayo ensayo` (solo en el ensayo) |
| 1 | principal | `Crea un worktree para el cambio startswith-prd sobre <base>` |
| 2 | `startswith-prd` (nueva) | el prompt que pide el PRD de startswith |
| 3 | `startswith-prd` | `Fusiona el cambio en la base` |
| 4 | principal | `Crea un worktree para el cambio startswith-vibe sobre <base>` |
| 5 | principal | `Crea un worktree para el cambio startswith-odd sobre <base>` |
| 6 | `startswith-vibe` (nueva) | el prompt que pide implementar el PRD con vibe coding |
| 6 | `startswith-odd` (nueva) | el prompt que pide implementar el PRD con ODD, a la vez que el anterior |
| 7 | `startswith-odd` | `Fusiona el cambio en la base` |
| 8 | principal | `Descarta el worktree startswith-vibe` |
| 9 | principal | `Elimina el worktree startswith-odd` |
| 10 | principal | `Elimina el worktree startswith-prd` |
| 11 | principal | `Descarta el ensayo ensayo` (solo en el ensayo) |

Después del paso 11 no queda nada del ensayo, y la demo empieza en el paso 1 con `<base>` = `main`. En la
demo, el paso 11 no existe: la historia de vibe coding queda en la etiqueta `exp/startswith-vibe` y la de
ODD, en `main`.

Los prompts de los pasos 2 y 6 son los habituales del proyecto (PRD con la skill `prd`, ejecución con vibe
coding o con ODD) y no forman parte de estas herramientas.
