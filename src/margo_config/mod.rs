mod v1;
mod v2;

use serde::{Deserialize, Serialize};
use std::str;

use self::v1::ConfigV1;
pub use self::v2::ConfigV2 as LatestConfig;
pub use self::v2::ConfigV2Html as LatestConfigIndex;

/// Supported margo_config versions for backwards compatibility.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "version")]
pub enum MargoConfig {
    #[serde(rename = "1")]
    V1(ConfigV1),
    #[serde(rename = "2")]
    V2(LatestConfig),
}

impl MargoConfig {
    /// Converts the loaded configuration file into the latest version.
    ///
    /// The returned margo_config is normalised.
    pub fn into_latest(self) -> LatestConfig {
        match self {
            MargoConfig::V1(c) => LatestConfig::from(c).normalised(),
            MargoConfig::V2(c) => c.normalised(),
        }
    }

    /// Returns a new Config enum containing the converted configuration using [Self::into_latest].
    pub fn with_latest(self) -> Self {
        Self::V2(self.into_latest())
    }

    /// True if the contained config is the latest version.
    pub fn is_latest(&self) -> bool {
        matches!(self, MargoConfig::V2(_))
    }
}

impl Default for MargoConfig {
    fn default() -> Self {
        MargoConfig::V1(ConfigV1::default()).with_latest()
    }
}

impl From<LatestConfig> for MargoConfig {
    fn from(config: LatestConfig) -> Self {
        MargoConfig::V2(config)
    }
}
