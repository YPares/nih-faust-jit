# Nih Faust Stereo Fx Jit

A plugin to load Faust dsp files and JIT-compile them with LLVM. A simple GUI is
provided to select which script to load and where to look for the Faust
libraries that this script may import. The selected DSP script is saved as part
of the plugin state and therefore is saved with your DAW project.

## Building

First install [Rust](https://rustup.rs/) and [Faust](https://faust.grame.fr/downloads/).

For now, Faust paths need to be provided through environment variables at build
time:

- `FAUST_LIB`: which `libfaustXXX` to link with. By default it statically links
  with `libfaustwithllvm` in order to generate a self-contained plugin (more
  convenient on Windows)
- `FAUST_LIB_PATH`: where to look for `libfaustXXX`
- `FAUST_HEADERS_PATH`: where to look for the Faust C/CPP headers
- `DSP_LIBS_PATH`: where the plugin should look by default for the [Faust DSP
  libraries](https://faustlibraries.grame.fr/), so your script can import
  `stdfaust.lib`. This can then be overriden at runtime with the plugin's GUI

You can set these env vars via command line, or edit the `.cargo/config.toml`
before building. Check `.github/workflows/rust.yml` to see e.g. how these are
overriden for building on Ubuntu.

Then, you can the compile and package the CLAP, VST and standalone plugins with:

```shell
cargo xtask bundle nih_faust_stereo_fx_jit --release
```

Running the standalone version of the plugin is just:

```shell
cargo run --release
```
