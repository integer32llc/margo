use crate::config_json::ConfigJson;
use crate::margo_config::{LatestConfig, MargoConfig};
use crate::prelude::*;
use crate::registry::{Index, PackagedCrate};
use crate::util::PathExt;
use crate::Result;

use cargo_util_schemas::manifest::PackageName;
use colored::Colorize;
use rayon::prelude::*;
use semver::Version;
use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{fs, str};
use walkdir::WalkDir;

pub const MARGO_CONFIG_FILE_NAME: &str = "margo-config.toml";
pub const MARGO_INDEX_FILE_NAME: &str = "margo-index.json";
const CONFIG_JSON_FILE_NAME: &str = "config.json";

const CRATES_DIR_NAME: &str = "crates";

#[derive(Debug)]
pub struct Registry {
    path: PathBuf,
    config: LatestConfig,
    index: BTreeSet<PackageName>,
}

impl Registry {
    /// Get the root directory containing the registry files.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the loaded registry configuration.
    pub fn config(&self) -> &LatestConfig {
        &self.config
    }

    /// Initialise a new registry and create the basic file structure.
    pub fn initialise(path: impl AsRef<Path>, config: LatestConfig) -> Result<Self> {
        println!("Initialising new registry");

        // Create the registry directory
        let path = path.as_ref();
        fs::create_dir_all(path).context("Can't create registry directory.")?;

        // Initialise Registry struct
        let registry = Registry {
            path: path.to_path_buf(),
            index: Default::default(),
            config,
        };

        // Create registry config.json
        let config_json_path = path.join(CONFIG_JSON_FILE_NAME);
        let config_json = ConfigJson::new(registry.config())?;
        let config_json = serde_json::to_string(&config_json)?;
        fs::write(&config_json_path, config_json)
            .context("Can't write config.json to registry directory.")?;

        // Create the Margo config file
        registry.save_config()?;

        // Render the initial index.html
        registry.maybe_generate_html()?;

        Ok(registry)
    }

    /// Open an existing registry in a directory.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        println!("Opening registry");

        let path = path.as_ref().to_path_buf();

        let config_path = path.join(MARGO_CONFIG_FILE_NAME);
        let config = fs::read_to_string(&config_path).context(format!(
            "Can't read margo-config.toml in {path}.",
            path = path.display()
        ))?;
        let config =
            toml::from_str::<MargoConfig>(&config).context("Can't parse margo-config.toml.")?;

        let index_path = path.join(MARGO_INDEX_FILE_NAME);
        let index = match fs::read_to_string(&index_path) {
            Ok(index) => Some(index),
            Err(e) if e.kind() == ErrorKind::NotFound => None,
            Err(e) => Err(e).context("Can't read margo-index.json.")?,
        };
        // If the index file doesn't exist, refresh it from disk.
        let should_refresh_index = index.is_none();
        let index = match index {
            Some(json) => serde_json::from_str::<BTreeSet<PackageName>>(&json)
                .context("Can't parse margo-index.json.")?,
            None => Default::default(),
        };

        // If the loaded config is not the latest version, we should resave it in the latest format
        // once it's been converted below.
        let should_resave = !config.is_latest();

        let mut registry = Registry {
            path: path.to_path_buf(),
            config: config.into_latest(),
            index,
        };

        if should_refresh_index {
            registry.refresh_index_from_disk()?;
        }

        if should_resave || should_refresh_index {
            registry.save_config()?;
        }

        Ok(registry)
    }

    /// Add a crate to the registry.
    ///
    /// The crate path should point to the `target/package/*.crate` file in the project you want to
    /// publish. Run `cargo package` to create this file.
    pub fn add(&self, crate_path: impl AsRef<Path>) -> Result<()> {
        let crate_path = crate_path.as_ref();
        let crate_path = crate_path
            .canonicalize()
            .context(format!("Can't resolve path {}", crate_path.display()))?;

        println!("Adding crate to registry");
        println!(" from {}", crate_path.display().to_string().cyan());

        let packaged_crate = PackagedCrate::open(&crate_path)?;

        self.add_from_packaged_crate(packaged_crate)
    }

    /// Add a crate to the registry from a loaded crate package.
    fn add_from_packaged_crate(&self, packaged_crate: PackagedCrate) -> Result<()> {
        let (entry, crate_contents) = packaged_crate.into_index_entry(&self.config)?;

        // Make sure the version doesn't exist yet.
        let mut index = Index::open_or_new(entry.name.clone(), self)?;
        if index.contains_version(&entry.vers) {
            return Err(MargoError::DuplicateVersion(entry.name, entry.vers).into());
        }

        // Create directories
        fs::create_dir_all(self.index_dir_for(&entry.name))?;
        fs::create_dir_all(self.crate_dir_for(&entry.name))?;

        // Get the crate path first because we can't access `entry` any more after adding it to the index.
        let crate_path = self.crate_file_path_for(&entry.name, &entry.vers);

        index.add(entry);
        index.save()?;

        // FUTURE: Stronger file system consistency (atomic file overwrites, rollbacks on error)
        // FUTURE: "transactional" adding of multiple crates

        fs::write(&crate_path, crate_contents)
            .context("Can't write crate to registry crates directory.")?;
        println!("Wrote crate to `{}`", crate_path.display());

        Ok(())
    }

    /// Delete a crate version from the registry.
    ///
    /// This deletes the entry from the index and the corresponding .crate file from
    /// the registry. You should only do this in cases of abuse or when testing. Prefer
    /// [Self::yank] instead, for regular production usage.
    pub fn remove(&self, name: &PackageName, version: &Version) -> Result<()> {
        let mut index = Index::open_or_new(name.clone(), self)?;
        index.remove(version);
        index.save()?;

        let crate_file_path = self.crate_file_path_for(name, version);

        match fs::remove_file(&crate_file_path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e).context("Can't delete removed crate file."),
        }?;

        crate_file_path.remove_dirs_if_empty()?;

        Ok(())
    }

    /// Yank a crate version from the registry.
    pub fn yank(&self, name: &PackageName, version: &Version, yanked: bool) -> Result<()> {
        let mut index = Index::open_or_new(name.clone(), self)?;
        index.set_yanked(version, yanked)?;
        index.save()?;

        Ok(())
    }

    /// List all indexes in the registry.
    pub fn list_indexes(&self) -> Result<Vec<Index>> {
        self.index
            .par_iter()
            .map(|n| Index::open_or_new(n.clone(), self))
            .collect()
    }

    /// Get the .crate package information for the given version.
    pub fn read_crate_info(&self, name: &PackageName, version: &Version) -> Result<PackagedCrate> {
        PackagedCrate::open(&self.crate_file_path_for(name, version))
    }

    /// Refresh the Margo package index by scanning the .crate files on disk.
    pub fn refresh_index_from_disk(&mut self) -> Result<()> {
        println!("Building registry index from disk");

        self.index = WalkDir::new(self.crate_dir())
            .into_iter()
            .filter_map(|e| e.ok())
            // Ignore anything that isn't a .crate file.
            .filter_map(|e| match e.path().extension() {
                Some(ext) if ext == "crate" => Some(e),
                _ => None,
            })
            .filter_map(|e| Some(e.path().parent()?.file_name()?.to_str()?.to_string()))
            .map(|n| {
                PackageName::from_str(&n).context(format!("Invalid package name: {name}", name = n))
            })
            .collect::<Result<BTreeSet<PackageName>>>()?;

        println!("Index updated");
        Ok(())
    }

    /// Generate HTML if the registry is configured to output HTML contents.
    ///
    /// This method simply calls [Registry::generate_html] if the html config is set.
    pub fn maybe_generate_html(&self) -> Result<()> {
        match &self.config.html {
            Some(_) => self.generate_html(),
            _ => Ok(()),
        }
    }

    /// Generate the HTML contents using the configured template.
    #[cfg(feature = "html")]
    pub fn generate_html(&self) -> Result<()> {
        use crate::template::Template;

        let config = &self
            .config
            .html
            .as_ref()
            .context("Missing HTML configuration in registry config.")?;

        let template = Template::load_from_reference(&config.template)?;
        template.render(self, config)?;

        println!("Done");
        Ok(())
    }

    #[cfg(not(feature = "html"))]
    pub fn generate_html(&self) -> Result<()> {
        Err(MargoError::NoHtml)
    }

    /// Save the Margo config file.
    fn save_config(&self) -> Result<()> {
        // Save Margo config.
        println!(
            "Writing registry configuration to {name}",
            name = MARGO_CONFIG_FILE_NAME
        );
        let config_toml_path = self.path.join(MARGO_CONFIG_FILE_NAME);
        let config_toml = toml::to_string(&MargoConfig::from(self.config.clone()))?;

        fs::write(&config_toml_path, config_toml)
            .context("Can't write margo-config.toml to registry directory.")?;

        // Save Margo index.
        println!(
            "Writing registry index to {name}",
            name = MARGO_INDEX_FILE_NAME
        );
        let index_json_path = self.path.join(MARGO_INDEX_FILE_NAME);
        let index_json = serde_json::to_string_pretty(&self.index)?;

        fs::write(&index_json_path, &index_json)
            .context("Can't write margo-index.json to registry directory.")?;

        Ok(())
    }

    /// Calculate the containing directory of the index file for a given package name.
    pub(super) fn index_dir_for(&self, name: &PackageName) -> PathBuf {
        self.path.join_prefix_directories(name)
    }

    /// Calculate the full path of the index file for a given package name.
    pub(super) fn index_file_path_for(&self, name: &PackageName) -> PathBuf {
        self.index_dir_for(name).join(name.as_str())
    }

    /// The directory containing the crates in this registry.
    fn crate_dir(&self) -> PathBuf {
        self.path.join(CRATES_DIR_NAME)
    }

    /// Calculate the containing directory for the .crate file for a given package name.
    fn crate_dir_for(&self, name: &PackageName) -> PathBuf {
        self.crate_dir()
            .join_prefix_directories(name)
            .join(name.as_str())
    }

    /// Calculate the full path for the .crate file for a given package name and version.
    fn crate_file_path_for(&self, name: &PackageName, version: &Version) -> PathBuf {
        self.crate_dir_for(name).join(format!("{}.crate", version))
    }
}

#[cfg(test)]
mod test {
    use super::{Registry, CRATES_DIR_NAME};
    use crate::margo_config::{LatestConfig, MargoConfig};
    use assert_fs::prelude::*;
    use assert_fs::TempDir;
    use cargo_util_schemas::manifest::PackageName;
    use predicates::prelude::*;
    use registry_conformance::{Crate, ScratchSpace};
    use semver::Version;
    use std::str::FromStr;
    use tokio::fs;

    #[test]
    fn paths() {
        let temp_dir = TempDir::with_prefix("margo-tests-").unwrap();
        let config = MargoConfig::default().into_latest();
        let registry = Registry::initialise(&temp_dir, config).unwrap();

        let crates_dir = temp_dir.join(CRATES_DIR_NAME);

        assert_eq!(
            registry.path(),
            temp_dir.path(),
            "Registry path should be the given directory"
        );
        assert_eq!(
            registry.crate_dir(),
            crates_dir,
            "Crates dir should be inside the given directory"
        );

        let name = PackageName::from_str("margo-test-package").unwrap();
        let version = Version::new(1, 0, 0);

        assert_eq!(
            registry.index_dir_for(&name),
            temp_dir.join("ma/rg"),
            "Index dir should be ma/rg"
        );
        assert_eq!(
            registry.index_file_path_for(&name),
            temp_dir.join("ma/rg/margo-test-package"),
            "Index file path should be ma/rg/margo-test-package"
        );

        assert_eq!(
            registry.crate_dir_for(&name),
            crates_dir.join("ma/rg/margo-test-package"),
            "Crate dir should be ma/rg/margo-test-package"
        );
        assert_eq!(
            registry.crate_file_path_for(&name, &version),
            crates_dir.join("ma/rg/margo-test-package/1.0.0.crate"),
            "Crate file path should be ma/rg/margo-test-package/1.0.0.crate"
        );
    }

    /// [Registry::initialise] should create the given directory and basic configuration files.
    #[test]
    fn initialise() {
        use predicate::path::{exists, is_dir, is_file};
        use predicate::str::is_empty;

        let temp_dir = TempDir::with_prefix("margo-tests-").unwrap();

        let registry_dir = temp_dir.child("registry");
        let config_json = registry_dir.child("config.json");
        let margo_config_toml = registry_dir.child("margo-config.toml");
        let index_html = registry_dir.child("index.html");

        let config = MargoConfig::default().into_latest();
        Registry::initialise(&registry_dir, config).unwrap();

        registry_dir.assert(exists().name("registry path should exist"));
        registry_dir.assert(is_dir().name("registry path should be a directory"));

        config_json.assert(exists().name("config.json should exist"));
        config_json.assert(is_file().name("config.json should be a file"));
        config_json.assert(is_empty().not().name("config.json should not be empty"));

        margo_config_toml.assert(exists().name("margo-config.toml should exist"));
        margo_config_toml.assert(is_file().name("margo-config.toml should be a file"));
        margo_config_toml.assert(
            is_empty()
                .not()
                .name("margo-config.toml should not be empty"),
        );

        index_html.assert(exists().name("index.html should exist"));
        index_html.assert(is_file().name("index.html should be a file"));
        index_html.assert(is_empty().not().name("index.html should not be empty"));
    }

    /// [Registry::initialise] should not create an index.html if the index option is set to None.
    #[test]
    fn initialise_index_none() {
        use predicate::path::exists;

        let temp_dir = TempDir::with_prefix("margo-tests-").unwrap();

        let registry_dir = temp_dir.child("registry");
        let index_html = registry_dir.child("index.html");

        let config = LatestConfig {
            html: None,
            ..MargoConfig::default().into_latest()
        };
        Registry::initialise(&registry_dir, config).unwrap();

        index_html.assert(exists().not().name("index.html should not exist"));
    }

    /// [Registry::add] should add a crate to the registry from a file.
    #[tokio::test]
    async fn add() {
        use predicate::path::{exists, is_file};
        use predicate::str::is_empty;

        let temp_dir = TempDir::with_prefix("margo-tests-").unwrap();

        let config = MargoConfig::default().into_latest();
        let registry = Registry::initialise(&temp_dir, config).unwrap();

        let name = PackageName::from_str("margo-test-package").unwrap();
        let version = Version::new(1, 0, 0);

        let scratch = ScratchSpace::new().await.unwrap();
        let test_crate = Crate::new(name.as_str(), version.to_string())
            .lib_rs(r#"pub const ID: u8 = 1;"#)
            .create_in(&scratch)
            .await
            .unwrap();

        let crate_path = test_crate.package().await.unwrap();

        assert!(
            registry.add(crate_path).is_ok(),
            "Should be able to add the crate"
        );

        let index_file = temp_dir.child(registry.index_file_path_for(&name));
        let crate_file = temp_dir.child(registry.crate_file_path_for(&name, &version));

        index_file.assert(exists().name("Index should exist"));
        index_file.assert(is_file().name("Index should be a file"));
        index_file.assert(is_empty().not().name("Index should not be empty"));

        crate_file.assert(exists().name("Crate should exist"));
        crate_file.assert(is_file().name("Crate should be a file"));
    }

    #[tokio::test]
    async fn ading_duplicate_crate() {
        let scratch = ScratchSpace::new().await.unwrap();

        let config = MargoConfig::default().into_latest();
        let registry = Registry::initialise(scratch.registry(), config).unwrap();

        let test_crate = Crate::new("duplicated", "1.0.0")
            .lib_rs(r#"pub const ID: u8 = 1;"#)
            .create_in(&scratch)
            .await
            .unwrap();

        let crate_path = test_crate.package().await.unwrap();

        assert!(
            registry.add(&crate_path).is_ok(),
            "Should be able to add the crate"
        );
        assert!(
            registry.add(&crate_path).is_err(),
            "Should not be able to add the crate a second time"
        );

        let name = PackageName::from_str(test_crate.name()).unwrap();

        let index_file_path = registry.index_file_path_for(&name);
        let index_contents = fs::read_to_string(index_file_path).await.unwrap();

        assert_eq!(1, index_contents.lines().count());
    }

    #[tokio::test]
    async fn removing_a_crate_deletes_from_disk() {
        let scratch = ScratchSpace::new().await.unwrap();

        let config = MargoConfig::default().into_latest();

        let registry = Registry::initialise(scratch.registry(), config).unwrap();

        let name = "to-go-away";
        let version = "1.0.0";

        let test_crate = Crate::new(name, version)
            .lib_rs(r#"pub const ID: u8 = 1;"#)
            .create_in(&scratch)
            .await
            .unwrap();

        let original_crate_path = test_crate.package().await.unwrap();

        let name = name.parse().unwrap();
        let version = version.parse().unwrap();
        let target_crate_path = registry.crate_file_path_for(&name, &version);

        registry.add(original_crate_path).unwrap();

        assert!(
            target_crate_path.exists(),
            "The crate file should be in the registry at {}",
            target_crate_path.display(),
        );

        registry.remove(&name, &version).unwrap();

        assert!(
            !target_crate_path.exists(),
            "The crate file should not be in the registry at {}",
            target_crate_path.display(),
        );
    }
}
