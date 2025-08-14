use cargo_util_schemas::manifest::{FeatureName, PackageName, RustVersion};
use semver::{Version, VersionReq};
use serde::Deserialize;
use std::collections::BTreeMap;
use url::Url;

pub type DependencyMap = BTreeMap<PackageName, Dependency>;

/// A normalised Cargo.toml manifest in a packaged .crate file.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PackagedCargoToml {
    pub package: Package,

    #[serde(default)]
    pub features: BTreeMap<FeatureName, Vec<String>>,

    #[serde(default)]
    pub dependencies: DependencyMap,

    #[serde(default)]
    pub build_dependencies: DependencyMap,

    #[serde(default)]
    pub dev_dependencies: DependencyMap,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Package {
    pub name: PackageName,

    pub version: Version,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub rust_version: Option<RustVersion>,

    #[serde(default)]
    pub repository: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Dependency {
    pub version: VersionReq,

    #[serde(default)]
    pub features: Vec<FeatureName>,

    #[serde(default)]
    pub optional: bool,

    #[serde(default = "default_features")]
    pub default_features: bool,

    #[serde(default)]
    pub registry_index: Option<Url>,
}

/// Default value for the `default_features` field in [Dependency].
fn default_features() -> bool {
    true
}
