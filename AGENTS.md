# AGENTS

This repo follows the shared engineering standards vendored at [`standards/`](standards/)
(a git submodule). **Read [`standards/AGENTS.md`](standards/AGENTS.md) first** — it is the
entrypoint and indexes every rule.

- **Archetype:** library — see `standards/docs/archetypes/library.md`.
- **Identifier:** `dev.thmsn.id`
- Treat the standards as binding defaults. Repo-specific overrides (if any) are noted
  below; everything else defers to `standards/`.

## Repo-specific overrides

_None yet. Document any intentional deviation from the standards here, with a reason._

## Keeping standards current

```sh
git submodule update --remote standards
```
