use regex::Regex;
use std::{
    env,
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

fn main() {
    if cfg!(feature = "html") {
        capture_html_assets();
    }
}

fn capture_html_assets() {
    const ASSET_ROOT: &str = "ui/dist";
    const ASSET_INDEX: &str = "ui.html";

    let root = env::var("CARGO_MANIFEST_DIR").expect("`CARGO_MANIFEST_DIR` must be set");
    let root = PathBuf::from(root);

    let asset_root = root.join(ASSET_ROOT);
    let asset_index = asset_root.join(ASSET_INDEX);

    let entry = fs::read_to_string(&asset_index).expect("Could not read the UI entrypoint");

    let find_css =
        Regex::new(r#"href="assets/(ui.[a-zA-Z0-9]+.css)""#).expect("Invalid CSS regex");
    let (_, [css_name]) = find_css
        .captures(&entry)
        .expect("Could not find CSS")
        .extract();

    let css = asset_root.join(css_name);
    let css_map = {
        let mut c = css.clone();
        c.as_mut_os_string().push(".map");
        c
    };

    let out_path = env::var("OUT_DIR").expect("`OUT_DIR` must be set");
    let mut out_path = PathBuf::from(out_path);
    out_path.push("html");

    fs::create_dir_all(&out_path).unwrap_or_else(|e| {
        panic!(
            "Could not create the HTML assets directory `{path}`: {e}",
            path = out_path.display(),
        );
    });

    out_path.push("assets.rs");
    let mut output = File::create(&out_path).unwrap_or_else(|e| {
        panic!(
            "Could not open the HTML assets file `{path}`: {e}",
            path = out_path.display(),
        );
    });

    write!(
        output,
        r##"
        pub const INDEX: &str = include_str!("{asset_index}");

        pub const CSS_NAME: &str = "{css_name}";
        pub const CSS: &str = include_str!("{css}");
        pub const CSS_MAP: &str = include_str!("{css_map}");
        "##,
        asset_index = asset_index.display(),
        css_name = css_name.escape_default(),
        css = css.display(),
        css_map = css_map.display(),
    )
    .expect("Could not write HTML assets file");

    println!("cargo::rerun-if-changed=build.rs");
    println!(
        "cargo::rerun-if-changed={asset_index}",
        asset_index = asset_index.display(),
    );
}
