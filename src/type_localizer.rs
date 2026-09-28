//! Elm's Reporting.Render.Type.Localizer rules for displayed type names.
use crate::module::{Exposed, Exposing, Header};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
struct Import {
    prefix: String,
    exposed: Option<BTreeSet<String>>,
}

#[derive(Debug, Default, Clone)]
pub struct Localizer {
    imports: BTreeMap<String, Import>,
}

impl Localizer {
    pub fn from_header(header: &Header, owner: &str) -> Self {
        let mut result = Self {
            imports: BTreeMap::new(),
        };
        result.insert(&header.name, &header.name, &Exposing::All);
        // The Haskell parser inserts these imports before explicit imports.
        // Rust keeps the original header and inserts defaults during analysis.
        if owner != "elm/core" {
            for (name, prefix, exposing) in crate::analyze::default_imports() {
                result.insert(name, prefix, &exposing);
            }
        }
        for import in &header.imports {
            result.insert(
                &import.name,
                import.alias.as_deref().unwrap_or(&import.name),
                &import.exposing,
            );
        }
        result
    }

    fn insert(&mut self, name: &str, prefix: &str, exposing: &Exposing) {
        let exposed = match exposing {
            Exposing::All => None,
            Exposing::Explicit(items) => Some(
                items
                    .iter()
                    .filter_map(|item| match item {
                        Exposed::Type { name, .. } => Some(name.clone()),
                        _ => None,
                    })
                    .collect(),
            ),
        };
        self.imports.insert(
            name.into(),
            Import {
                prefix: prefix.into(),
                exposed,
            },
        );
    }

    pub fn name(&self, canonical_module: &str, name: &str) -> String {
        let module = canonical_module
            .split_once(':')
            .map_or(canonical_module, |(_, module)| module);
        match self.imports.get(module) {
            None => format!("{module}.{name}"),
            Some(import)
                if import
                    .exposed
                    .as_ref()
                    .is_none_or(|names| names.contains(name))
                    || (canonical_module == "elm/core:List" && name == "List") =>
            {
                name.into()
            }
            Some(import) => format!("{}.{name}", import.prefix),
        }
    }
}
