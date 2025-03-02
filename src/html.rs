use indoc::formatdoc;
use maud::{html, Markup, PreEscaped, DOCTYPE};
use semver::Version;
use snafu::prelude::*;
use std::{fs, io, path::PathBuf};

use crate::{index_entry, ConfigV1, Index, ListAll, Registry};

#[rustfmt::skip]
mod assets;

pub fn write(registry: &Registry) -> Result<(), Error> {
    use error::*;

    let crates = registry.list_all()?;
    let index = index(&registry.config, &crates).into_string();
    let index_path = registry.path.join("index.html");
    fs::write(&index_path, index).context(WriteIndexSnafu { path: index_path })?;

    let assets_dir = registry.path.join("assets");
    fs::create_dir_all(&assets_dir).context(AssetDirSnafu { path: &assets_dir })?;

    let css_path = assets_dir.join(assets::CSS_NAME);
    fs::write(&css_path, assets::CSS).context(CssSnafu { path: &css_path })?;

    let css_map_path = {
        let mut css_map_path = css_path;
        css_map_path.as_mut_os_string().push(".map");
        css_map_path
    };
    fs::write(&css_map_path, assets::CSS_MAP).context(CssMapSnafu {
        path: &css_map_path,
    })?;

    let js_path = assets_dir.join(assets::JS_NAME);
    fs::write(&js_path, assets::JS).context(JsSnafu { path: &js_path })?;

    let js_map_path = {
        let mut js_map_path = js_path;
        js_map_path.as_mut_os_string().push(".map");
        js_map_path
    };
    fs::write(&js_map_path, assets::JS_MAP).context(JsMapSnafu { path: &js_map_path })?;

    Ok(())
}

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum Error {
    #[snafu(display("Could not list the crates"))]
    #[snafu(context(false))]
    ListAll { source: crate::ListAllError },

    #[snafu(display("Could not write the HTML index page to {}", path.display()))]
    WriteIndex { source: io::Error, path: PathBuf },

    #[snafu(display("Could not create the HTML asset directory at {}", path.display()))]
    AssetDir { source: io::Error, path: PathBuf },

    #[snafu(display("Could not write the CSS file to {}", path.display()))]
    Css { source: io::Error, path: PathBuf },

    #[snafu(display("Could not write the CSS sourcemap file to {}", path.display()))]
    CssMap { source: io::Error, path: PathBuf },

    #[snafu(display("Could not write the JS file to {}", path.display()))]
    Js { source: io::Error, path: PathBuf },

    #[snafu(display("Could not write the JS sourcemap file to {}", path.display()))]
    JsMap { source: io::Error, path: PathBuf },
}

const CARGO_DOCS: &str =
    "https://doc.rust-lang.org/cargo/reference/registries.html#using-an-alternate-registry";

fn index(config: &ConfigV1, crates: &ListAll) -> Markup {
    let base_url = &config.base_url;
    let suggested_name = config.html.suggested_registry_name();

    let asset_head_elements = PreEscaped(assets::INDEX);

    fn link(href: &str, content: &str) -> Markup {
        html! {
            a href=(href) class="underline text-blue-600 hover:text-blue-800 visited:text-purple-600" {
                (content)
            }
        }
    }

    fn section(name: &str, id: &str, content: Markup) -> Markup {
        html! {
            section class="p-1" {
                h1 class="text-2xl" {
                    a class="hover:after:content-['_§']" id=(id) href={"#" (id)} {
                        (name)
                    }
                }

                (content)
            }
        }
    }

    fn code_block(content: impl AsRef<str>) -> Markup {
        let content = content.as_ref();

        let span_class = "col-start-1 row-start-1 leading-none p-1";

        html! {
            mg-copy {
                pre class="relative border border-black bg-theme-rose-light m-1 p-1 overflow-x-auto" {
                    button class="hidden absolute top-0 right-0 grid" data-target="copy" {
                        span class=(span_class) data-target="state0" { "Copy" }
                        span class={(span_class) " invisible"} data-target="state1" { "Copied" }
                    }
                    code data-target="content" { (content) }
                }
            }
        }
    }

    let config_stanza = formatdoc! {r#"
        [registries]
        {suggested_name} = {{ index = "sparse+{base_url}" }}
    "#};

    let cargo_add_stanza = formatdoc! {"
        cargo add --registry {suggested_name} some-crate-name
    "};

    html! {
        (DOCTYPE)
        html lang="en-US" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Margo Crate Registry" };
                (asset_head_elements);
            }

            body class="flex flex-col min-h-screen bg-theme-salmon-light" {
                header {
                    h1 class="text-3xl font-bold bg-theme-purple text-theme-salmon-light p-2 drop-shadow-xl" {
                        "Margo Crate Registry"
                    }
                }

                (section("Getting started", "getting-started", html! {
                    ol class="list-inside list-decimal" {
                        li {
                            "Add the registry definition to your "
                            code { ".cargo/config.toml" }
                            ":"

                            (code_block(config_stanza))
                        }

                        li {
                            "Add your dependency to your project:"

                            (code_block(cargo_add_stanza))
                        }
                    }

                    "For complete details, check the "
                    (link(CARGO_DOCS, "Cargo documentation"))
                    "."
                }))

                (section("Available crates", "crates", html! {
                    table class="table-fixed w-full" {
                        thead {
                            tr {
                                th class="w-4/5 text-left" { "Name" }
                                th { "Versions" }
                            }
                        }

                        tbody {
                            @for (c, v) in crates {
                                tr class="hover:bg-theme-orange" {
                                    td {
                                        span class="truncate" { (c.as_str()) }
                                    }
                                    td {
                                        select class="w-full bg-white" name="version" {
                                            @for (v, c, select) in most_interesting(v) {
                                                @let suffix = if c.yanked { " (yanked)" } else { "" };
                                                option selected[select] { (v) (suffix) }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }))

                footer class="grow place-content-end text-center" {
                    span class="border-t border-dashed border-theme-purple" {
                        "Powered by "
                        (link("https://github.com/integer32llc/margo", "Margo"))
                    }
                }
            }
        }
    }
}

fn most_interesting(i: &Index) -> impl Iterator<Item = (&Version, &index_entry::Root, bool)> {
    let last_non_yanked = i.iter().rfind(|(_, c)| !c.yanked).map(|(v, _)| v);

    i.iter()
        .map(move |(v, c)| (v, c, Some(v) == last_non_yanked))
}
