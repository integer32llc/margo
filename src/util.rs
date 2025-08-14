use cargo_util_schemas::manifest::PackageName;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::{fs, io};
use url::Url;

use anyhow::{Context, Result};

pub trait UrlExt {
    fn ensure_trailing_slash(&mut self);
}

impl UrlExt for Url {
    /// Make sure that the URL ends with an empty segment (i.e. a trailing slash).
    fn ensure_trailing_slash(&mut self) {
        if let Ok(mut s) = self.path_segments_mut() {
            s.pop_if_empty();
            s.push("");
        }
    }
}

pub trait PathExt {
    /// Join the prefix directories for a given package name, as specified in
    /// [https://doc.rust-lang.org/cargo/reference/registry-index.html#index-files](the cargo reference).
    fn join_prefix_directories(&self, name: &PackageName) -> PathBuf;

    /// Remove this directory and any parent directories that are empty.
    ///
    /// If the given path is not a directory, this method will fall back
    /// to its parent directory ([PathBuf::parent]).
    fn remove_dirs_if_empty(&self) -> Result<()>;
}

impl PathExt for Path {
    fn join_prefix_directories(&self, name: &PackageName) -> PathBuf {
        match name.len() {
            0 => unreachable!(),
            1 => self.join("1"),
            2 => self.join("2"),
            3 => self.join("3").join(&name[0..1]),
            _ => self.join(&name[0..2]).join(&name[2..4]),
        }
    }

    fn remove_dirs_if_empty(&self) -> Result<()> {
        if !self.exists() || !self.is_dir() {
            return match self.parent() {
                Some(dir) => dir.remove_dirs_if_empty(),
                None => Ok(()),
            };
        }

        match fs::remove_dir(self) {
            Ok(_) => match self.parent() {
                Some(dir) => dir.remove_dirs_if_empty(),
                _ => Ok(()),
            },
            Err(e) if e.kind() == io::ErrorKind::DirectoryNotEmpty => Ok(()),
            Err(e) => Err(e).context("Error while trying to remove empty directories."),
        }
    }
}

pub trait OptionExt<T> {
    fn apply_default(self, use_default: bool, value: impl Into<T>) -> Self;

    fn unwrap_or_dialog(self, arg: &str, f: impl FnOnce() -> dialoguer::Result<T>) -> Result<T>;
}

impl<T> OptionExt<T> for Option<T> {
    fn apply_default(self, use_default: bool, value: impl Into<T>) -> Self {
        if self.is_none() && use_default {
            Some(value.into())
        } else {
            self
        }
    }

    fn unwrap_or_dialog(self, arg: &str, f: impl FnOnce() -> dialoguer::Result<T>) -> Result<T> {
        match self {
            Some(v) => Ok(v),
            None => f().context(format!("Reading config value {}.", arg)),
        }
    }
}

pub trait VecExt<T> {
    fn sorted_by<F>(self, f: F) -> Self
    where
        F: FnMut(&T, &T) -> Ordering;
}

impl<T> VecExt<T> for Vec<T> {
    fn sorted_by<F>(mut self, f: F) -> Self
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        self.sort_by(f);
        self
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use assert_fs::TempDir;
    use std::str::FromStr;

    #[test]
    fn url_ext_ensure_trailing_slash() {
        let mut with = Url::parse("https://example.com/a/b/").unwrap();
        let mut without = Url::parse("https://example.com/a/b").unwrap();

        with.ensure_trailing_slash();
        without.ensure_trailing_slash();

        assert_eq!(
            with.as_str(),
            "https://example.com/a/b/",
            "URL should have a trailing slash."
        );
        assert_eq!(
            without.as_str(),
            "https://example.com/a/b/",
            "URL should have a trailing slash."
        );
    }

    #[test]
    fn path_ext_join_prefix_directories() {
        let path = Path::new("a/b/");
        let name = PackageName::from_str("helloworld").unwrap();

        let path = path.join_prefix_directories(&name);
        assert_eq!(path.as_os_str().to_str().unwrap(), "a/b/he/ll");
    }

    #[test]
    fn path_ext_remove_dirs_if_empty() {
        let dir = TempDir::new().unwrap();

        let a = dir.join("a");
        fs::create_dir_all(&a).unwrap();
        fs::write(&a.join(".keep"), "hello world").unwrap();

        let b = a.join("b");
        let c = b.join("c");
        fs::create_dir_all(&c).unwrap();

        c.remove_dirs_if_empty().unwrap();
        assert!(a.exists(), "a should still exist as it contains a file");
        assert!(!b.exists(), "b should be removed");
        assert!(!c.exists(), "c should be removed");
    }

    #[test]
    fn vec_ext_sorted_by() {
        let vec = vec![1, 5, 3];
        let sorted = vec.sorted_by(|a, b| a.cmp(&b));
        assert_eq!(sorted, vec![1, 3, 5]);
    }
}
