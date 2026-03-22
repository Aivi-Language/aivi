use crate::arena::ArenaId;

macro_rules! define_id {
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

define_id!(ItemId);
define_id!(ExprId);
define_id!(PatternId);
define_id!(TypeId);
define_id!(DecoratorId);
define_id!(MarkupNodeId);
define_id!(ControlNodeId);
define_id!(ClusterId);
define_id!(BindingId);
define_id!(TypeParameterId);
define_id!(ImportId);
