//! Typed entity identifiers for the fleet.
//!
//! Two types, one boundary rule (`standards/docs/public-ids.md`):
//!
//! - [`Id<T>`] — the *internal* identifier: a `UUIDv7` tagged with its entity.
//!   Time-ordered, so it's the key for storage, sorting, and pagination. Never
//!   leaves internal surfaces.
//! - [`PublicId<T>`] — the *public* identifier: an 11-char Crockford Base32
//!   alias stored beside the internal id. The only identifier exposed in URLs,
//!   JSON, and events; resolved back to `Id<T>` at the HTTP boundary.
//!
//! Feature gates:
//! - `serde` (default) — both types serialize as their canonical strings.
//! - `sqlx` — `SQLite` bindings: `Id<T>` as BLOB (via `Uuid`), `PublicId<T>` as TEXT.
//! - `poem-openapi` — both types are opaque strings in the `OpenAPI` contract.

mod id;
mod public_id;

pub use id::Id;
pub use public_id::{ParseError as PublicIdParseError, PublicEntity, PublicId};

// A real round-trip through SQLite exercises the Type/Encode/Decode impls the
// way consumers use them (bind directly, decode via query_scalar).
#[cfg(all(test, feature = "sqlx"))]
mod sqlx_tests {
    use crate::{Id, PublicEntity, PublicId};

    struct Widget;
    impl PublicEntity for Widget {}

    struct Server;
    impl PublicEntity for Server {
        const PREFIX: &'static str = "srv";
    }

    #[tokio::test]
    async fn round_trips_through_sqlite() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE widget (id BLOB PRIMARY KEY, public_id TEXT NOT NULL UNIQUE)")
            .execute(&pool)
            .await
            .unwrap();

        let id = Id::<Widget>::new();
        let public = PublicId::<Widget>::new();
        sqlx::query("INSERT INTO widget (id, public_id) VALUES (?, ?)")
            .bind(id)
            .bind(public)
            .execute(&pool)
            .await
            .unwrap();

        // Decode straight into the typed forms.
        let (got_id, got_public): (Id<Widget>, PublicId<Widget>) =
            sqlx::query_as("SELECT id, public_id FROM widget")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(got_id, id);
        assert_eq!(got_public, public);

        // Point lookup by public id — the resolve path.
        let resolved: Id<Widget> = sqlx::query_scalar("SELECT id FROM widget WHERE public_id = ?")
            .bind(public)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(resolved, id);
    }

    #[tokio::test]
    async fn prefixed_form_is_what_hits_the_column() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE server (public_id TEXT NOT NULL UNIQUE)")
            .execute(&pool)
            .await
            .unwrap();

        let public = PublicId::<Server>::new();
        sqlx::query("INSERT INTO server (public_id) VALUES (?)")
            .bind(public)
            .execute(&pool)
            .await
            .unwrap();

        // Stored as the canonical prefixed TEXT…
        let raw: String = sqlx::query_scalar("SELECT public_id FROM server")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(raw.starts_with("srv_"), "stored form is canonical: {raw}");

        // …and decodes back to the typed form.
        let got: PublicId<Server> = sqlx::query_scalar("SELECT public_id FROM server")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(got, public);
    }
}
