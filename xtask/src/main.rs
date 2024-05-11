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

#[snafu::report]
fn main() -> Result<(), Error> {
    let args: Args = argh::from_env();

    match args.subcommand {
        Subcommand::Assets(args) => do_assets(args)?,
    }

    Ok(())
}

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(transparent)]
    Assets { source: AssetsError },
}

fn do_assets(args: AssetsArgs) -> Result<(), AssetsError> {
    use assets_error::*;

    let root = env::var("CARGO_MANIFEST_DIR").context(CargoManifestSnafu)?;
    let mut root = PathBuf::from(root);
    root.pop(); // Exit the `xtask` directory

    let asset_root = join!(&root, "ui", "dist");
    let asset_index = join!(&asset_root, "ui.html");

    pnpm("install")?;

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

    pnpm("watch")?;

    Ok(())
}

#[derive(Debug, Snafu)]
#[snafu(module)]
enum AssetsWatchError {
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
    pnpm("build")?;
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

fn pnpm(subcommand: &str) -> Result<(), PnpmError> {
    use pnpm_error::*;

    let status = Command::new("pnpm")
        .arg(subcommand)
        .status()
        .context(SpawnSnafu)?;
    ensure!(status.success(), SuccessSnafu);
    Ok(())
}

#[derive(Debug, Snafu)]
#[snafu(module)]
enum PnpmError {
    #[snafu(display("Could not start the `pnpm` process"))]
    Spawn { source: io::Error },

    #[snafu(display("The `pnpm` process did not succeed"))]
    Success,
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
