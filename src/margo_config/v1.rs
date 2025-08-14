use serde::{Deserialize, Serialize};
use std::str;
use url::Url;

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigV1 {
    pub base_url: Url,

    #[serde(default)]
    pub auth_required: bool,

    #[serde(default)]
    pub html: ConfigV1Html,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ConfigV1Html {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub suggested_registry_name: Option<String>,
}

impl ConfigV1Html {
    pub const USER_DEFAULT_SUGGESTED_REGISTRY_NAME: &'static str = "my-awesome-registry";
}

impl Default for ConfigV1 {
    fn default() -> Self {
        ConfigV1 {
            base_url: Url::parse("http://example.com").unwrap(),
            auth_required: false,
            html: ConfigV1Html {
                enabled: Some(true),
                suggested_registry_name: Some(
                    ConfigV1Html::USER_DEFAULT_SUGGESTED_REGISTRY_NAME.into(),
                ),
            },
        }
    }
}
