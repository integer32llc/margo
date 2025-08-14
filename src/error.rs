use cargo_util_schemas::manifest::PackageName;
use semver::Version;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MargoError {
    #[cfg(not(feature = "html"))]
    #[error("Please reinstall Margo with the `html` feature enabled in order to render HTML index pages.")]
    NoHtml,

    #[error("The template archive doesn't contain an index file (index.html or index.hbs), or the index file is empty.")]
    MissingTemplateIndex,
    
    #[error("Version {1} of package {0} already exists in the index.")]
    DuplicateVersion(PackageName, Version),
}
