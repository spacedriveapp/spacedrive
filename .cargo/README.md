## Cargo Config

The `config.toml` file is generated from `config.toml.mustache` by running:

```bash
cargo run -p xtask -- setup
```

This file is gitignored because it contains machine-specific paths. After setup, you can use `cargo xtask` for subsequent commands.
