use super::index::{Dependency, DependencyKind, IndexEntry};
use crate::margo_config::LatestConfig;
use crate::prelude::*;
use crate::registry::packaged_cargo_toml::PackagedCargoToml;
use lazy_static::lazy_static;
use sha2::Digest;
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use url::Url;

lazy_static! {
    /// The global crates.io index.
    pub static ref CRATES_IO_INDEX_URL: Url = "https://github.com/rust-lang/crates.io-index"
        .parse()
        .expect("Invalid crates.io index URL");
}

/// Information about a packaged .crate file.
pub struct PackagedCrate {
    /// Contents of the .crate file.
    contents: Vec<u8>,

    /// Checksum representing the .crate file.
    checksum: String,

    /// Contents of the readme.md file, if one is present.
    pub readme: Option<String>,

    /// File paths contained in the .crate file.
    pub files: Vec<PathBuf>,

    /// Root Cargo.toml manifest inside the package.
    pub manifest: PackagedCargoToml,
}

impl PackagedCrate {
    /// Open a .crate file at a given path and collect its metadata.
    pub fn open(path: &Path) -> Result<Self> {
        let contents =
            fs::read(path).context(format!("Can't read .crate file at {}.", path.display()))?;

        let checksum = sha2::Sha256::digest(&contents);
        let checksum = hex::encode(checksum);

        // Read the files in the archive to populate the files set and extract the cargo.toml.
        let crate_archive = flate2::read::GzDecoder::new(&contents[..]);
        let mut crate_archive = tar::Archive::new(crate_archive);

        // Variables to collect the data we need while going through the entries in the archive.
        let mut files = Vec::new();
        let mut manifest = None;
        let mut readme = None;

        let entries = crate_archive
            .entries()
            .context("Can't read the contents of the crate.")?;

        for entry in entries {
            let mut entry = entry?;
            let entry_path = entry.path()?;

            // Path.is_file() doesn't seem to work accurately inside a tarball archive but if
            // the entry has an extension it's probably a file.
            if entry_path.extension().is_some() {
                files.push(entry_path.to_path_buf());
            }

            // Cargo packages the crate in a root dir inside the .crate archive,
            // so to determine whether an entry is in the root dir, it should have
            // at most one parent directory.
            let is_in_root_dir = entry_path
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.to_str())
                .map(|p| p == "")
                .unwrap_or(false);

            if !is_in_root_dir {
                continue;
            }

            // Collect the Cargo.toml manifest when we encounter it.
            if entry_path.ends_with("Cargo.toml") {
                let mut data = String::with_capacity(entry.size() as usize);
                entry
                    .read_to_string(&mut data)
                    .context("Can't read the Cargo.toml file from the crate.")?;

                manifest = Some(
                    toml::from_str::<PackagedCargoToml>(&data)
                        .context("Can't parse the Cargo.toml in the crate.")?,
                );
            } else if entry_path.ends_with("README.md") {
                let mut data = String::with_capacity(entry.size() as usize);
                entry
                    .read_to_string(&mut data)
                    .context("Can't read the README.md file from the crate.")?;

                readme = Some(data);
            }
        }

        let manifest = manifest.context("Can't find a Cargo.toml file in the crate.")?;

        Ok(PackagedCrate {
            contents,
            checksum,
            manifest,
            readme,
            files,
        })
    }

    /// Format the metadata into a valid index entry structure.
    ///
    /// Also returns the contents of the crate file, so it can be written to disk along with the index.
    pub fn into_index_entry(mut self, config: &LatestConfig) -> Result<(IndexEntry, Vec<u8>)> {
        // Collect all regular and build dependencies, but not dev dependencies since those are
        // irrelevant when installing a crate from a registry.
        let dependencies = [self.manifest.build_dependencies, self.manifest.dependencies]
            .into_iter()
            .flatten()
            .collect::<BTreeMap<_, _>>();

        // Remove features that only refer to dev dependencies
        // Find all dev-only dependency names
        let dev_deps = self
            .manifest
            .dev_dependencies
            .into_keys()
            .filter(|name| !dependencies.contains_key(name))
            // We don't care about the official package name here as the feature syntax
            // has to match the user-specified dependency name.
            .map(|name| format!("{name}/"));

        for prefix in dev_deps {
            for val in self.manifest.features.values_mut() {
                val.retain(|v| !v.starts_with(&prefix));
            }
        }

        // Map the dependency information into the expected index format.
        let deps = dependencies
            .into_iter()
            .map(|(name, dep)| Dependency {
                name: name.clone(),
                req: dep.version,
                features: dep.features,
                optional: dep.optional,
                default_features: dep.default_features,
                target: None,
                kind: DependencyKind::Normal,
                registry: match dep.registry_index {
                    // If the dependency lists no registry, it refers to crates.io. Since this
                    // is a custom registry, we need to make that reference explicit.
                    None => Some(CRATES_IO_INDEX_URL.clone()),
                    // If the dependency lists a custom registry that matches this one,
                    // we can set it to None so cargo knows it's a local reference.
                    Some(url) if url == config.base_url => None,
                    // Otherwise we just pass the registry URL through.
                    Some(url) => Some(url),
                },
                package: Some(self.manifest.package.name.clone()),
            })
            .collect();

        let entry = IndexEntry {
            name: self.manifest.package.name,
            vers: self.manifest.package.version,
            deps,
            cksum: self.checksum,
            features: self.manifest.features,
            yanked: false,
            links: None,
            v: 2,
            features2: Default::default(),
            rust_version: self.manifest.package.rust_version,
        };

        Ok((entry, self.contents))
    }
}
