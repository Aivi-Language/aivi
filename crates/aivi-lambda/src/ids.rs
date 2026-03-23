use aivi_core::ArenaId;

macro_rules! define_arena_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u32);

        impl $name {
            pub const fn from_raw(raw: u32) -> Self {
                Self(raw)
            }

            pub const fn as_raw(self) -> u32 {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl ArenaId for $name {
            fn from_raw(raw: u32) -> Self {
                Self(raw)
            }

            fn as_raw(self) -> u32 {
                self.0
            }
        }
    };
}

define_arena_id!(ClosureId);
define_arena_id!(CaptureId);
