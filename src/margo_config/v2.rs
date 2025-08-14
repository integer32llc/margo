use crate::template_reference::{BuiltInTemplate, TemplateReference};
use crate::util::UrlExt;
use serde::{Deserialize, Serialize};
use std::str;
use url::Url;

use super::v1::{ConfigV1, ConfigV1Html};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConfigV2 {
    /// The public URL that the registry is hosted at.
    pub base_url: Url,

    /// True if authentication is required to download crates (i.e. this is a private registry).
    pub auth_required: bool,

    /// HTML rendering configuration, or None if no html pages should be rendered.
    pub html: Option<ConfigV2Html>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConfigV2Html {
    /// Template to use when rendering the index page.
    pub template: TemplateReference,

    /// Page title to display.
    pub title: String,

    /// Suggested name for the registry.
    pub suggested_registry_name: String,
}

impl ConfigV2 {
    pub fn normalised(mut self) -> Self {
        self.base_url.ensure_trailing_slash();
        self
    }
}

impl From<ConfigV1> for ConfigV2 {
    fn from(v1: ConfigV1) -> Self {
        ConfigV2 {
            base_url: v1.base_url,
            auth_required: v1.auth_required,
            html: match v1.html.enabled {
                Some(true) => Some(ConfigV2Html {
                    template: TemplateReference::BuiltIn(BuiltInTemplate::Classic),
                    title: "Margo Crate Registry".into(),
                    suggested_registry_name: v1
                        .html
                        .suggested_registry_name
                        .unwrap_or(ConfigV1Html::USER_DEFAULT_SUGGESTED_REGISTRY_NAME.to_string()),
                }),
                _ => None,
            },
        }
        .normalised()
    }
}
