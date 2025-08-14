// use snafu::Snafu;
// use std::backtrace::Backtrace;
// use std::io;
// use std::path::PathBuf;
//
// #[derive(Debug, Snafu)]
// #[snafu(visibility(pub))]
// pub enum RegistryError {
//     #[snafu(display("Couldn't create directory {}.", path.display()))]
//     CreateDir { source: io::Error, path: PathBuf },
//
//     #[snafu(display("Missing Margo configuration file."))]
//     MissingMargoConfig { source: io::Error },
//
//     #[snafu(display("Error while parsing the Margo configuration file."))]
//     ParseMargoConfig { source: toml::de::Error },
//
//     #[snafu(display("Error while serialising the Margo configuration."))]
//     SerialiseMargoConfig { source: toml::ser::Error },
//
//     #[snafu(display("Error while saving the Margo configuration file."))]
//     WriteMargoConfig { source: io::Error },
//
//     #[snafu(display("Error while serialising the registry config.json file."))]
//     SerialiseConfigJson { source: serde_json::Error },
//
//     #[snafu(display("Error while saving the registry config.json file."))]
//     WriteConfigJson { source: io::Error },
//
//     #[snafu(display("Error while reading the index file at {}.", path.display()))]
//     ReadIndex { source: io::Error, path: PathBuf },
//
//     #[snafu(display("Error while parsing the index file at {} (line {}).", path.display(), line))]
//     ParseIndex {
//         source: serde_json::Error,
//         path: PathBuf,
//         line: usize,
//     },
//
//     #[snafu(display("Couldn't modify the index at {} for version {}", path.display(), version))]
//     ModifyIndex {
//         path: PathBuf,
//         version: String,
//         backtrace: Backtrace,
//     },
//
//     #[snafu(display("Error while serialising the index file at {} (line {}).", path.display(), line))]
//     SerialiseIndex {
//         source: serde_json::Error,
//         path: PathBuf,
//         line: usize,
//     },
//
//     #[snafu(display("Error while saving the index file at {}.", path.display()))]
//     SaveIndex { source: io::Error, path: PathBuf },
//
//     #[snafu(display("Couldn't find or open the crate package {}.", path.display()))]
//     ReadCrate { source: io::Error, path: PathBuf },
//
//     #[snafu(display("Error while trying to read the contents of the crate package {}.", path.display()))]
//     ParseCrate { source: io::Error, path: PathBuf },
//
//     #[snafu(display("The crate package {} is invalid. Make sure it contains at least a Cargo.toml.", path.display()))]
//     InvalidCrate { path: PathBuf, backtrace: Backtrace },
//
//     #[snafu(display("Error while parsing the Cargo.toml inside the crate package {}.", path.display()))]
//     ParseCrateToml {
//         source: toml::de::Error,
//         path: PathBuf,
//     },
//
//     #[snafu(display("Missing or invalid data in the Cargo.toml inside the crate package {}.", path.display()))]
//     InvalidCrateToml { path: PathBuf, backtrace: Backtrace },
//
//     #[snafu(display("Error while writing the crate package to {}.", path.display()))]
//     WriteCrate { source: io::Error, path: PathBuf },
//
//     #[snafu(display("Error while deleting the file at {}.", path.display()))]
//     DeleteFile { source: io::Error, path: PathBuf },
//
//     #[snafu(display("Invalid version."))]
//     InvalidVersion { source: semver::Error },
//
//     #[snafu(display("Invalid URL."))]
//     InvalidUrl { source: url::ParseError },
// }
