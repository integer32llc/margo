use notify::{RecursiveMode, Watcher};
use quote::quote;
use regex::Regex;
use snafu::prelude::*;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc,
    thread,
    time::Duration,
};
use toml_edit::{DocumentMut, Item};

/// Build tools for Margo
#[derive(Debug, argh::FromArgs)]
struct Args {
    #[argh(subcommand)]
    subcommand: Subcommand,
}

#[derive(Debug, argh::FromArgs)]
#[argh(subcommand)]
enum Subcommand {
    Assets(AssetsArgs),
    PrepareRelease(PrepareReleaseArgs),
}

/// Manage assets
#[derive(Debug, argh::FromArgs)]
#[argh(subcommand)]
#[argh(name = "assets")]
struct AssetsArgs {
    /// rebuild assets as they change
    #[argh(switch)]
    watch: bool,
}

/// Prepare a release
#[derive(Debug, argh::FromArgs)]
#[argh(subcommand)]
#[argh(name = "prepare-release")]
struct PrepareReleaseArgs {
    #[argh(positional)]
    tag: String,
}

#[snafu::report]
fn main() -> Result<(), Error> {
    let args: Args = argh::from_env();

    match args.subcommand {
        Subcommand::Assets(args) => do_assets(args)?,
        Subcommand::PrepareRelease(args) => do_prepare_release(args)?,
    }

    Ok(())
}

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(transparent)]
    Assets { source: AssetsError },

    #[snafu(transparent)]
    PrepareRelease { source: PrepareReleaseError },
}

fn do_assets(args: AssetsArgs) -> Result<(), AssetsError> {
    use assets_error::*;

    let root = env::var("CARGO_MANIFEST_DIR").context(CargoManifestSnafu)?;
    let mut root = PathBuf::from(root);
    root.pop(); // Exit the `xtask` directory

    let asset_root = join!(&root, "ui", "dist");
    let asset_index = join!(&asset_root, "ui.html");

    pnpm!("install")?;

    if args.watch {
        do_assets_watch(root, asset_root, asset_index)?;
    } else {
        do_assets_once(root, asset_root, asset_index)?;
    }

    Ok(())
}

#[derive(Debug, Snafu)]
#[snafu(module)]
enum AssetsError {
    #[snafu(display("`CARGO_MANIFEST_DIR` must be set"))]
    CargoManifest { source: env::VarError },

    #[snafu(display("Could not install JS dependencies"))]
    #[snafu(context(false))]
    PnpmInstall { source: PnpmError },

    #[snafu(transparent)]
    Watch { source: AssetsWatchError },

    #[snafu(transparent)]
    Once { source: AssetsOnceError },
}

fn do_assets_watch(
    root: PathBuf,
    asset_root: PathBuf,
    asset_index: PathBuf,
) -> Result<(), AssetsWatchError> {
    use assets_watch_error::*;

    // The directory needs to exist before we can watch it.
    std::fs::create_dir_all(&asset_root)
        .context(AssetDirectoryCreateSnafu { path: &asset_root })?;

    let (tx, rx) = mpsc::channel();

    let mut watcher = notify::recommended_watcher(move |evt: notify::Result<notify::Event>| {
        if let Ok(evt) = evt {
            if evt.paths.iter().any(|p| is_asset_file(p).unwrap_or(false)) {
                let _ = tx.send(());
            }
        }
    })
    .context(WatcherCreateSnafu)?;

    watcher
        .watch(&asset_root, RecursiveMode::NonRecursive)
        .context(WatcherWatchSnafu)?;

    // Debounce notifications
    thread::spawn(move || -> Result<(), AssetsWatchError> {
        loop {
            recv_debounced(&rx)?;
            rebuild_asset_file(&root, &asset_root, &asset_index)?;
        }
    });

    pnpm!("watch")?;

    Ok(())
}

#[derive(Debug, Snafu)]
#[snafu(module)]
enum AssetsWatchError {
    #[snafu(display("Could not create the asset directory"))]
    AssetDirectoryCreate { source: io::Error, path: PathBuf },

    #[snafu(display("Could not create the filesystem watcher"))]
    WatcherCreate { source: notify::Error },

    #[snafu(display("Could not watch the asset directory"))]
    WatcherWatch { source: notify::Error },

    #[snafu(display("Event channel receiver closed unexpectedly"))]
    #[snafu(context(false))]
    RxClosed { source: mpsc::RecvError },

    #[snafu(transparent)]
    Rebuild { source: RebuildAssetFileError },

    #[snafu(display("Could not watch assets"))]
    #[snafu(context(false))]
    PnpmWatch { source: PnpmError },
}

fn is_asset_file(p: &Path) -> Option<bool> {
    let fname = p.file_name()?;
    let fname = Path::new(fname);
    let ext = fname.extension()?;

    let matched = if ext == "js" || ext == "css" || ext == "html" {
        true
    } else if ext == "map" {
        let stem = fname.file_stem()?;
        let stem = Path::new(stem);
        let ext = stem.extension()?;

        ext == "js" || ext == "css" || ext == "html"
    } else {
        false
    };

    Some(matched)
}

fn recv_debounced(rx: &mpsc::Receiver<()>) -> Result<(), mpsc::RecvError> {
    // Wait for an initial event
    rx.recv()?;

    loop {
        // Wait for subsequent events to stop coming in
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(()) => continue,
            Err(mpsc::RecvTimeoutError::Timeout) => return Ok(()),
            _ => return Err(mpsc::RecvError),
        };
    }
}

fn do_assets_once(
    root: PathBuf,
    asset_root: PathBuf,
    asset_index: PathBuf,
) -> Result<(), AssetsOnceError> {
    pnpm!("build")?;
    rebuild_asset_file(&root, &asset_root, &asset_index)?;

    Ok(())
}

#[derive(Debug, Snafu)]
#[snafu(module)]
enum AssetsOnceError {
    #[snafu(display("Could not build assets"))]
    #[snafu(context(false))]
    PnpmBuild { source: PnpmError },

    #[snafu(transparent)]
    Rebuild { source: RebuildAssetFileError },
}

fn rebuild_asset_file(
    root: &Path,
    asset_root: &Path,
    asset_index: &Path,
) -> Result<(), RebuildAssetFileError> {
    use rebuild_asset_file_error::*;

    let entry =
        fs::read_to_string(asset_index).context(ReadEntrypointSnafu { path: asset_index })?;

    let (css_name, css, css_map) = extract_asset(&entry, asset_root, {
        r#"href="assets/(ui.[a-zA-Z0-9]+.css)""#
    })
    .context(ExtractCssSnafu)?;

    let (js_name, js, js_map) = extract_asset(&entry, asset_root, {
        r#"src="assets/(ui.[a-zA-Z0-9]+.js)""#
    })
    .context(ExtractJsSnafu)?;

    let html_dir = join!(root, "src", "html");
    fs::create_dir_all(&html_dir).context(CreateHtmlDirSnafu { path: &html_dir })?;

    let asset_src = quote! {
        pub const INDEX: &str = #entry;

        pub const CSS_NAME: &str = #css_name;
        pub const CSS: &str = #css;
        pub const CSS_MAP: &str = #css_map;

        pub const JS_NAME: &str = #js_name;
        pub const JS: &str = #js;
        pub const JS_MAP: &str = #js_map;
    };

    let out_path = join!(html_dir, "assets.rs");
    fs::write(&out_path, asset_src.to_string()).context(WriteAssetFileSnafu { path: out_path })?;

    Ok(())
}

#[derive(Debug, Snafu)]
#[snafu(module)]
enum RebuildAssetFileError {
    #[snafu(display("Could not read the UI entrypoint from `{}`", path.display()))]
    ReadEntrypoint { source: io::Error, path: PathBuf },

    #[snafu(display("Could not extract the CSS filename"))]
    ExtractCss { source: ExtractAssetError },

    #[snafu(display("Could not extract the JS filename"))]
    ExtractJs { source: ExtractAssetError },

    #[snafu(display("Could not create the HTML assets directory `{}`", path.display()))]
    CreateHtmlDir { source: io::Error, path: PathBuf },

    #[snafu(display("Could not write HTML assets file `{}`", path.display()))]
    WriteAssetFile { source: io::Error, path: PathBuf },
}

fn extract_asset<'a>(
    entry: &'a str,
    asset_root: &Path,
    re: &str,
) -> Result<(&'a str, String, String), ExtractAssetError> {
    use extract_asset_error::*;

    let find_asset = Regex::new(re)?;
    let (_, [asset_name]) = find_asset
        .captures(entry)
        .context(AssetMissingSnafu)?
        .extract();

    let asset = join!(&asset_root, asset_name);
    let asset_map = {
        let mut a = asset.clone();
        a.as_mut_os_string().push(".map");
        a
    };

    let asset = fs::read_to_string(&asset).context(ReadAssetSnafu { path: asset })?;
    let asset_map =
        fs::read_to_string(&asset_map).context(ReadAssetMapSnafu { path: asset_map })?;

    Ok((asset_name, asset, asset_map))
}

#[derive(Debug, Snafu)]
#[snafu(module)]
enum ExtractAssetError {
    #[snafu(display("Invalid asset regex"))]
    #[snafu(context(false))]
    Regex { source: regex::Error },

    #[snafu(display("Could not find asset"))]
    AssetMissing,

    #[snafu(display("Could not read the asset from `{}`", path.display()))]
    ReadAsset { source: io::Error, path: PathBuf },

    #[snafu(display("Could not read the asset sourcemap from `{}`", path.display()))]
    ReadAssetMap { source: io::Error, path: PathBuf },
}

macro_rules! join {
    ($base:expr, $($c:expr),+ $(,)?) => {{
        let mut base = PathBuf::from($base);
        $(
            base.push($c);
        )*
        base
    }};
}
use join;

fn do_prepare_release(args: PrepareReleaseArgs) -> Result<(), PrepareReleaseError> {
    use prepare_release_error::*;

    let PrepareReleaseArgs { tag } = args;

    do_assets(AssetsArgs { watch: false })?;

    const ASSET_FILE: &str = "src/html/assets.rs";
    const CARGO_TOML_FILE: &str = "Cargo.toml";
    const CARGO_LOCK_FILE: &str = "Cargo.lock";

    let add_msg = format!("Commit assets for release {tag}");
    let update_msg = format!("Release {tag}");
    let rm_msg = format!("Remove assets for release {tag}");

    git!("add", "--force", ASSET_FILE).context(AssetAddSnafu)?;
    git!("commit", "--message", add_msg).context(AssetAddCommitSnafu)?;

    set_version(CARGO_TOML_FILE, &tag)?;
    cargo!("update", "margo").context(VersionLockUpdateSnafu)?;
    git!("add", CARGO_TOML_FILE, CARGO_LOCK_FILE).context(VersionAddSnafu)?;
    git!("commit", "--message", update_msg).context(VersionCommitSnafu)?;
    git!("tag", tag).context(VersionTagSnafu)?;

    git!("rm", ASSET_FILE).context(AssetRmSnafu)?;
    git!("commit", "--message", rm_msg).context(AssetRmCommitSnafu)?;

    Ok(())
}

#[derive(Debug, Snafu)]
#[snafu(module)]
enum PrepareReleaseError {
    #[snafu(transparent)]
    AssetBuild { source: AssetsError },

    #[snafu(display("Could not add the asset file to git"))]
    AssetAdd { source: GitError },

    #[snafu(display("Could not commit the asset file addition to git"))]
    AssetAddCommit { source: GitError },

    #[snafu(transparent)]
    VersionSet { source: SetVersionError },

    #[snafu(display("Could not update Cargo.lock"))]
    VersionLockUpdate { source: CargoError },

    #[snafu(display("Could not add Cargo.toml and Cargo.lock to git"))]
    VersionAdd { source: GitError },

    #[snafu(display("Could not commit Cargo.toml and Cargo.lock to git"))]
    VersionCommit { source: GitError },

    #[snafu(display("Could not tag the release commit in git"))]
    VersionTag { source: GitError },

    #[snafu(display("Could not remove the asset file from git"))]
    AssetRm { source: GitError },

    #[snafu(display("Could not commit the asset file removal to git"))]
    AssetRmCommit { source: GitError },
}

fn set_version(fname: impl AsRef<Path>, version: &str) -> Result<(), SetVersionError> {
    use set_version_error::*;

    let fname = fname.as_ref();

    let cargo_toml = fs::read_to_string(fname).context(ReadSnafu)?;
    let mut cargo_toml: DocumentMut = cargo_toml.parse().context(ParseSnafu)?;

    *cargo_toml
        .get_mut("package")
        .context(PackageSnafu)?
        .get_mut("version")
        .context(VersionSnafu)? = Item::Value(version.into());

    let cargo_toml = cargo_toml.to_string();
    fs::write(fname, cargo_toml).context(WriteSnafu)?;

    Ok(())
}

#[derive(Debug, Snafu)]
#[snafu(module)]
enum SetVersionError {
    #[snafu(display("Could not read the file"))]
    Read { source: io::Error },

    #[snafu(display("Could not parse the file"))]
    Parse { source: toml_edit::TomlError },

    #[snafu(display("The file did not contain a package table"))]
    Package,

    #[snafu(display("The file did not contain a version field"))]
    Version,

    #[snafu(display("Could not write the file"))]
    Write { source: io::Error },
}

macro_rules! pnpm {
    ($cmd:expr $(, $arg:expr)* $(,)?) => {
        command!("pnpm", $cmd $(, $arg)*).map_err(PnpmError::from)
    };
}
use pnpm;

#[derive(Debug, Snafu)]
#[snafu(display("Executing `pnpm` failed"))]
#[snafu(context(false))]
struct PnpmError {
    source: ProcessError,
}

macro_rules! git {
    ($cmd:expr $(, $arg:expr)* $(,)?) => {
        command!("git", $cmd $(, $arg)*).map_err(GitError::from)
    };
}
use git;

#[derive(Debug, Snafu)]
#[snafu(display("Executing `git` failed"))]
#[snafu(context(false))]
struct GitError {
    source: ProcessError,
}

macro_rules! cargo {
    ($cmd:expr $(, $arg:expr)* $(,)?) => {
        command!("cargo", $cmd $(, $arg)*).map_err(CargoError::from)
    };
}
use cargo;

#[derive(Debug, Snafu)]
#[snafu(display("Executing `cargo` failed"))]
#[snafu(context(false))]
struct CargoError {
    source: ProcessError,
}

macro_rules! command {
    ($cmd:expr $(, $arg:expr)* $(,)?) => {
        (|| -> Result<(), ProcessError> {
            use process_error::*;

            let mut cmd = Command::new($cmd);
            $(
                cmd.arg($arg);
            )*

            let status = cmd.status()
               .context(SpawnSnafu)?;

            ensure!(status.success(), SuccessSnafu);

            Ok(())
        })()
    };
}
use command;

#[derive(Debug, Snafu)]
#[snafu(module)]
enum ProcessError {
    #[snafu(display("Could not start the process"))]
    Spawn { source: io::Error },

    #[snafu(display("The process did not succeed"))]
    Success,
}
