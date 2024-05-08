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

This will create a new registry in the directory
`my-registry-directory` that you plan to serve from
`https://my-registry.example.com`.

```bash
margo init my-registry-directory --base-url https://my-registry.example.com
```

### Add a crate to the registry

To add a new crate or version to the registry, run `margo add` and specify
the path to the directory you gave to `margo init` and the `.crate` file
to publish.

```bash
# Acquire a crate package, such as by running `cargo package`
margo add --registry my-registry-directory some-crate/target/package/some-crate-1.2.3.crate
```

### Serve the registry files with your choice of webserver

For example, using Python and serving the registry in the directory
at `127.0.0.1`:

```bash
python3 -m http.server --bind '127.0.0.1' --dir 'my-registry-directory'
```

You should be able to visit `127.0.0.1/config.json` in your browser.
Your next step is to serve those files from
`https://my-registry.example.com` instead, in whatever way you
serve static files from whatever URL you've specified.

### Configure Cargo

```bash
# In your Rust project that wants to use `some-crate`
mkdir .cargo
cat >>.cargo/config.toml <<EOF
[registries]
my-registry = { index = "sparse+https://my-registry.example.com" }
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

## Development

See [DEVELOPMENT.md](./DEVELOPMENT.md).
