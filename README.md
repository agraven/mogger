# Mogger

Mogger is the blogging engine I use to run my website, [amandag.net](https://amandag.net). It's written in Rust and built with the gotham web framework.

## Building

Mogger depends on libpq to build. You can install it on debian based systems by running the command `sudo apt-get install libpq-dev`. Note that it's exclusively a build-time dependency, you won't need it to run mogger once you've built it.

Once you've installed dependencies, you can build mogger by running
```bash
cargo build --release
```
You can also use [cargo-deb](https://github.com/mmstick/cargo-deb) to build a debian package.

## Installing

The preferred way of installing mogger is with the debian packages provided on the GitHub releases page. If you want to install it manually, you can look in `Cargo.toml` to see what files go where.
