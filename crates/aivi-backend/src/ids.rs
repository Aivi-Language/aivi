macro_rules! define_local_id {
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

            pub const fn index(self) -> usize {
                self.0 as usize
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

aivi_base::define_arena_id!(pub ItemId);
aivi_base::define_arena_id!(pub PipelineId);
aivi_base::define_arena_id!(pub KernelId);
aivi_base::define_arena_id!(pub KernelExprId);
aivi_base::define_arena_id!(pub LayoutId);
aivi_base::define_arena_id!(pub SourceId);
aivi_base::define_arena_id!(pub DecodePlanId);
aivi_base::define_arena_id!(pub DecodeStepId);
define_local_id!(EnvSlotId);
define_local_id!(InlineSubjectId);
