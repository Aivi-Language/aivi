use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub(crate) const DOC_INDEX_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/doc_index.json"));

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum QuickInfoKind {
    Module,
    Function,
    Type,
    Class,
    Domain,
    Operator,
    ClassMember,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct QuickInfoEntry {
    pub(crate) kind: QuickInfoKind,
    pub(crate) name: String,
    pub(crate) module: Option<String>,
    pub(crate) content: String,
    pub(crate) signature: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct DocIndex {
    entries: Vec<QuickInfoEntry>,
    #[serde(skip)]
    by_name: HashMap<String, Vec<usize>>,
}

impl DocIndex {
    pub(crate) fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let entries: Vec<QuickInfoEntry> = serde_json::from_str(json)?;
        let mut index = DocIndex {
            entries,
            ..Default::default()
        };
        index.rebuild_maps();
        Ok(index)
    }

    pub(crate) fn lookup_best(&self, name: &str, module: Option<&str>) -> Option<&QuickInfoEntry> {
        let candidates = self.by_name.get(name)?;
        if let Some(module) = module {
            for i in candidates {
                let entry = self.entries.get(*i)?;
                if entry.module.as_deref() == Some(module) {
                    return Some(entry);
                }
            }
        }
        if candidates.len() == 1 {
            candidates.first().and_then(|i| self.entries.get(*i))
        } else {
            None
        }
    }

    pub(crate) fn lookup_module(&self, module_name: &str) -> Option<&QuickInfoEntry> {
        self.lookup_best(module_name, None)
            .filter(|entry| entry.kind == QuickInfoKind::Module)
    }

    fn rebuild_maps(&mut self) {
        self.by_name.clear();
        for (i, entry) in self.entries.iter().enumerate() {
            self.by_name.entry(entry.name.clone()).or_default().push(i);
        }
    }
}
