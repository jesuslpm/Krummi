# <Title> — PRD

**Version**: 1.0
**Date**: YYYY-MM-DD
**Parent document**: <path to the direct parent PRD, or "none" for the root PRD>

## Objective and context

<What changes, for whom, and why now. Business language only.>

### Starting state

<What already exists today, verified in code and data. State gaps explicitly.>

## Actors

- **<Actor>**: <role and what they do in this change>

## Flow

1. <Step from the actor's point of view>
2. <Next step>

## Validations and messages

| Situation | Rule | Message to the user |
|---|---|---|
| <situation> | <rule> | <message> |

Cover every input the change accepts, including the existing values it does not name (for example the fields
an option does not apply to), not only the new ones.

## Output

<The exact public output for each case: keys, shape and wording, with a literal example. Include the edge cases
(empty lists, a condition with only the new part) and say whether existing output keeps its shape.>

## Reuse

<Existing behavior, rules, screens or data this change relies on instead of rebuilding.>

## Out of scope

- <Explicitly excluded item — where the next change starts>

## Left to the implementation

- <Observable point the user agreed not to fix, and why (for example, no performance target)>

## Acceptance criteria

1. **AC1** — Given <context>, when <action>, then <observable outcome>.

## References

- <Ancestry, from the direct parent up to the root; related PRDs; project docs>

## Elaboration time

<Do not write this section: `scripts/prd_time.py` fills it at the end of the session. Spanish heading:
"Tiempo de elaboración".>
