//! Public short ids — opaque, URL-facing aliases for entities.
//!
//! `PublicId<T>` is an 11-character Crockford Base32 code (55 bits) tagged with the
//! entity it names, minted at creation and stored beside the internal [`crate::Id`]
//! (`UUIDv7`) as a `TEXT NOT NULL UNIQUE` column. It is the *only* identifier that
//! crosses the public boundary — URLs, query params, JSON, event envelopes — and is
//! translated to/from `Id<T>` in the exposer layer (the poem API). The UUID never
//! leaves internal/admin surfaces. See `standards/docs/public-ids.md`.
//!
//! Entities implement [`PublicEntity`]; the default is the bare 11-char code. An
//! entity may override [`PublicEntity::PREFIX`] to make its canonical form
//! self-describing (`srv_9TXK4P2RQ8M`) — the prefix is then part of what is stored,
//! displayed, and parsed; the 11-char code still carries all the entropy.
//!
//! This is a handle, not a secret: authorization is still enforced server-side, so
//! knowing a code grants nothing. Runtime representation is 11 ASCII bytes (`Copy`).
//!
//! `Ord`/`PartialOrd` exist for deterministic collections only — the code is
//! random, so never sort, range-scan, or paginate on it; key those on `Id<T>`.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::str::FromStr;

/// Crockford Base32 alphabet: digits + A–Z minus the confusable `I L O U`.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// 11 symbols × 5 bits = 55 bits of entropy.
const LEN: usize = 11;

/// Marker for entities addressable by public id. The default canonical form is
/// the bare 11-char code; override [`Self::PREFIX`] with a short lowercase tag
/// (e.g. `"srv"`) to canonicalize as `srv_<code>` instead.
pub trait PublicEntity {
    /// Lowercase ASCII tag, without the `_` separator. Empty = no prefix.
    const PREFIX: &'static str = "";
}

pub struct PublicId<T> {
    code: [u8; LEN], // ASCII, canonical uppercase Crockford
    _marker: PhantomData<fn() -> T>,
}

impl<T> PublicId<T> {
    /// Mint a fresh code from 55 random bits. The caller inserts it against a
    /// `UNIQUE` index and regenerates on conflict (a collision is a transparent
    /// retry, not an error).
    #[must_use]
    pub fn new() -> Self {
        Self::from_bits(rand::random::<u64>() & ((1u64 << (5 * LEN)) - 1))
    }

    /// The bare 11-character code, without any entity prefix. The canonical
    /// (possibly prefixed) form is `Display`.
    #[must_use]
    // `code` only ever holds ASCII bytes drawn from `ALPHABET`, so the `expect`
    // is an invariant assertion that cannot fire — no `# Panics` section warranted.
    #[allow(clippy::missing_panics_doc)]
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.code).expect("code is ASCII")
    }

    fn from_bits(mut bits: u64) -> Self {
        let mut code = [0u8; LEN];
        // Fill least-significant symbol last, so the code reads most-significant first.
        for slot in code.iter_mut().rev() {
            // `bits & 0x1f` is at most 31, so the cast is lossless.
            #[allow(clippy::cast_possible_truncation)]
            let sym = (bits & 0x1f) as usize;
            *slot = ALPHABET[sym];
            bits >>= 5;
        }
        Self { code, _marker: PhantomData }
    }
}

/// Map one input byte to its 5-bit value, folding Crockford's confusables
/// (`I/L → 1`, `O → 0`) and accepting either case. `U` is not in the alphabet
/// and is rejected.
fn decode_symbol(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'O' | b'o' => Some(0),
        b'I' | b'i' | b'L' | b'l' => Some(1),
        _ => {
            let up = c.to_ascii_uppercase();
            // The alphabet has 32 symbols, so the position always fits a u8.
            #[allow(clippy::cast_possible_truncation)]
            ALPHABET.iter().position(|&a| a == up).map(|p| p as u8)
        }
    }
}

/// Why a string failed to parse as a [`PublicId`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Missing or wrong `<prefix>_` tag for the entity being parsed.
    Prefix { expected: &'static str },
    /// The code was not exactly [`LEN`] characters.
    Length(usize),
    /// The code contained a character outside the Crockford alphabet.
    Char(char),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prefix { expected } => {
                write!(f, "public id must start with \"{expected}_\"")
            }
            Self::Length(n) => write!(f, "public id must be {LEN} chars, got {n}"),
            Self::Char(c) => write!(f, "invalid public-id character {c:?}"),
        }
    }
}
impl std::error::Error for ParseError {}

impl<T: PublicEntity> FromStr for PublicId<T> {
    type Err = ParseError;
    /// Strips the entity prefix (when the entity declares one), then normalizes
    /// the code (uppercase + confusable folding) and validates, so any spelling
    /// of a code resolves to the same canonical value.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let code = if T::PREFIX.is_empty() {
            s
        } else {
            s.strip_prefix(T::PREFIX)
                .and_then(|rest| rest.strip_prefix('_'))
                .ok_or(ParseError::Prefix { expected: T::PREFIX })?
        };
        let bytes = code.as_bytes();
        if bytes.len() != LEN {
            return Err(ParseError::Length(bytes.len()));
        }
        let mut bits: u64 = 0;
        for &c in bytes {
            let v = decode_symbol(c).ok_or(ParseError::Char(c as char))?;
            bits = (bits << 5) | u64::from(v);
        }
        Ok(Self::from_bits(bits))
    }
}

impl<T> Default for PublicId<T> {
    fn default() -> Self {
        Self::new()
    }
}

// Manual impls so the PhantomData type parameter doesn't force `T: Trait` bounds.
impl<T> Clone for PublicId<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for PublicId<T> {}
impl<T> PartialEq for PublicId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code
    }
}
impl<T> Eq for PublicId<T> {}
impl<T> Hash for PublicId<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.code.hash(state);
    }
}
impl<T> PartialOrd for PublicId<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for PublicId<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.code.cmp(&other.code)
    }
}

impl<T: PublicEntity> fmt::Display for PublicId<T> {
    /// The canonical form: the bare code, or `<prefix>_<code>` when the entity
    /// declares a prefix. This is what's stored, exposed, and parsed.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if T::PREFIX.is_empty() {
            f.write_str(self.as_str())
        } else {
            write!(f, "{}_{}", T::PREFIX, self.as_str())
        }
    }
}
impl<T> fmt::Debug for PublicId<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicId({})", self.as_str())
    }
}

// ── serde: the canonical string; deserializes through `parse` so confusable
// spellings are normalized on the way in ─────────────────────────────────────
#[cfg(feature = "serde")]
impl<T: PublicEntity> serde::Serialize for PublicId<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
#[cfg(feature = "serde")]
impl<'de, T: PublicEntity> serde::Deserialize<'de> for PublicId<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

// ── sqlx (SQLite): stored as the canonical TEXT form ─────────────────────────
#[cfg(feature = "sqlite")]
impl<T> sqlx::Type<sqlx::Sqlite> for PublicId<T> {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <String as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
    fn compatible(ty: &sqlx::sqlite::SqliteTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Sqlite>>::compatible(ty)
    }
}
#[cfg(feature = "sqlite")]
impl<'q, T: PublicEntity> sqlx::Encode<'q, sqlx::Sqlite> for PublicId<T> {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <String as sqlx::Encode<'q, sqlx::Sqlite>>::encode(self.to_string(), buf)
    }
}
#[cfg(feature = "sqlite")]
impl<'r, T: PublicEntity> sqlx::Decode<'r, sqlx::Sqlite> for PublicId<T> {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <String as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
        s.parse().map_err(|e: ParseError| Box::new(e) as sqlx::error::BoxDynError)
    }
}

// ── sqlx (MySQL/MariaDB): stored as the canonical VARCHAR form ───────────────
// VARCHAR, not TEXT: MySQL/MariaDB can't put a UNIQUE index on a TEXT column
// without a prefix length. `VARCHAR(16)` holds the bare 11-char code and any
// prefix of up to four characters (`<prefix>_<code>`).
#[cfg(feature = "mysql")]
impl<T> sqlx::Type<sqlx::MySql> for PublicId<T> {
    fn type_info() -> sqlx::mysql::MySqlTypeInfo {
        <String as sqlx::Type<sqlx::MySql>>::type_info()
    }
    fn compatible(ty: &sqlx::mysql::MySqlTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::MySql>>::compatible(ty)
    }
}
#[cfg(feature = "mysql")]
impl<T: PublicEntity> sqlx::Encode<'_, sqlx::MySql> for PublicId<T> {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<u8>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <String as sqlx::Encode<'_, sqlx::MySql>>::encode(self.to_string(), buf)
    }
}
#[cfg(feature = "mysql")]
impl<'r, T: PublicEntity> sqlx::Decode<'r, sqlx::MySql> for PublicId<T> {
    fn decode(value: sqlx::mysql::MySqlValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::MySql>>::decode(value)?;
        s.parse().map_err(|e: ParseError| Box::new(e) as sqlx::error::BoxDynError)
    }
}

// ── sqlx (PostgreSQL): stored as the canonical TEXT form ────────────────────
// TEXT is idiomatic here (same storage as VARCHAR, and UNIQUE on TEXT is fine);
// decoding also accepts VARCHAR/CHAR columns.
#[cfg(feature = "postgres")]
impl<T> sqlx::Type<sqlx::Postgres> for PublicId<T> {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }
    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}
// `TEXT[]`, so a `Vec<PublicId<T>>` binds for `WHERE public_id = ANY($1)`.
#[cfg(feature = "postgres")]
impl<T> sqlx::postgres::PgHasArrayType for PublicId<T> {
    fn array_type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::postgres::PgHasArrayType>::array_type_info()
    }
}
#[cfg(feature = "postgres")]
impl<T: PublicEntity> sqlx::Encode<'_, sqlx::Postgres> for PublicId<T> {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <String as sqlx::Encode<'_, sqlx::Postgres>>::encode(self.to_string(), buf)
    }
}
#[cfg(feature = "postgres")]
impl<'r, T: PublicEntity> sqlx::Decode<'r, sqlx::Postgres> for PublicId<T> {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        s.parse().map_err(|e: ParseError| Box::new(e) as sqlx::error::BoxDynError)
    }
}

// ── poem-openapi: an opaque string in the contract ───────────────────────────
#[cfg(feature = "poem-openapi")]
mod poem_impls {
    use std::borrow::Cow;

    use poem_openapi::registry::{MetaSchema, MetaSchemaRef};
    use poem_openapi::types::{
        ParseError as OaiParseError, ParseFromJSON, ParseFromParameter, ParseResult, ToJSON, Type,
    };
    use serde_json::Value;

    use super::{PublicEntity, PublicId};

    impl<T: PublicEntity + Send + Sync> Type for PublicId<T> {
        const IS_REQUIRED: bool = true;
        type RawValueType = Self;
        type RawElementValueType = Self;

        fn name() -> Cow<'static, str> {
            "public_id".into()
        }
        fn schema_ref() -> MetaSchemaRef {
            MetaSchemaRef::Inline(Box::new(MetaSchema::new("string")))
        }
        fn as_raw_value(&self) -> Option<&Self::RawValueType> {
            Some(self)
        }
        fn raw_element_iter<'a>(
            &'a self,
        ) -> Box<dyn Iterator<Item = &'a Self::RawElementValueType> + 'a> {
            Box::new(self.as_raw_value().into_iter())
        }
    }
    impl<T: PublicEntity + Send + Sync> ParseFromJSON for PublicId<T> {
        fn parse_from_json(value: Option<Value>) -> ParseResult<Self> {
            match value.unwrap_or_default() {
                Value::String(s) => s.parse().map_err(OaiParseError::custom),
                other => Err(OaiParseError::expected_type(other)),
            }
        }
    }
    impl<T: PublicEntity + Send + Sync> ParseFromParameter for PublicId<T> {
        fn parse_from_parameter(value: &str) -> ParseResult<Self> {
            value.parse().map_err(OaiParseError::custom)
        }
    }
    impl<T: PublicEntity + Send + Sync> ToJSON for PublicId<T> {
        fn to_json(&self) -> Option<Value> {
            Some(Value::String(self.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ParseError, PublicEntity, PublicId, LEN};

    struct Foo;
    impl PublicEntity for Foo {}

    // Only referenced by the serde-gated test below.
    #[cfg(feature = "serde")]
    struct Bar;
    #[cfg(feature = "serde")]
    impl PublicEntity for Bar {}

    /// An entity that opts into a prefixed canonical form.
    struct Widget;
    impl PublicEntity for Widget {
        const PREFIX: &'static str = "wgt";
    }

    #[test]
    fn new_is_eleven_alphabet_chars() {
        let p = PublicId::<Foo>::new();
        assert_eq!(p.as_str().len(), LEN);
        assert!(
            p.as_str().bytes().all(|b| b"0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(&b)),
            "minted code {} must be canonical Crockford",
            p.as_str()
        );
    }

    #[test]
    fn round_trips_through_string() {
        let a = PublicId::<Foo>::new();
        let s = a.to_string();
        assert_eq!(s, a.as_str(), "no prefix by default");
        let b: PublicId<Foo> = s.parse().unwrap();
        assert_eq!(a, b);
        assert_eq!(a.as_str(), b.as_str());
    }

    #[test]
    fn normalizes_confusables_and_case() {
        // Lowercase, plus the confusables I/L → 1 and O → 0, must fold to the
        // canonical spelling.
        let canonical: PublicId<Foo> = "1234567890A".parse().unwrap();
        let spelled: PublicId<Foo> = "i234567890a".parse().unwrap();
        assert_eq!(canonical, spelled);
        // O folds to 0, L folds to 1.
        let a: PublicId<Foo> = "0AAAAAAAAAA".parse().unwrap();
        let b: PublicId<Foo> = "OaaaaaaaaaA".parse().unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn canonical_string_has_no_confusables() {
        // Whatever we mint, its canonical form round-trips to itself with no
        // further folding.
        let p = PublicId::<Foo>::new();
        let again: PublicId<Foo> = p.as_str().parse().unwrap();
        assert_eq!(p.as_str(), again.as_str());
    }

    #[test]
    fn rejects_wrong_length() {
        assert_eq!("SHORT".parse::<PublicId<Foo>>(), Err(ParseError::Length(5)));
        assert_eq!(
            "WAYTOOLONGXX".parse::<PublicId<Foo>>(),
            Err(ParseError::Length(12))
        );
    }

    #[test]
    fn rejects_out_of_alphabet_char() {
        // `U` is deliberately excluded from Crockford; it must be rejected even
        // though the length is right.
        assert_eq!(
            "UUUUUUUUUUU".parse::<PublicId<Foo>>(),
            Err(ParseError::Char('U'))
        );
    }

    #[test]
    fn prefixed_entity_displays_and_round_trips() {
        let id = PublicId::<Widget>::new();
        let s = id.to_string();
        assert!(s.starts_with("wgt_"), "canonical form is prefixed: {s}");
        assert_eq!(s.len(), 4 + LEN);
        let parsed: PublicId<Widget> = s.parse().expect("canonical form parses");
        assert_eq!(id, parsed);
    }

    #[test]
    fn prefixed_entity_normalizes_code() {
        let canonical: PublicId<Widget> = "wgt_0123456789A".parse().expect("parses");
        // o→0, i/l→1, lowercase folds up.
        let sloppy: PublicId<Widget> = "wgt_ol23456789a".parse().expect("parses");
        assert_eq!(canonical, sloppy);
    }

    #[test]
    fn prefixed_entity_rejects_wrong_or_missing_prefix() {
        assert_eq!(
            "srv_0123456789A".parse::<PublicId<Widget>>(),
            Err(ParseError::Prefix { expected: "wgt" })
        );
        assert_eq!(
            "0123456789A".parse::<PublicId<Widget>>(),
            Err(ParseError::Prefix { expected: "wgt" })
        );
        assert_eq!(
            "wgt_0123".parse::<PublicId<Widget>>(),
            Err(ParseError::Length(4))
        );
    }

    #[test]
    fn parse_error_renders() {
        assert!(ParseError::Length(3).to_string().contains("11 chars"));
        assert!(ParseError::Char('U').to_string().contains("'U'"));
        assert!(
            ParseError::Prefix { expected: "wgt" }
                .to_string()
                .contains("wgt_")
        );
    }

    #[test]
    fn ordering_and_hashing_are_consistent() {
        use std::collections::HashSet;
        let a: PublicId<Foo> = "00000000000".parse().unwrap();
        let b: PublicId<Foo> = "00000000001".parse().unwrap();
        assert!(a < b);
        assert_eq!(a.partial_cmp(&b), Some(a.cmp(&b)));
        let mut set = HashSet::new();
        set.insert(a);
        set.insert(a);
        set.insert(b);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn copy_and_debug() {
        let a = PublicId::<Foo>::new();
        let copied = a; // Copy
        #[allow(clippy::clone_on_copy)]
        let cloned = a.clone();
        assert_eq!(a, copied);
        assert_eq!(a, cloned);
        assert_eq!(format!("{a:?}"), format!("PublicId({})", a.as_str()));
    }

    #[test]
    fn default_mints_distinct_values() {
        // Overwhelmingly likely to differ (55 bits); a failure means the RNG is
        // broken.
        let a = PublicId::<Foo>::default();
        let b = PublicId::<Foo>::default();
        assert_ne!(a, b);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trips_as_canonical_string() {
        let p = PublicId::<Bar>::new();
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(json, format!("\"{p}\""));
        let back: PublicId<Bar> = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
        assert!(serde_json::from_str::<PublicId<Foo>>("\"nope\"").is_err());

        // Prefixed entities serialize the prefixed form.
        let w = PublicId::<Widget>::new();
        let json = serde_json::to_string(&w).unwrap();
        assert!(json.starts_with("\"wgt_"));
        let back: PublicId<Widget> = serde_json::from_str(&json).unwrap();
        assert_eq!(w, back);
    }

    #[cfg(feature = "mysql")]
    #[test]
    fn mysql_binds_as_canonical_varchar() {
        use sqlx::{Encode, MySql, Type, TypeInfo};
        assert_eq!(<PublicId<Widget> as Type<MySql>>::type_info().name(), "VARCHAR");
        let p = PublicId::<Widget>::new();
        let canonical = p.to_string();
        // The prefixed form of a four-char-or-shorter prefix fits VARCHAR(16).
        assert!(canonical.len() <= 16);
        let mut buf = Vec::new();
        let null =
            <PublicId<Widget> as Encode<MySql>>::encode_by_ref(&p, &mut buf).expect("encode");
        assert!(matches!(null, sqlx::encode::IsNull::No));
        // Length-encoded string: one length byte, then the canonical text.
        assert_eq!(usize::from(buf[0]), canonical.len());
        assert_eq!(&buf[1..], canonical.as_bytes());
    }

    #[cfg(feature = "postgres")]
    #[test]
    fn postgres_binds_as_text() {
        use sqlx::postgres::{PgHasArrayType, PgTypeInfo};
        use sqlx::{Postgres, Type};
        assert_eq!(<PublicId<Widget> as Type<Postgres>>::type_info(), PgTypeInfo::with_name("TEXT"));
        assert!(<PublicId<Widget> as Type<Postgres>>::compatible(&PgTypeInfo::with_name("VARCHAR")));
        assert_eq!(
            <PublicId<Widget> as PgHasArrayType>::array_type_info(),
            <String as PgHasArrayType>::array_type_info()
        );
        // Binary protocol: the canonical text, no length prefix.
        let p = PublicId::<Widget>::new();
        let mut buf = sqlx::postgres::PgArgumentBuffer::default();
        let null = <PublicId<Widget> as sqlx::Encode<Postgres>>::encode_by_ref(&p, &mut buf)
            .expect("encode");
        assert!(matches!(null, sqlx::encode::IsNull::No));
        assert_eq!(&buf[..], p.to_string().as_bytes());
    }

    #[cfg(feature = "poem-openapi")]
    #[test]
    fn poem_parses_and_emits_json_strings() {
        use poem_openapi::types::{ParseFromJSON, ParseFromParameter, ToJSON};
        let p = PublicId::<Foo>::new();
        let value = p.to_json().expect("always some");
        let back = PublicId::<Foo>::parse_from_json(Some(value)).expect("round trip");
        assert_eq!(p, back);
        let from_param = PublicId::<Foo>::parse_from_parameter(p.as_str()).expect("param");
        assert_eq!(p, from_param);
        assert!(PublicId::<Foo>::parse_from_json(None).is_err());
        assert!(
            PublicId::<Foo>::parse_from_json(Some(serde_json::Value::Bool(true))).is_err()
        );
    }
}
