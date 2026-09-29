# libid

Typed entity identifiers for the fleet — the shared implementation of
`standards/docs/public-ids.md` and the `Id<T>` template, extracted from the
per-repo copies that had drifted apart.

Two types, one boundary rule:

| | `Id<T>` | `PublicId<T>` |
|---|---|---|
| What | UUIDv7 tagged with its entity | 11-char Crockford Base32 alias (55 bits) |
| Where | internal only — `core`/`db`/`engine`, PKs, sorting, pagination | the *only* id in URLs, JSON, events |
| Storage | binary primary key (see [Storage](#storage)) | unique string beside the PK |
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

## Storage

The column types differ per backend — copy the row for yours:

| | SQLite (`sqlite`) | MySQL / MariaDB (`mysql`) | PostgreSQL (`postgres`) |
|---|---|---|---|
| `Id<T>` | `BLOB PRIMARY KEY` | `BINARY(16) PRIMARY KEY` | `UUID PRIMARY KEY` |
| `PublicId<T>` | `TEXT NOT NULL UNIQUE` | `VARCHAR(16) NOT NULL UNIQUE` | `TEXT NOT NULL UNIQUE` |

- **`PublicId<T>` is `VARCHAR`, not `TEXT`, on MySQL.** MySQL rejects a
  `UNIQUE` index on a `TEXT` column without a prefix length (error 1170), so the
  SQLite/Postgres schema fails there at migration time. (MariaDB accepts it
  through a hash-based long unique key; `VARCHAR` is portable and a plain B-tree
  index.) `16` fits the bare 11-char code and any `PREFIX` of up to four
  characters (`<prefix>_<code>`); widen it to `PREFIX.len() + 12` for longer
  prefixes. Postgres has no such limit, and `TEXT` is its idiomatic string type
  (`VARCHAR(n)` columns decode too).
- **`Id<T>` is never the 36-char text form.** SQLite and MySQL store the 16 raw
  bytes; Postgres uses its native `UUID`. An existing `CHAR(36)`/`TEXT` uuid
  column is not an `Id<T>` column: on MySQL it type-checks but fails to decode,
  on SQLite and Postgres it's a type mismatch. Migrate the column (e.g.
  `UNHEX(REPLACE(id, '-', ''))` on MySQL, `id::uuid` on Postgres) rather than
  binding a string.
- **Postgres arrays:** both types implement `PgHasArrayType` (`UUID[]` /
  `TEXT[]`), so a `Vec<Id<T>>` or `Vec<PublicId<T>>` binds for
  `WHERE id = ANY($1)`.
- **`sqlx::Any` is not supported.** sqlx 0.8's `Any` driver has no uuid type:
  an `Id<T>` could only travel as a blob, which matches SQLite `BLOB` and MySQL
  `BINARY(16)` but not a Postgres `UUID` column, so a portable impl would be
  wrong on one backend. Enable the concrete backend features instead.

## Features

- **`serde`** (default) — both types serialize as their canonical strings;
  deserialization goes through `parse`, so confusable spellings normalize on the
  way in.
- **`sqlite`**, **`mysql`**, **`postgres`** — sqlx `Type`/`Encode`/`Decode` for
  that backend, with the column types in [Storage](#storage). Each is
  independent and compiles only its own driver; enable any combination. Bind
  the typed values directly — no `.to_string()` at call sites.
- **`sqlx-all`** — all three backends.
- **`sqlx`** — *legacy* alias for `sqlite`, from before the other backends
  existed. Kept for compatibility; new code should say `sqlite`.
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

## Tests

```sh
cargo test --all-features
```

The MySQL and Postgres round-trips need a live server, so they're `#[ignore]`d
by default. Point them at scratch databases (they only create `TEMPORARY`
tables); the MySQL suite runs against MySQL or MariaDB:

```sh
LIBID_TEST_MYSQL_URL=mysql://user:pass@localhost/scratch \
LIBID_TEST_POSTGRES_URL=postgres://user:pass@localhost/scratch \
  cargo test --all-features -- --ignored
```

## Coverage

```sh
cargo tarpaulin --engine llvm
```

Config in [`tarpaulin.toml`](tarpaulin.toml) (all-features, 80% floor); CI runs
it on every push ([`.woodpecker/rust.ci.yml`](.woodpecker/rust.ci.yml)).
