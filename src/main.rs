use crate::margo_config::MargoConfig;
use crate::margo_config::{LatestConfig, LatestConfigIndex};
use crate::registry::{Registry, MARGO_CONFIG_FILE_NAME};
use crate::template_reference::{BuiltInTemplate, TemplateReference};
use anyhow::bail;
use cargo_util_schemas::manifest::PackageName;
use colored::Colorize;
use dialoguer::{Confirm, Input, Select};
use semver::Version;
use std::env::current_dir;
use std::{path::PathBuf, str};
use url::Url;

use crate::prelude::*;

mod prelude {
    pub use crate::error::MargoError;
    pub use crate::util::*;
    pub use anyhow::{Context, Result};
}

mod config_json;
mod error;
mod margo_config;
mod registry;
#[cfg(feature = "html")]
mod template;
mod template_reference;
mod util;

fn main() -> Result<()> {
    let args: Args = argh::from_env();

    // Print the Margo program title and version.
    println!(
        "{title} v{version}",
        title = "Margo".yellow().bold(),
        version = env!("CARGO_PKG_VERSION"),
    );

    // Resolve and normalise the given path into an absolute path
    // in the current working directory.
    let mut registry_path = args.registry_path.clone();
    if !registry_path.is_absolute() {
        registry_path = current_dir()?.join(registry_path).canonicalize()?;
    }

    // Print the resolved registry path.
    println!(" in {}", registry_path.display().to_string().dimmed());

    match args.subcommand {
        Subcommand::Init(init) => init.handle(registry_path)?,
        Subcommand::Add(add) => add.handle(registry_path)?,
        Subcommand::Remove(rm) => rm.handle(registry_path)?,
        Subcommand::Yank(yank) => yank.handle(registry_path)?,
        Subcommand::List(list) => list.handle(registry_path)?,
        Subcommand::RebuildIndex(rebuild) => rebuild.handle(registry_path)?,
        #[cfg(feature = "html")]
        Subcommand::GenerateHtml(html) => html.handle(registry_path)?,
    }

    Ok(())
}

/// Manage a static crate registry
#[derive(Debug, argh::FromArgs)]
struct Args {
    /// path to the registry directory, defaults to the current working directory.
    #[argh(positional, default = "std::env::current_dir().unwrap()")]
    registry_path: PathBuf,

    #[argh(subcommand)]
    subcommand: Subcommand,
}

#[derive(Debug, argh::FromArgs)]
#[argh(subcommand)]
enum Subcommand {
    Init(Init),
    Add(Add),
    Remove(Remove),
    Yank(Yank),
    List(List),
    RebuildIndex(RebuildIndex),
    #[cfg(feature = "html")]
    GenerateHtml(GenerateHtml),
}

/// Initialise a new registry
#[derive(Debug, argh::FromArgs)]
#[argh(subcommand, name = "init")]
struct Init {
    /// use default values where possible, instead of prompting for them
    #[argh(switch, short = 'y')]
    use_defaults: bool,

    /// the URL that the registry is hosted at
    #[argh(option)]
    base_url: Option<Url>,

    /// require HTTP authentication to access crates
    #[argh(option)]
    auth_required: Option<bool>,

    /// whether to render an HTML index page
    #[argh(option)]
    html: Option<bool>,

    /// path to the template tarball to use, if `html` is true
    #[argh(option)]
    template: Option<TemplateReference>,

    /// title of the HTML index page, if `html` is true
    #[argh(option)]
    title: Option<String>,

    /// name you'd like to suggest other people call your registry
    #[argh(option)]
    suggested_registry_name: Option<String>,
}

impl Init {
    fn handle(self, registry_path: PathBuf) -> Result<()> {
        if registry_path.join(MARGO_CONFIG_FILE_NAME).exists() {
            bail!(
                "Can't create a new registry in {path}, it already contains a {filename} file.",
                path = registry_path.display(),
                filename = MARGO_CONFIG_FILE_NAME
            );
        }

        let defaults = MargoConfig::default().into_latest();

        // Gather configuration values
        let base_url = self.base_url.unwrap_or_dialog("base-url", || {
            Input::new()
                .with_prompt("Which URL will the registry be hosted at?")
                .interact()
        })?;

        let auth_required = self
            .auth_required
            .apply_default(self.use_defaults, defaults.auth_required)
            .unwrap_or_dialog("auth-required", || {
                Confirm::new()
                    .default(defaults.auth_required)
                    .show_default(true)
                    .with_prompt("Require HTTP authentication to access crates?")
                    .interact()
            })?;

        let html_enabled = match self.html {
            Some(i) => i,
            None if self.use_defaults => defaults.html.is_some(),
            None => Confirm::new()
                .with_prompt("Render an HTML index page for the registry?")
                .default(true)
                .interact()?,
        };

        // If the `html` feature isn't enabled, show an error if the user tries to enable the index.
        #[cfg(not(feature = "html"))]
        ensure!(has_index == false, NoHtmlSnafu);

        // Gather the index HTML configuration, if one is needed.
        let index = if html_enabled && self.use_defaults {
            defaults.html
        } else if html_enabled {
            let defaults = defaults.html.expect("Default config must have an index.");

            let template = match self.template {
                Some(t) => t,
                None => {
                    // Prompting to get the template is a two-step process. First, the user can
                    // select from one of the built-in templates or choose a custom value. If
                    // they select custom, they're then asked to provide a path.

                    let mut all = BuiltInTemplate::all().collect::<Vec<_>>();

                    let built_in = Select::new()
                        .with_prompt("Which template do you want to use for the HTML index page?")
                        .items(&all.iter().map(|t| t.name()).collect::<Vec<_>>())
                        .item("<custom>")
                        .default(match defaults.template {
                            TemplateReference::BuiltIn(b) => {
                                all.iter().position(|t| &b == t).unwrap()
                            }
                            TemplateReference::File(_) => all.len(),
                        })
                        .interact()?;

                    if built_in < all.len() {
                        TemplateReference::BuiltIn(all.swap_remove(built_in))
                    } else {
                        TemplateReference::File(
                            Input::<String>::new()
                                .with_prompt("Path to your custom template tarball")
                                .interact()?
                                .into(),
                        )
                    }
                }
            };

            let title = self
                .title
                .apply_default(self.use_defaults, &defaults.title)
                .unwrap_or_dialog("title", || {
                    Input::new()
                        .with_prompt("Title to use for the HTML index page")
                        .default(defaults.title)
                        .show_default(true)
                        .interact()
                })?;

            let suggested_registry_name = self
                .suggested_registry_name
                .apply_default(self.use_defaults, &defaults.suggested_registry_name)
                .unwrap_or_dialog("suggested-registry-name", || {
                    Input::new()
                        .with_prompt("Name you'd like to suggest other people call your registry")
                        .default(defaults.suggested_registry_name)
                        .show_default(true)
                        .interact()
                })?;

            Some(LatestConfigIndex {
                template,
                title,
                suggested_registry_name,
            })
        } else {
            None
        };

        let config = LatestConfig {
            base_url,
            auth_required,
            html: index,
        };
        let registry = Registry::initialise(registry_path, config)?;
        registry.maybe_generate_html()?;

        Ok(())
    }
}

/// Add a crate to the registry
#[derive(Debug, argh::FromArgs)]
#[argh(subcommand, name = "add")]
struct Add {
    /// path to the .crate file to add
    #[argh(positional)]
    crate_path: Vec<PathBuf>,
}

impl Add {
    fn handle(self, registry_path: PathBuf) -> Result<()> {
        let registry = Registry::open(registry_path)?;

        for path in self.crate_path {
            registry.add(path)?;
        }
        registry.maybe_generate_html()?;

        Ok(())
    }
}

/// Remove a crate from the registry
#[derive(Debug, argh::FromArgs)]
#[argh(subcommand, name = "rm")]
struct Remove {
    #[argh(positional)]
    name: PackageName,

    // FUTURE: Allow removing all versions at once?
    /// the version of the crate
    #[argh(positional)]
    version: Version,
}

impl Remove {
    fn handle(self, registry_path: PathBuf) -> Result<()> {
        let registry = Registry::open(registry_path)?;

        registry.remove(&self.name, &self.version)?;
        registry.maybe_generate_html()?;

        Ok(())
    }
}

/// Yank a version of a crate from the registry
#[derive(Debug, argh::FromArgs)]
#[argh(subcommand, name = "yank")]
struct Yank {
    /// undo a previous yank
    #[argh(switch)]
    undo: bool,

    /// the name of the crate
    #[argh(positional)]
    name: PackageName,

    /// the version of the crate
    #[argh(positional)]
    version: Version,
}

impl Yank {
    fn handle(&self, registry_path: PathBuf) -> Result<()> {
        let registry = Registry::open(registry_path)?;

        registry.yank(&self.name, &self.version, !self.undo)?;
        registry.maybe_generate_html()?;

        Ok(())
    }
}

/// List all crates and their versions in the registry
#[derive(Debug, argh::FromArgs)]
#[argh(subcommand, name = "list")]
struct List {}

impl List {
    fn handle(self, registry_path: PathBuf) -> Result<()> {
        let registry = Registry::open(registry_path)?;

        let indexes = registry.list_indexes()?;

        let packages_count = indexes.len();
        let total_versions_count = indexes
            .iter()
            .map(|i| i.entries.len())
            .reduce(|total, i| total + i)
            .unwrap_or(0);

        println!(
            "{packages} package{packages_s} in the registry ({versions} total version{versions_s}):",
            packages = packages_count.to_string().bold(),
            packages_s = if packages_count == 1 { "" } else { "s" },
            versions = total_versions_count.to_string().bold(),
            versions_s = if total_versions_count == 1 { "" } else { "s" },
        );

        for i in indexes {
            // Display the latest non-yanked version on top.
            let latest_version_str = i
                .latest_non_yanked_version()
                .map(|v| v.to_string())
                .unwrap_or_default();

            println!(" {} {}", i.name.cyan().bold(), latest_version_str);

            let indent = i.name.len();

            for entry in i.entries.values() {
                let version = entry.vers.to_string();
                if version == latest_version_str {
                    continue;
                }

                if entry.yanked {
                    println!(
                        " {indent} {version} {yanked}",
                        indent = " ".repeat(indent),
                        version = version.dimmed().strikethrough(),
                        yanked = "yanked".red().dimmed()
                    );
                } else {
                    println!(" {indent} {version}", indent = " ".repeat(indent));
                }
            }
        }

        Ok(())
    }
}

/// Rebuild the Margo index by scanning the crate files on disk.
#[derive(Debug, argh::FromArgs)]
#[argh(subcommand, name = "rebuild-index")]
struct RebuildIndex {}

impl RebuildIndex {
    fn handle(self, registry_path: PathBuf) -> Result<()> {
        let mut registry = Registry::open(registry_path)?;
        registry.refresh_index_from_disk()?;
        Ok(())
    }
}

/// Generate an HTML index for the registry
#[derive(Debug, argh::FromArgs)]
#[argh(subcommand, name = "generate-html")]
struct GenerateHtml {}

impl GenerateHtml {
    fn handle(self, registry_path: PathBuf) -> Result<()> {
        let registry = Registry::open(registry_path)?;
        registry.generate_html()?;
        Ok(())
    }
}
