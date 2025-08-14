//! This module contains all required logic to load and render templates from a .tar file.
//!
//! # Built-in templates
//! Margo ships with a few templates built-in, to help you get up and running quickly.
//!
//! - `Classic` (default): the original Margo registry template.
//!
//! # Custom templates
//! A template is a simple uncompressed tarball archive (`.tar`, not `.tar.gz`) containing all
//! necessary files required for the template to work. All files inside the template archive will
//! be copied as-is to the registry output directory, except for the index template.
//!
//! ## Index
//! The index template must be a file called `index.hbs` or `index.html`. This file must contain
//! a [handlebars](https://handlebarsjs.com/guide/) template to render the HTML for the index page.
//!
//! ### Variables
//! See [TemplateData] for all variables provided to the Handlebars template while rendering.
//!
//! ### Helpers
//! Margo adds a few Rust-specific helpers to the Handlebars renderer:
//!
//! - `len [arr]`: Return the length of an array.
//! - `eq [lhs] [rhs]`: Check if two values are equal.
//! - `ne [lhs] [rhs]`: Check if two values are not equal.
//! - `gt [lhs] [rhs]`: Check if `lhs` is greater than `rhs`.
//! - `lt [lhs] [rhs]`: Check if `lhs` is less than `rhs`.
//!
//! ### Minimal example
//! ```hbs
//! <h1>{{title}}<h1>
//!
//! <h2>Installation</h2>
//! <p>Add this registry to <code>.cargo/config.toml</code>:</p>
//! <pre>
//! [registries]
//! {{registry.suggested_name}} = { index = "sparse+{{registry.base_url}}" }
//! </pre>
//!
//! <p>Install a crate from this registry:</p>
//! <pre>
//! cargo add --registry={{registry.suggested_name}} [crate name]
//! </pre>
//!
//! <h2>Crates</h2>
//! {{#each crates}}
//!   <h3>{{name}}</h3>
//!   {{#each versions}}
//!     <p>{{this}}</p>
//!   {{/each}}
//! {{/each}}
//! ```

use crate::margo_config::LatestConfigIndex;
use crate::prelude::*;
use crate::registry::{PackagedCargoToml, Registry};
use crate::template_reference::{BuiltInTemplate, TemplateReference};
use crate::Result;

use cargo_util_schemas::manifest::{FeatureName, PackageName};
use colored::Colorize;
use comrak::nodes::{NodeLink, NodeValue};
use comrak::{format_html, parse_document, Arena, Options};
use handlebars::{handlebars_helper, Handlebars, JsonValue};
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::{Path, PathBuf};
use tar::Archive;
use url::Url;

#[derive(Debug, Default)]
pub struct Template {
    index_hbs: String,
    assets: BTreeMap<PathBuf, Vec<u8>>,
}

impl Template {
    /// Load a template from a reference.
    pub fn load_from_reference(reference: &TemplateReference) -> Result<Template> {
        match reference {
            TemplateReference::BuiltIn(template) => Self::load_from_built_in(template),
            TemplateReference::File(path) => Self::load_from_file(path),
        }
    }

    /// Load a template from the filesystem.
    ///
    /// The file should be a .tar archive containing at least 1 file named "index.html" or
    /// "index.hbs". This file will be considered the template code.
    ///
    /// Any additional files in the archive will be included as-is in the output.
    pub fn load_from_file(path: &Path) -> Result<Template> {
        println!(
            "Loading template from {path}",
            path = path.display().to_string().magenta()
        );
        let file = File::open(path).context("Can't open template file")?;
        let reader = BufReader::new(file);

        Self::load_from_reader(reader)
    }

    /// Load a template from a built-in templates archive.
    pub fn load_from_built_in(template: &BuiltInTemplate) -> Result<Template> {
        println!(
            "Loading built-in template {name}",
            name = template.name().cyan()
        );
        let tar_bytes = template.tar_bytes();
        let reader = Cursor::new(tar_bytes);

        Self::load_from_reader(reader)
    }

    /// Load a template from a read stream.
    ///
    /// The stream should contain the data of a .tar archive that matches the file requirements
    /// noted in [Template::load_from_file].
    fn load_from_reader(reader: impl Read) -> Result<Template> {
        let mut archive = Archive::new(reader);
        let entries = archive.entries().context(
            "Can't read entries from template tarball. Is it a valid, uncompressed .tar archive?",
        )?;

        let mut template = Template::default();
        for file in entries {
            let mut file = file?;

            let path = file.header().path()?.to_path_buf();

            // is_file() and is_dir() don't return correct/expected values on these
            // archive-local paths, so we identify files by checking if they have an
            // extension. This is not 100% foolproof, but it should be sufficient
            // for our purposes.
            if path.extension().is_none() {
                continue;
            }

            let mut contents = Vec::with_capacity(file.header().size().unwrap_or(0) as usize);
            file.read_to_end(&mut contents)?;

            if path.ends_with("index.html") || path.ends_with("index.hbs") {
                template.index_hbs = String::from_utf8(contents)?;
            } else {
                template.assets.insert(path, contents);
            }
        }

        if template.index_hbs.is_empty() {
            Err(MargoError::MissingTemplateIndex)?;
        }

        Ok(template)
    }

    /// Render the index for a given registry using this template.
    pub fn render(&self, registry: &Registry, index_config: &LatestConfigIndex) -> Result<()> {
        let hbs = Self::init_handlebars();
        let index_data = self.collect_template_data(registry, index_config)?;

        println!("Rendering HTML template");
        let index_html = hbs
            .render_template(&self.index_hbs, &index_data)
            .context("An error occurred while rendering the handlebars template.")?;

        let path = registry.path().join("index.html");
        fs::write(&path, index_html).context("Can't write index.html.")?;

        for (path, contents) in &self.assets {
            let path = &registry.path().join(path);

            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir).context("Can't write")?;
            }

            fs::write(&path, contents).context("Can't write template asset.")?;
        }

        Ok(())
    }

    fn collect_template_data<'a>(
        &self,
        registry: &'a Registry,
        index_config: &'a LatestConfigIndex,
    ) -> Result<TemplateData<'a>> {
        let indexes = registry.list_indexes()?;

        let mut crates = Vec::with_capacity(indexes.len());
        for index in &indexes {
            let Some(latest_non_yanked_version) = index.latest_non_yanked_version() else {
                continue;
            };

            let c = registry.read_crate_info(&index.name, latest_non_yanked_version)?;

            let readme_html = match &c.readme {
                Some(md) => Some(Self::render_readme(md, &c.manifest)?),
                _ => None,
            };

            crates.push(TemplateCrate {
                name: index.name.clone(),
                description: c.manifest.package.description,
                readme_html,
                readme_md: c.readme,
                latest_non_yanked_version: TemplateVersion {
                    version: latest_non_yanked_version.to_string(),
                    yanked: false,
                },
                versions: index
                    .entries
                    .iter()
                    .map(|(v, e)| TemplateVersion {
                        version: v.to_string(),
                        yanked: e.yanked,
                    })
                    .collect::<Vec<_>>()
                    .sorted_by(|a, b| b.version.cmp(&a.version)),
                files: c.files,
                features: c.manifest.features.into_keys().collect(),
                dependencies: c
                    .manifest
                    .dependencies
                    .into_iter()
                    .map(|(name, dep)| (name, dep.version.to_string()))
                    .collect(),
                build_dependencies: c
                    .manifest
                    .build_dependencies
                    .into_iter()
                    .map(|(name, dep)| (name, dep.version.to_string()))
                    .collect(),
            });
        }

        Ok(TemplateData {
            title: &index_config.title,
            registry: TemplateRegistryData {
                suggested_name: &index_config.suggested_registry_name,
                base_url: &registry.config().base_url,
            },
            crates,
        })
    }

    fn render_readme(markdown: &str, manifest: &PackagedCargoToml) -> Result<String> {
        let mut options = Options::default();
        options.extension.strikethrough = true;
        options.extension.tagfilter = true;
        options.extension.table = true;
        options.extension.tasklist = true;
        options.extension.superscript = true;
        options.extension.subscript = true;
        options.extension.shortcodes = true;

        let arena = Arena::new();
        let root = parse_document(&arena, &markdown, &options);

        fn prefix_link(link: &mut NodeLink, manifest: &PackagedCargoToml) {
            if link.url.starts_with('#') || link.url.starts_with("http") {
                return;
            }

            if let Some(repository) = &manifest.package.repository {
                link.url = format!("{}/blob/HEAD/{}", repository, link.url);
            } else {
                link.url = format!("#{}", link.url);
            }
        }

        for node in root.descendants() {
            match node.data.borrow_mut().value {
                NodeValue::Link(ref mut link) => prefix_link(link, manifest),
                NodeValue::Image(ref mut image) => prefix_link(image, manifest),
                NodeValue::Heading(ref mut h) => h.level += 1,
                NodeValue::Document => {}
                NodeValue::FrontMatter(_) => {}
                NodeValue::BlockQuote => {}
                NodeValue::List(_) => {}
                NodeValue::Item(_) => {}
                NodeValue::DescriptionList => {}
                NodeValue::DescriptionItem(_) => {}
                NodeValue::DescriptionTerm => {}
                NodeValue::DescriptionDetails => {}
                NodeValue::CodeBlock(_) => {}
                NodeValue::HtmlBlock(_) => {}
                NodeValue::Paragraph => {}
                NodeValue::ThematicBreak => {}
                NodeValue::FootnoteDefinition(_) => {}
                NodeValue::Table(_) => {}
                NodeValue::TableRow(_) => {}
                NodeValue::TableCell => {}
                NodeValue::Text(_) => {}
                NodeValue::TaskItem(_) => {}
                NodeValue::SoftBreak => {}
                NodeValue::LineBreak => {}
                NodeValue::Code(_) => {}
                NodeValue::HtmlInline(_) => {}
                NodeValue::Raw(_) => {}
                NodeValue::Emph => {}
                NodeValue::Strong => {}
                NodeValue::Strikethrough => {}
                NodeValue::Superscript => {}
                NodeValue::FootnoteReference(_) => {}
                NodeValue::ShortCode(_) => {}
                NodeValue::Math(_) => {}
                NodeValue::MultilineBlockQuote(_) => {}
                NodeValue::Escaped => {}
                NodeValue::WikiLink(_) => {}
                NodeValue::Underline => {}
                NodeValue::Subscript => {}
                NodeValue::SpoileredText => {}
                NodeValue::EscapedTag(_) => {}
                NodeValue::Alert(_) => {}
            }
        }

        let mut html = Vec::new();
        format_html(root, &options, &mut html)?;

        Ok(String::from_utf8(html)?)
    }

    fn init_handlebars<'a>() -> Handlebars<'a> {
        let mut hbs = Handlebars::new();

        fn cmp_values(a: JsonValue, b: JsonValue, ord: Ordering) -> bool {
            if let (Some(a), Some(b)) = (a.as_i64(), b.as_i64()) {
                return a.cmp(&b) == ord;
            }

            if let (Some(a), Some(b)) = (a.as_f64(), b.as_f64()) {
                return a.partial_cmp(&b).map(|cmp| cmp == ord).unwrap_or_default();
            }

            if let (Some(a), Some(b)) = (a.as_str(), b.as_str()) {
                return a.cmp(&b) == ord;
            }

            false
        }

        handlebars_helper!(len: |arr: Vec<JsonValue>| arr.len());
        handlebars_helper!(eq: |a: JsonValue, b: JsonValue| a.eq(&b));
        handlebars_helper!(ne: |a: JsonValue, b: JsonValue| a.ne(&b));
        handlebars_helper!(gt: |a: JsonValue, b: JsonValue| cmp_values(a, b, Ordering::Greater));
        handlebars_helper!(lt: |a: JsonValue, b: JsonValue| cmp_values(a, b, Ordering::Less));

        hbs.register_helper("len", Box::new(len));
        hbs.register_helper("eq", Box::new(eq));
        hbs.register_helper("ne", Box::new(ne));
        hbs.register_helper("gt", Box::new(gt));
        hbs.register_helper("lt", Box::new(lt));

        hbs
    }
}

/// Variables provided to the Handlebars template while rendering.
#[derive(Debug, Clone, Serialize)]
struct TemplateData<'a> {
    /// The configured title of the registry page. Use e.g. in the `<title>` tag in the
    /// page head, or at the top of the page.
    title: &'a str,

    /// Registry information.
    registry: TemplateRegistryData<'a>,

    /// All crates in the registry.
    crates: Vec<TemplateCrate>,
}

/// Registry information.
///
/// This struct contains configuration data and statistics about the registry, not
/// related to any specific crate.
#[derive(Debug, Clone, Serialize)]
struct TemplateRegistryData<'a> {
    /// Configured registry name to suggest to the user (e.g. in example code blocks).
    suggested_name: &'a str,

    /// Configured registry base URL.
    base_url: &'a Url,
}

/// Crate information.
///
/// Contains information and metadata related to the given crate.
///
/// Data that may change between versions is always retrieved from the latest
/// (non-yanked) version in the registry. It's currently not possible to get this
/// data for previous versions. This is a performance consideration,
/// since Margo doesn't keep track of all metadata in a database so it would
/// have to parse every .crate file in the registry (a slow and computationally
/// expensive action) instead of just one per package.
#[derive(Debug, Clone, Serialize)]
struct TemplateCrate {
    /// Name of the crate, from `Cargo.toml`
    name: PackageName,

    /// Description, from `Cargo.toml`.
    description: Option<String>,

    /// Contents of the README.md file, if one exists in the crate.
    readme_md: Option<String>,

    /// The README.md file contents rendered to HTML tags.
    readme_html: Option<String>,

    /// The latest version of the crate, as a string.
    latest_non_yanked_version: TemplateVersion,

    /// An array of all available versions for this crate, as strings.
    versions: Vec<TemplateVersion>,

    /// Available crate features.
    features: Vec<FeatureName>,

    /// A list of all files in the crate package.
    files: Vec<PathBuf>,

    /// The crate's main dependencies, as a `name: version` dictionary.
    dependencies: BTreeMap<PackageName, String>,

    /// The crate's build dependencies, as a `name: version` dictionary.
    build_dependencies: BTreeMap<PackageName, String>,
}

/// A version in the crate.
///
/// Internally these are [semver::Version] structs, but they are serialised before sending
/// them to the template to improve rendering performance and make it easier for templates
/// to display version strings.
#[derive(Debug, Clone, Serialize)]
struct TemplateVersion {
    /// The version string.
    version: String,

    /// Whether this version was yanked.
    yanked: bool,
}
