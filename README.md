# libid

Typed entity identifiers for the fleet — the shared implementation of
`standards/docs/public-ids.md` and the `Id<T>` template, extracted from the
per-repo copies that had drifted apart.

Two types, one boundary rule:

| | `Id<T>` | `PublicId<T>` |
|---|---|---|
| What | UUIDv7 tagged with its entity | 11-char Crockford Base32 alias (55 bits) |
| Where | internal only — `core`/`db`/`engine`, PKs, sorting, pagination | the *only* id in URLs, JSON, events |
| Storage | `BLOB` primary key | `TEXT NOT NULL UNIQUE` beside the PK |
| Ordering | time-ordered — sort/paginate on this | random — never sort or paginate on it |

The exposer layer (the poem API) translates between them: inbound
`PublicId<T>` → one indexed lookup → `Id<T>`; outbound `Id<T>` → `PublicId<T>`.
A public id is a handle, not a secret — authz is enforced server-side regardless.

## Usage

```toml
[dependencies]
libid = { git = "https://github.com/charliethomson/libid" }
```

```rust
use libid::{Id, PublicEntity, PublicId};

struct Subscription;
impl PublicEntity for Subscription {}          // bare 11-char codes (the default)

struct Server;
impl PublicEntity for Server {
    const PREFIX: &'static str = "srv";        // canonical form: srv_<code>
}

let id = Id::<Subscription>::new();            // internal, UUIDv7
let public = PublicId::<Subscription>::new();  // mint at insert, retry on UNIQUE conflict

// Parsing normalizes case + Crockford confusables (o→0, i/l→1):
let same: PublicId<Subscription> = public.as_str().to_lowercase().parse().unwrap();
assert_eq!(same, public);
```

See [`libid/examples/basic.rs`](libid/examples/basic.rs) for the walkthrough.

## Features

- **`serde`** (default) — both types serialize as their canonical strings;
  deserialization goes through `parse`, so confusable spellings normalize on the
  way in.
- **`sqlx`** — SQLite bindings: `Id<T>` binds/decodes as `BLOB` (delegating to
  `Uuid`), `PublicId<T>` as canonical `TEXT`. Bind the typed values directly —
  no `.to_string()` at call sites.
- **`poem-openapi`** — `Type`/`ParseFromJSON`/`ParseFromParameter`/`ToJSON` for
  both, so they appear as opaque strings in the OpenAPI contract.

## What stays in the consuming repo

- **Entity markers + `PublicEntity` impls** — the entities are yours.
- **The mint-retry insert loop** — it touches your error classifier: retry the
  insert only on a `public_id` `UNIQUE` violation (matched by column name); every
  other `UNIQUE` is a real domain `Conflict`. See the loop in
  `standards/docs/public-ids.md`.
- **Resolve methods** — `Db::resolve_<entity>(&PublicId<E>) -> Result<Id<E>>`,
  mapping a miss to your standard not-found error.

## Coverage

```sh
cargo tarpaulin --engine llvm
```

Config in [`tarpaulin.toml`](tarpaulin.toml) (all-features, 80% floor); CI runs
it on every push ([`.woodpecker/rust.ci.yml`](.woodpecker/rust.ci.yml)).
