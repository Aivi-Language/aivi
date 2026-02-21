use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct BindingQueryKey {
    pub(super) module: String,
    pub(super) binding: String,
}

#[derive(Default, Clone, Debug)]
pub(super) struct TypeQueryCache {
    binding_types: HashMap<BindingQueryKey, String>,
}

impl TypeQueryCache {
    pub(super) fn store_binding_type(
        &mut self,
        module: impl Into<String>,
        binding: impl Into<String>,
        rendered_type: impl Into<String>,
    ) {
        let key = BindingQueryKey {
            module: module.into(),
            binding: binding.into(),
        };
        self.binding_types.insert(key, rendered_type.into());
    }

    #[allow(dead_code)]
    pub(super) fn get_binding_type(&self, module: &str, binding: &str) -> Option<&str> {
        let key = BindingQueryKey {
            module: module.to_string(),
            binding: binding.to_string(),
        };
        self.binding_types.get(&key).map(|s| s.as_str())
    }

    pub(super) fn clear_module(&mut self, module: &str) {
        self.binding_types.retain(|k, _| k.module != module);
    }
}
