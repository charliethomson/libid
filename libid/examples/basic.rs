//! The core surface: typed internal ids, public aliases, and the boundary
//! translation between them.
//!
//! ```sh
//! cargo run -p libid --example basic
//! ```

use libid::{Id, PublicEntity, PublicId};

/// An entity. Internal code keys everything on `Id<Subscription>`.
struct Subscription;
/// Addressable from outside → it declares itself a `PublicEntity`.
/// The default canonical form is the bare 11-char code.
impl PublicEntity for Subscription {}

/// A prefixed entity: its canonical public form is `srv_<code>`, so a pasted id
/// is self-describing in a URL or support ticket.
struct Server;
impl PublicEntity for Server {
    const PREFIX: &'static str = "srv";
}

fn main() {
    // Internal id: UUIDv7, time-ordered — mint at insert, use as the PK.
    let id = Id::<Subscription>::new();
    println!("internal : {id}");

    // Public id: minted at insert beside the internal id, stored in a
    // `TEXT NOT NULL UNIQUE` column, regenerated on conflict.
    let public = PublicId::<Subscription>::new();
    println!("public   : {public}");

    // The exposer layer resolves inbound codes, normalizing confusable
    // spellings (o→0, i/l→1, any case) to the canonical form.
    let sloppy = public.as_str().to_lowercase();
    let parsed: PublicId<Subscription> = sloppy.parse().expect("normalizes");
    assert_eq!(parsed, public);

    // The type tag means a Server's public id cannot resolve a Subscription —
    // and a prefixed entity's canonical form carries its tag.
    let server = PublicId::<Server>::new();
    println!("prefixed : {server}");
    assert!(server.to_string().starts_with("srv_"));
}
