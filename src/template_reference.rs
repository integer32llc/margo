use argh::FromArgValue;
use enum_iterator::Sequence;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// References a template to be used.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum TemplateReference {
    /// A built-in template identified by its name.
    BuiltIn(BuiltInTemplate),

    /// A .tar file containing a custom template.
    File(PathBuf),
}

impl FromArgValue for TemplateReference {
    fn from_arg_value(value: &str) -> Result<Self, String> {
        Ok(match serde_json::from_str::<BuiltInTemplate>(value) {
            // If it parses as a built-in template, then we simply return it.
            Ok(builtin) => TemplateReference::BuiltIn(builtin),
            // Otherwise, assume the given string is a path.
            Err(_) => TemplateReference::File(PathBuf::from(value)),
        })
    }
}

/// The built-in templates.
#[derive(Debug, Serialize, Deserialize, Sequence, Clone, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BuiltInTemplate {
    Classic,
    Bright,
}

impl BuiltInTemplate {
    /// Get the bytes of the tar archive containing this template.
    pub fn tar_bytes(&self) -> &[u8] {
        match self {
            Self::Classic => include_bytes!("templates/classic.tar"),
            Self::Bright => include_bytes!("templates/bright.tar"),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Classic => "classic",
            Self::Bright => "bright",
        }
    }

    pub fn all() -> impl Iterator<Item = BuiltInTemplate> {
        enum_iterator::all::<Self>()
    }
}
