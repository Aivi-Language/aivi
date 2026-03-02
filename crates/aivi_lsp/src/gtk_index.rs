use std::collections::HashMap;

use serde::Deserialize;

pub const GTK_INDEX_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/gtk_index.json"));

#[derive(Debug, Clone, Deserialize)]
pub struct GtkWidgetInfo {
    pub name: String,
    pub parent: Option<String>,
    #[serde(default)]
    pub doc: Option<String>,
    pub properties: Vec<GtkPropertyInfo>,
    pub signals: Vec<GtkSignalInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GtkPropertyInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub prop_type: String,
    pub writable: bool,
    pub construct_only: bool,
    #[serde(default)]
    pub default_value: Option<String>,
    #[serde(default)]
    pub doc: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GtkSignalInfo {
    pub name: String,
    #[serde(default)]
    pub doc: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct GtkIndex {
    widgets: Vec<GtkWidgetInfo>,
    by_name: HashMap<String, usize>,
}

impl GtkIndex {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let widgets: Vec<GtkWidgetInfo> = serde_json::from_str(json)?;
        let by_name: HashMap<String, usize> = widgets
            .iter()
            .enumerate()
            .map(|(i, w)| (w.name.clone(), i))
            .collect();
        Ok(GtkIndex { widgets, by_name })
    }

    pub fn lookup(&self, name: &str) -> Option<&GtkWidgetInfo> {
        self.by_name.get(name).and_then(|&i| self.widgets.get(i))
    }

    /// Returns widget names matching a prefix.
    pub fn complete_widget_name(&self, prefix: &str) -> Vec<&GtkWidgetInfo> {
        self.widgets
            .iter()
            .filter(|w| w.name.starts_with(prefix))
            .collect()
    }

    /// Returns all widget names.
    pub fn all_widget_names(&self) -> impl Iterator<Item = &str> {
        self.widgets.iter().map(|w| w.name.as_str())
    }
}
