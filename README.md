# nih-faust-jit

A plugin to load Faust dsp files and JIT-compile them with LLVM. A simple GUI is
provided to select which script to load and where to look for the Faust
libraries that this script may import. The selected DSP script is saved as part
of the plugin state and therefore is saved with your DAW project.

Both effect and instrument DSPs are supported, with MIDI notes and CCs in both
cases. The DSP script type is normally detected from its metadata. E.g. if the
script contains a line like:

`declare options "[midi:on][nvoices:12]";`

then the script will be considered to be an instrument with `12` voices of
polyphony. But you can override this via the GUI to force the DSP script type
and number of voices: this is notably useful for scripts that describe
instruments but do not contain a `[nvoices:xxx]` metadata.


## Building

First install [Rust](https://rustup.rs/) and [Faust](https://faust.grame.fr/downloads/).

For now, Faust paths need to be provided through environment variables at build
time:

- `FAUST_LIB`: which `libfaustXXX` to link with. By default it statically links
  with `libfaustwithllvm` in order to generate a self-contained plugin (which is
  more convenient if you are on Windows, as else you would need extra setup so
  the plugin can find Faust and llvm DLLs at runtime). Just set it to `"faust"`
  if you are on OSX or Linux and want to dynamically link with a regular system
  installation of Faust and llvm (dynamic linking is cargo's default, and
  shouldn't be a problem there)
- `FAUST_LIB_PATH`: where to look for the faust static/dynamic library
- `FAUST_HEADERS_PATH`: where to look for the Faust C/CPP headers
- `DSP_LIBS_PATH`: where the plugin should look by default for the [Faust DSP
  libraries](https://faustlibraries.grame.fr/), so your script can import e.g.
  `"stdfaust.lib"`. This can then be overriden at runtime with the plugin's GUI

You can set these env vars via command line, or edit the `.cargo/config.toml`
before building. You may need to run `cargo clean` after changing them so new
values are taken into account. Check `.github/workflows/rust.yml` to see e.g.
how these are overriden for building on Ubuntu.

Then, you can compile and package the VST3 and CLAP plugins with:

```shell
cargo xtask bundle nih_faust_jit --release
```

Running the standalone version of the plugin is just:

```shell
cargo run --release
```

## Known shortcomings

- Scripts are (re)loaded only when clicking on the `Set or reload DSP script`
  button. Therefore, anytime you modify a parameter on the GUI, don't forget to
  manually reload the script (just re-select the same file in the file picker).
- Volume can get high quickly when using polyphonic DSPs, because Faust voices
  are just summed together. The plugin exposes a Gain parameter to the host.
  Don't forget to use it if your instrument script doesn't perform some volume
  reduction already.
- When using polyphonic instruments, voice stealings will result in small
  audible clicks in the sound. To be investigated, but for now set your voice
  number high enough.
