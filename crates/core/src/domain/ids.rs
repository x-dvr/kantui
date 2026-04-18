use std::fmt;

/// Opaque 128-bit identifier. Adapters choose the encoding (UUIDv4, ULID, ...);
/// the domain only sees the bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId([u8; 16]);

impl EntityId {
    #[must_use]
    pub const fn from_bytes(b: [u8; 16]) -> Self {
        Self(b)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    #[must_use]
    pub const fn nil() -> Self {
        Self([0u8; 16])
    }
}

impl fmt::Debug for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EntityId({self})")
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

macro_rules! newtype_id {
    ($name:ident) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name(pub EntityId);

        impl $name {
            #[must_use]
            pub const fn new(id: EntityId) -> Self {
                Self(id)
            }

            #[must_use]
            pub const fn inner(self) -> EntityId {
                self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

newtype_id!(ProjectId);
newtype_id!(StateId);
newtype_id!(TaskId);
newtype_id!(TagId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_hex() {
        let id = EntityId::from_bytes([0xab; 16]);
        assert_eq!(id.to_string(), "abababababababababababababababab");
    }

    #[test]
    fn newtypes_do_not_mix() {
        let p = ProjectId::new(EntityId::nil());
        let s = StateId::new(EntityId::nil());
        // compile-time: this would fail if we tried to compare p and s directly.
        assert_eq!(p.inner(), s.inner());
    }
}
