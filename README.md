# Margo

Margo is an alternate [Cargo registry][registry] that is built using
completely static files, allowing it to be easily served.

[registry]: https://doc.rust-lang.org/cargo/reference/registries.html

## Quickstart

To use Margo in GitHub Actions, such as hosting your registry on
GitHub Pages, check out our [action][].

[action]: https://github.com/integer32llc/margo-actions

### Install Margo

```bash
cargo install margo
```

### Initialize the registry

```bash
margo init my-registry-directory --base-url https://my-registry.example.com
```

### Add a crate to the registry

```bash
# Acquire a crate package, such as by running `cargo package`
margo add --registry my-registry-directory some-crate/target/package/some-crate-1.2.3.crate
```

### Serve the registry files with your choice of webserver

```bash
python3 -m http.server --bind '127.0.0.1' my-registry-directory
```

### Configure Cargo

```bash
# In your Rust project that wants to use `some-crate`
mkdir .cargo
cat >>.cargo/config.toml <<EOF
[registries]
my-registry = { index = "https://my-registry.example.com" }
EOF
```

### Add your crate

```bash
cargo add --registry my-registry some-crate
```

## Key differences from Crates.io

- 💅 Does not impose file size limits
- 💅 Can depend on crates from registries other than crates.io
- 💅 Dependencies are not required to exist
- 💅 Development dependency info is not stored in the index
- 💅 Does not require JavaScript

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md).
