//! Typed, sortable entity identifiers.
//!
//! `Id<T>` is a `UUIDv7` tagged with the entity it identifies, so an `Id<Order>`
//! cannot be passed where an `Id<Item>` is expected. `UUIDv7` is time-ordered, so
//! primary-key inserts stay append-friendly and sorting by id approximates
//! creation order. Runtime representation is exactly a `Uuid` (zero-cost tag).
//!
//! This is the *internal* identifier: it stays inside `core`/`db`/`engine` and
//! never crosses the public boundary — that's [`crate::PublicId`]'s job
//! (see `standards/docs/public-ids.md`).

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::str::FromStr;

use uuid::Uuid;

pub struct Id<T> {
    raw: Uuid,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    /// Mint a fresh, time-ordered identifier.
    #[must_use]
    pub fn new() -> Self {
        Self::from_uuid(Uuid::now_v7())
    }

    #[must_use]
    pub const fn from_uuid(raw: Uuid) -> Self {
        Self { raw, _marker: PhantomData }
    }

    #[must_use]
    pub const fn as_uuid(&self) -> Uuid {
        self.raw
    }
}

impl<T> Default for Id<T> {
    fn default() -> Self {
        Self::new()
    }
}

// Manual impls so the PhantomData type parameter doesn't force `T: Trait` bounds.
impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Id<T> {}
impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}
impl<T> Eq for Id<T> {}
impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}
impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.raw.cmp(&other.raw)
    }
}

impl<T> fmt::Display for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}
impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({})", self.raw)
    }
}

impl<T> FromStr for Id<T> {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from_uuid(Uuid::from_str(s)?))
    }
}

impl<T> From<Uuid> for Id<T> {
    fn from(raw: Uuid) -> Self {
        Self::from_uuid(raw)
    }
}
impl<T> From<Id<T>> for Uuid {
    fn from(id: Id<T>) -> Self {
        id.as_uuid()
    }
}

// ── serde: canonical hyphenated UUID string ──────────────────────────────────
#[cfg(feature = "serde")]
impl<T> serde::Serialize for Id<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.raw.to_string())
    }
}
#[cfg(feature = "serde")]
impl<'de, T> serde::Deserialize<'de> for Id<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Uuid::from_str(&s).map(Self::from_uuid).map_err(serde::de::Error::custom)
    }
}

// ── sqlx (SQLite): delegate to Uuid, so ids store as BLOB ────────────────────
#[cfg(feature = "sqlite")]
impl<T> sqlx::Type<sqlx::Sqlite> for Id<T> {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <Uuid as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
    fn compatible(ty: &sqlx::sqlite::SqliteTypeInfo) -> bool {
        <Uuid as sqlx::Type<sqlx::Sqlite>>::compatible(ty)
    }
}
#[cfg(feature = "sqlite")]
impl<'q, T> sqlx::Encode<'q, sqlx::Sqlite> for Id<T> {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <Uuid as sqlx::Encode<'q, sqlx::Sqlite>>::encode_by_ref(&self.raw, buf)
    }
}
#[cfg(feature = "sqlite")]
impl<'r, T> sqlx::Decode<'r, sqlx::Sqlite> for Id<T> {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        Ok(Self::from_uuid(<Uuid as sqlx::Decode<sqlx::Sqlite>>::decode(value)?))
    }
}

// ── sqlx (MySQL/MariaDB): delegate to Uuid, so ids store as BINARY(16) ──────
// Same delegation as SQLite: the 16 raw bytes, never the 36-char text form. A
// `CHAR(36)` column type-checks (sqlx lets `[u8]` read string columns) but
// fails at decode with a length error, so the schema must be `BINARY(16)`.
#[cfg(feature = "mysql")]
impl<T> sqlx::Type<sqlx::MySql> for Id<T> {
    fn type_info() -> sqlx::mysql::MySqlTypeInfo {
        <Uuid as sqlx::Type<sqlx::MySql>>::type_info()
    }
    fn compatible(ty: &sqlx::mysql::MySqlTypeInfo) -> bool {
        <Uuid as sqlx::Type<sqlx::MySql>>::compatible(ty)
    }
}
#[cfg(feature = "mysql")]
impl<T> sqlx::Encode<'_, sqlx::MySql> for Id<T> {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<u8>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <Uuid as sqlx::Encode<'_, sqlx::MySql>>::encode_by_ref(&self.raw, buf)
    }
}
#[cfg(feature = "mysql")]
impl<'r, T> sqlx::Decode<'r, sqlx::MySql> for Id<T> {
    fn decode(value: sqlx::mysql::MySqlValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        Ok(Self::from_uuid(<Uuid as sqlx::Decode<sqlx::MySql>>::decode(value)?))
    }
}

// ── sqlx (PostgreSQL): delegate to Uuid, so ids store as native UUID ────────
// Postgres has a real UUID type, so there's no byte-vs-text choice to make: a
// TEXT column holding uuids is a type mismatch, not an `Id<T>` column.
#[cfg(feature = "postgres")]
impl<T> sqlx::Type<sqlx::Postgres> for Id<T> {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <Uuid as sqlx::Type<sqlx::Postgres>>::type_info()
    }
    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <Uuid as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}
// `UUID[]`, so a `Vec<Id<T>>` binds for `WHERE id = ANY($1)`.
#[cfg(feature = "postgres")]
impl<T> sqlx::postgres::PgHasArrayType for Id<T> {
    fn array_type_info() -> sqlx::postgres::PgTypeInfo {
        <Uuid as sqlx::postgres::PgHasArrayType>::array_type_info()
    }
}
#[cfg(feature = "postgres")]
impl<T> sqlx::Encode<'_, sqlx::Postgres> for Id<T> {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <Uuid as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.raw, buf)
    }
}
#[cfg(feature = "postgres")]
impl<'r, T> sqlx::Decode<'r, sqlx::Postgres> for Id<T> {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        Ok(Self::from_uuid(<Uuid as sqlx::Decode<sqlx::Postgres>>::decode(value)?))
    }
}

// ── poem-openapi: a `string` (uuid) in the contract ──────────────────────────
#[cfg(feature = "poem-openapi")]
mod poem_impls {
    use std::borrow::Cow;

    use poem_openapi::registry::{MetaSchema, MetaSchemaRef};
    use poem_openapi::types::{
        ParseError as OaiParseError, ParseFromJSON, ParseFromParameter, ParseResult, ToJSON, Type,
    };
    use serde_json::Value;

    use super::Id;

    impl<T: Send + Sync> Type for Id<T> {
        const IS_REQUIRED: bool = true;
        type RawValueType = Self;
        type RawElementValueType = Self;

        fn name() -> Cow<'static, str> {
            "id".into()
        }
        fn schema_ref() -> MetaSchemaRef {
            MetaSchemaRef::Inline(Box::new(MetaSchema::new_with_format("string", "uuid")))
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
    impl<T: Send + Sync> ParseFromJSON for Id<T> {
        fn parse_from_json(value: Option<Value>) -> ParseResult<Self> {
            match value.unwrap_or_default() {
                Value::String(s) => s.parse().map_err(OaiParseError::custom),
                other => Err(OaiParseError::expected_type(other)),
            }
        }
    }
    impl<T: Send + Sync> ParseFromParameter for Id<T> {
        fn parse_from_parameter(value: &str) -> ParseResult<Self> {
            value.parse().map_err(OaiParseError::custom)
        }
    }
    impl<T: Send + Sync> ToJSON for Id<T> {
        fn to_json(&self) -> Option<Value> {
            Some(Value::String(self.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Id;
    use uuid::Uuid;

    struct Widget;
    struct Gadget;

    #[test]
    fn round_trips_through_display_and_parse() {
        let id = Id::<Widget>::new();
        let parsed: Id<Widget> = id.to_string().parse().expect("canonical uuid parses");
        assert_eq!(id, parsed);
    }

    #[test]
    fn new_ids_are_time_ordered() {
        let a = Id::<Widget>::new();
        let b = Id::<Widget>::new();
        assert!(a < b, "UUIDv7 ids mint in ascending order");
        assert_eq!(a.partial_cmp(&b), Some(a.cmp(&b)));
    }

    #[test]
    fn uuid_conversions_round_trip() {
        let raw = Uuid::now_v7();
        let id: Id<Widget> = raw.into();
        assert_eq!(id.as_uuid(), raw);
        let back: Uuid = id.into();
        assert_eq!(back, raw);
    }

    #[test]
    fn copy_hash_and_debug() {
        use std::collections::HashSet;
        let a = Id::<Widget>::new();
        let copied = a; // Copy
        #[allow(clippy::clone_on_copy)]
        let cloned = a.clone();
        assert_eq!(a, copied);
        assert_eq!(a, cloned);
        assert_eq!(format!("{a:?}"), format!("Id({a})"));
        let mut set = HashSet::new();
        set.insert(a);
        set.insert(a);
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn default_mints_fresh() {
        let a = Id::<Gadget>::default();
        let b = Id::<Gadget>::default();
        assert_ne!(a, b);
    }

    #[test]
    fn rejects_malformed_uuid() {
        assert!("not-a-uuid".parse::<Id<Widget>>().is_err());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trips_as_hyphenated_string() {
        let id = Id::<Widget>::new();
        let json = serde_json::to_string(&id).expect("serialize");
        assert_eq!(json, format!("\"{id}\""));
        let back: Id<Widget> = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(id, back);
        assert!(serde_json::from_str::<Id<Widget>>("\"nope\"").is_err());
    }

    // Decode needs a live row (MySqlValueRef has no public constructor); that
    // path is covered by the MySQL round-trip in `lib.rs`.
    #[cfg(feature = "mysql")]
    #[test]
    fn mysql_binds_as_the_16_raw_bytes() {
        use sqlx::{Encode, MySql, Type, TypeInfo};
        let id = Id::<Widget>::new();
        assert_eq!(
            <Id<Widget> as Type<MySql>>::type_info().name(),
            <Uuid as Type<MySql>>::type_info().name()
        );
        assert!(<Id<Widget> as Type<MySql>>::compatible(&<Uuid as Type<MySql>>::type_info()));
        let mut buf = Vec::new();
        let null = <Id<Widget> as Encode<MySql>>::encode_by_ref(&id, &mut buf).expect("encode");
        assert!(matches!(null, sqlx::encode::IsNull::No));
        // Length-encoded: one length byte (16), then the raw UUID bytes.
        assert_eq!(buf.len(), 17);
        assert_eq!(buf[0], 16);
        assert_eq!(&buf[1..], id.as_uuid().as_bytes());
    }

    #[cfg(feature = "postgres")]
    #[test]
    fn postgres_binds_as_native_uuid() {
        use sqlx::postgres::{PgHasArrayType, PgTypeInfo};
        use sqlx::{Postgres, Type};
        assert_eq!(<Id<Widget> as Type<Postgres>>::type_info(), PgTypeInfo::with_name("UUID"));
        assert!(<Id<Widget> as Type<Postgres>>::compatible(&PgTypeInfo::with_name("UUID")));
        assert!(!<Id<Widget> as Type<Postgres>>::compatible(&PgTypeInfo::with_name("TEXT")));
        assert_eq!(
            <Id<Widget> as PgHasArrayType>::array_type_info(),
            <Uuid as PgHasArrayType>::array_type_info()
        );
        // Binary protocol: the 16 raw bytes, no length prefix.
        let id = Id::<Widget>::new();
        let mut buf = sqlx::postgres::PgArgumentBuffer::default();
        let null = <Id<Widget> as sqlx::Encode<Postgres>>::encode_by_ref(&id, &mut buf)
            .expect("encode");
        assert!(matches!(null, sqlx::encode::IsNull::No));
        assert_eq!(&buf[..], id.as_uuid().as_bytes());
    }

    #[cfg(feature = "poem-openapi")]
    #[test]
    fn poem_parses_and_emits_json_strings() {
        use poem_openapi::types::{ParseFromJSON, ParseFromParameter, ToJSON};
        let id = Id::<Widget>::new();
        let value = id.to_json().expect("always some");
        let back = Id::<Widget>::parse_from_json(Some(value)).expect("round trip");
        assert_eq!(id, back);
        let from_param = Id::<Widget>::parse_from_parameter(&id.to_string()).expect("param");
        assert_eq!(id, from_param);
        assert!(Id::<Widget>::parse_from_json(None).is_err());
        assert!(Id::<Widget>::parse_from_json(Some(serde_json::Value::Bool(true))).is_err());
    }
}
