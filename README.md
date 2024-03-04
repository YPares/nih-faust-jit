# Nih Faust Stereo Fx Jit

Very much work in progress. Initial script to load is fixed at build time for now.

## Building

First install [Rust](https://rustup.rs/) and [Faust](https://faust.grame.fr/downloads/).

For now, Faust paths need to be provided through environment variables at build
time:

- `FAUST_LIB_PATH`: where to look for `libfaust`. It will link statically with
  `libfaustwithllvm` in order to generate a self-contained plugin
- `FAUST_HEADERS_PATH`: where to look for the Faust C/CPP headers
- `DSP_LIBS_PATH`: where the plugin should look for the [Faust DSP
  libraries](https://faustlibraries.grame.fr/), so your script can import
  `stdfaust.lib`
- `DSP_SCRIPT_PATH`: which Faust script to load when the plugin starts

You can set these env vars via command line, or edit the `.cargo/config.toml`
before building.

Then, you can the compile and package the CLAP, VST and standalone plugins with:

```shell
cargo xtask bundle nih_faust_stereo_fx_jit --release
```

Running the standalone version of the plugin is just:

```shell
cargo run --release
```
