use super::Registry;
use crate::prelude::*;
use cargo_util_schemas::manifest::{FeatureName, PackageName, RustVersion};
use colored::Colorize;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::{fs, io};
use url::Url;

/// A registry index file, representing a series of version entries relating to a crate.
pub struct Index {
    pub name: PackageName,
    pub path: PathBuf,
    pub entries: BTreeMap<Version, IndexEntry>,
}

impl Index {
    pub fn latest_non_yanked_version(&self) -> Option<&Version> {
        self.entries.values().rfind(|e| !e.yanked).map(|e| &e.vers)
    }

    pub fn set_yanked(&mut self, version: &Version, yanked: bool) -> Result<()> {
        let entry = self.entries.get_mut(version).context(format!(
            "Version {version} doesn't exist in the index for this crate.",
            version = version.to_string(),
        ))?;

        entry.yanked = yanked;

        Ok(())
    }

    pub fn add(&mut self, entry: IndexEntry) {
        self.entries.insert(entry.vers.clone(), entry);
    }

    pub fn remove(&mut self, version: &Version) {
        if self.entries.remove(version).is_none() {
            println!(
                "Version {version} doesn't exist in index. Nothing changed.",
                version = version.to_string().magenta().bold(),
            );
        }
    }

    pub fn contains_version(&self, version: &Version) -> bool {
        self.entries.contains_key(version)
    }

    /// Open the index file for a package, or create a new empty index if
    /// no index file exists for the given package name.
    pub fn open_or_new(name: PackageName, registry: &Registry) -> Result<Self> {
        println!("Opening index for {name}", name = name.cyan());

        fs::create_dir_all(&registry.index_dir_for(&name))?;
        let path = registry.index_file_path_for(&name);

        Self::open_or_new_in_path(name, path)
    }

    /// Open the index file at the given path, or create an empty index if no such file exists.
    fn open_or_new_in_path(name: PackageName, path: PathBuf) -> Result<Self> {
        let index_file = match File::open(&path) {
            Ok(f) => BufReader::new(f),
            // If the index file doesn't exist, return an empty index - no further parsing necessary.
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(Self {
                    name,
                    path,
                    entries: BTreeMap::new(),
                })
            }
            Err(e) => Err(e).context(format!(
                "Can't open index file for {name} ({path}).",
                path = path.display()
            ))?,
        };

        let mut entries = BTreeMap::new();

        for (line, json) in index_file.lines().enumerate() {
            let json = json.context(format!(
                "Can't read next line #{line} in index file for {name} ({path}).",
                path = path.display()
            ))?;
            let entry = serde_json::from_str::<IndexEntry>(&json).context(format!(
                "Invalid JSON on line #{line} in index for {name} ({path}).",
                path = path.display()
            ))?;

            entries.insert(entry.vers.clone(), entry);
        }

        Ok(Self {
            name,
            path,
            entries,
        })
    }

    /// Write the entries of this index file to disk.
    ///
    /// **Caution**: this will replace any existing contents of the index file.
    pub fn save(&self) -> Result<()> {
        println!("Saving index for {name}", name = self.name.cyan());

        let path = &self.path;

        if self.entries.is_empty() {
            if path.exists() {
                fs::remove_file(path).context(format!(
                    "Can't delete empty index file at {}.",
                    path.display()
                ))?;
            }
            path.remove_dirs_if_empty()?;
        } else {
            let file = File::create(path)
                .context(format!("Can't create index file at {}.", path.display()))?;
            let mut file = BufWriter::new(file);

            for (line, entry) in self.entries.values().enumerate() {
                serde_json::to_writer(&mut file, entry).context(format!(
                    "Can't write line #{line} to index file at {}.",
                    path.display()
                ))?;
                file.write_all(b"\n").context(format!(
                    "Can't write EOL at line #{line} to index file at {}.",
                    path.display()
                ))?;
            }

            file.flush().context("Can't write index file.")?;
            println!("Wrote crate index to `{}`", path.display());
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct IndexEntry {
    /// The name of the package.
    pub name: PackageName,

    /// The version of the package this entry is describing.
    ///
    /// This must be a valid version according to the
    /// Semantic Versioning 2.0.0 spec at https://semver.org/.
    pub vers: Version,

    /// Direct dependencies of the package.
    pub deps: Vec<Dependency>,

    /// A SHA256 checksum of the `.crate` file.
    pub cksum: String,

    /// Set of features defined for the package.
    ///
    /// Each feature maps to features or dependencies it enables.
    pub features: BTreeMap<FeatureName, Vec<String>>,

    /// Boolean of whether this version has been yanked.
    pub yanked: bool,

    /// The `links` value from the package's manifest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<String>,

    /// The schema version of this entry.
    ///
    /// If this is not specified, it should be interpreted as the default of 1.
    ///
    /// Cargo (starting with version 1.51) will ignore versions it does not
    /// recognise. This provides a method to safely introduce changes to index
    /// entries and allow older versions of cargo to ignore newer entries it
    /// doesn't understand. Versions older than 1.51 ignore this field, and
    /// thus may misinterpret the meaning of the index entry.
    ///
    /// The current values are:
    ///
    /// * 1: The schema as documented here, not including newer additions.
    ///   This is honoured in Rust version 1.51 and newer.
    /// * 2: The addition of the `features2` field.
    ///   This is honoured in Rust version 1.60 and newer.
    pub v: u32,

    /// Features with new, extended syntax, such as namespaced
    /// features (`dep:`) and weak dependencies (`pkg?/feat`).
    ///
    /// This is separated from `features` because versions older than 1.19
    /// will fail to load due to not being able to parse the new syntax, even
    /// with a `Cargo.lock` file.
    ///
    /// Cargo will merge any values listed here with the "features" field.
    ///
    /// If this field is included, the "v" field should be set to at least 2.
    ///
    /// Registries are not required to use this field for extended feature
    /// syntax, they are allowed to include those in the "features" field.
    /// Using this is only necessary if the registry wants to support cargo
    /// versions older than 1.19, which in practice is only crates.io since
    /// those older versions do not support other registries.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub features2: BTreeMap<String, Vec<String>>,

    /// The minimal supported Rust version
    ///
    /// This must be a valid version requirement without an operator (e.g. no `=`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rust_version: Option<RustVersion>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Dependency {
    /// Name of the dependency.
    ///
    /// If the dependency is renamed from the original package
    /// name, this is the new name. The original package name is
    /// stored in the `package` field.
    pub name: PackageName,

    /// The SemVer requirement for this dependency.
    ///
    /// This must be a valid version requirement defined at
    /// https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html.
    pub req: VersionReq,

    /// Features enabled for this dependency.
    pub features: Vec<FeatureName>,

    /// Whether this is an optional dependency.
    pub optional: bool,

    /// Whether default features are enabled.
    pub default_features: bool,

    /// The target platform for the dependency.
    ///
    /// A string such as `cfg(windows)`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,

    /// The dependency kind.
    ///
    /// Note: this is a required field, but a small number of entries
    /// exist in the crates.io index with either a missing or null
    /// `kind` field due to implementation bugs.
    pub kind: DependencyKind,

    /// The URL of the index of the registry where this dependency
    /// is from.
    ///
    /// If not specified or null, it is assumed the dependency is
    /// in the current registry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<Url>,

    /// If the dependency is renamed, this is the actual package
    /// name.
    ///
    /// If not specified or null, this dependency is not renamed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<PackageName>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    #[allow(unused)]
    // Stored in the index but not used by Cargo
    Dev,
    Build,
    Normal,
}

#[cfg(test)]
mod test {
    use super::{Index, IndexEntry};
    use assert_fs::prelude::*;
    use assert_fs::TempDir;
    use cargo_util_schemas::manifest::PackageName;
    use predicates::prelude::*;
    use semver::Version;
    use std::str::FromStr;

    fn name() -> PackageName {
        PackageName::from_str("test_package").unwrap()
    }

    fn test_index_entry(major: u64, minor: u64, patch: u64) -> IndexEntry {
        IndexEntry {
            name: name(),
            vers: Version::new(major, minor, patch),
            deps: vec![],
            cksum: "".to_string(),
            features: Default::default(),
            yanked: false,
            links: None,
            v: 0,
            features2: Default::default(),
            rust_version: None,
        }
    }

    #[test]
    fn open_or_new() {
        let dir = TempDir::new().unwrap();
        let index_file = dir.child("index");

        let mut index_a = Index::open_or_new_in_path(name(), index_file.to_path_buf()).unwrap();
        index_a.add(test_index_entry(1, 0, 0));
        index_a.save().unwrap();

        let index_b = Index::open_or_new_in_path(name(), index_file.to_path_buf()).unwrap();
        assert_eq!(
            index_a.entries, index_b.entries,
            "indexes should contain the same entries"
        );
    }

    #[test]
    fn save() {
        let dir = TempDir::new().unwrap();
        dir.child(".keep").touch().unwrap();

        let index_file = dir.child("index");
        index_file.assert(predicate::path::missing().name("index file should not exist yet"));

        let mut index = Index::open_or_new_in_path(name(), index_file.to_path_buf()).unwrap();

        // Add 1 version
        index.add(test_index_entry(1, 0, 0));
        index.save().unwrap();

        index_file.assert(predicate::path::exists().name("index file should be created"));
        index_file.assert(
            predicate::str::contains('\n')
                .count(1)
                .name("index file should contain 1 line"),
        );
        index_file
            .assert(predicate::str::ends_with('\n').name("index file should end with newline"));

        // Add a 2nd version
        index.add(test_index_entry(1, 0, 1));
        index.save().unwrap();

        index_file.assert(
            predicate::str::contains('\n')
                .count(2)
                .name("index file should contain 2 lines"),
        );
        index_file
            .assert(predicate::str::ends_with('\n').name("index file should end with newline"));

        // Remove both versions
        index.remove(&Version::new(1, 0, 0));
        index.remove(&Version::new(1, 0, 1));
        index.save().unwrap();

        index_file.assert(predicate::path::missing().name("index file should be removed"));
    }

    #[test]
    fn add() {
        let dir = TempDir::new().unwrap();
        let index_file = dir.child("index");

        let mut index = Index::open_or_new_in_path(name(), index_file.to_path_buf()).unwrap();
        index.add(test_index_entry(1, 0, 0)).unwrap();
        assert_eq!(index.entries.len(), 1, "should contain 1 entry");

        assert!(index.add(test_index_entry(1, 0, 0)).is_err(), "should error when adding a duplicate version");
        assert_eq!(index.entries.len(), 1, "should still contain 1 entry");

        index.add(test_index_entry(1, 2, 0)).unwrap();
        assert_eq!(index.entries.len(), 2, "should contain 2 entries");
    }

    #[test]
    fn remove() {
        let dir = TempDir::new().unwrap();
        let index_file = dir.child("index");

        let mut index = Index::open_or_new_in_path(name(), index_file.to_path_buf()).unwrap();
        index.add(test_index_entry(1, 0, 0));
        index.add(test_index_entry(1, 1, 0));
        index.add(test_index_entry(1, 1, 1));
        index.save().unwrap();

        assert_eq!(index.entries.len(), 3, "index should contain 3 versions");

        index.remove(&Version::new(1, 0, 0));
        assert_eq!(
            index.entries.len(),
            2,
            "index should contain 2 versions after removing version"
        );

        index.remove(&Version::new(1, 0, 0));
        assert_eq!(
            index.entries.len(),
            2,
            "index should still contain 2 versions after removing nonexistent version"
        );
    }

    #[test]
    fn contains_version() {
        let dir = TempDir::new().unwrap();
        let index_file = dir.child("index");

        let mut index = Index::open_or_new_in_path(name(), index_file.to_path_buf()).unwrap();

        index.add(test_index_entry(1, 0, 0));

        assert!(
            index.contains_version(&Version::new(1, 0, 0)),
            "index should contain same version"
        );
        assert!(
            !index.contains_version(&Version::new(0, 1, 1)),
            "index should not contain other version"
        );
    }

    #[test]
    fn yank() {
        let dir = TempDir::new().unwrap();
        let index_file = dir.child("index");

        let mut index = Index::open_or_new_in_path(name(), index_file.to_path_buf()).unwrap();

        index.add(test_index_entry(1, 0, 0));
        index.add(test_index_entry(1, 1, 0));

        assert_eq!(
            index.latest_non_yanked_version(),
            Some(&Version::new(1, 1, 0)),
            "latest non-yanked version should return latest version"
        );

        index.set_yanked(&Version::new(1, 1, 0), true).unwrap();
        assert_eq!(
            index.latest_non_yanked_version(),
            Some(&Version::new(1, 0, 0)),
            "latest non-yanked version should return previous version"
        );
    }
}
