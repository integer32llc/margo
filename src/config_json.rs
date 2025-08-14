use crate::margo_config::LatestConfig;
use crate::prelude::*;
use serde::Serialize;

/// The config.json file required for the registry.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ConfigJson {
    // This field cannot be a `url::Url` because that type
    // percent-escapes the `{` and `}` characters. Cargo performs
    // string-replacement on this value based on those literal `{`
    // and `}` characters.
    pub dl: String,

    pub api: Option<String>, // Modified

    /// A private registry requires all operations to be authenticated.
    ///
    /// This includes API requests, crate downloads and sparse
    /// index updates.
    pub auth_required: bool,
}

impl ConfigJson {
    pub fn new(config: &LatestConfig) -> Result<Self> {
        Ok(ConfigJson {
            dl: format!("{base}/crates/{{lowerprefix}}/{{crate}}/{{version}}.crate", base = config.base_url),
            api: None,
            auth_required: config.auth_required,
        })
    }
}
