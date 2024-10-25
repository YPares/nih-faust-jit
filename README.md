[![built with garnix](https://img.shields.io/endpoint.svg?url=https%3A%2F%2Fgarnix.io%2Fapi%2Fbadges%2FYPares%2Fnih-faust-jit)](https://garnix.io/repo/YPares/nih-faust-jit)

# nih-faust-jit

A plugin to load Faust dsp files and JIT-compile them with LLVM. Limited to
stereo audio (DSP scripts with more than 2 input/ouput chans will be refused).
The selected DSP script is saved as part of the plugin state and therefore is
saved with your DAW project. A two-part GUI is provided:

- Select which script to load and where to look for the Faust libraries that
this script may import
- Tweak the parameters described in the script (shown as various `egui` widgets)

Both effect and instrument DSPs are supported, with MIDI notes and CCs in both
cases. The DSP script type is normally detected from its metadata. E.g. if the
script contains a line like:

`declare options "[midi:on][nvoices:12]";`

then the script will be considered to be an instrument with `12` voices of
polyphony. But you can override this via the GUI to force the DSP script type
and number of voices: this is notably useful for scripts that describe
instruments but do not contain a `[nvoices:xxx]` metadata.

## UI

![screenshot](./_misc/screenshot.png)

- DSP widgets are shown in a two-directional scrollable panel (you can also
  left-click on empty space and drag to pan around)
- `v`/`h`/`tgroup`s are implemented as foldable containers
- double-click on any slider's label to reset it to its default value
- hover a bargraph to see its current value

## Building

First install [Rust](https://rustup.rs/) and [Faust](https://faust.grame.fr/downloads/).

For now, Faust paths need to be provided through environment variables at build
time. The **build time** environment variables are:

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
- `LLVM_CACHE_FOLDER`: where to cache the llvm bytecode of the scripts, for
  shorter reload times. This variable must be set, but can be an empty string if
  you do not want to use caching. This folder will be created if it doesn't
  exist, so you can just delete it to flush the cache. **Caching is based only
  on the contents of the script itself, not on what it may import**.

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

On Windows, if you are getting an error like:

```
thread 'cpal_wasapi_out' panicked at 'Received 1056 samples, while the configured buffer size is 512'
```

when using the standalone exe with the default WASAPI audio backend, it means
you should set the buffer size with:

```shell
cargo run --release -- -p 1056
```

## Installing via the Nix flake

`nih_faust_jit` is also packaged with Nix. If you are not using NixOS, running the plugin (standalone or not)
requires your Nix installation to be able to run OpenGL applications, which requires an extra bit of setup.
You can use [nix-system-graphics](https://github.com/soupglasses/nix-system-graphics) for that effect.

Then, running the standalone exe of the plugin is:

```shell
nix run . # Runs the default app, which is nih_faust_jit_standalone
```

and building and packaging the VST3 & CLAP plugins and the standalone exe is:

```shell
nix build . # Builds the default package, which is nih_faust_jit
```

which will create a `./result` symlink with two folders, `plugin` and `bin`.

## Known shortcomings

- Scripts are reloaded only when clicking on the `Set or reload DSP script`
  button. Therefore, anytime you modify something in the top panel (ie. things
  related to how the DSP should be loaded), don't forget to manually reload the
  script (just re-select the same file in the file picker).
- Volume can get high quickly when using polyphonic DSPs, because Faust voices
  are just summed together. The plugin exposes a Gain parameter to the host.
  Don't forget to use it if your instrument script doesn't perform some volume
  reduction already.
- Parameters changed via the GUI widgets are not saved in the plugin's state.
  They will return to the default value they have in the script when the
  plugin is reloaded.
- Keyboard input is not supported (you cannot directly type a value in numeric entry).
  This comes from [a bug in baseview](https://github.com/RustAudio/baseview/issues/152).

## Faust features not yet supported

- Soundfiles
- Some style (`knob` and `led`) and scale (`exp` for sliders/bargraphs, and
  `log` for bargraphs) metadata are not taken into account in the GUI

## Crates

The main crate containing the plugin is `nih_faust_jit`. Parts of its logic are
exposed as lower-level crates, that could be reused in other projects:

**`faust_jit`** defines the `SingletonDsp` type. It wraps the part of the
`libfaust` API that is needed to:

- load an effect or instrument DSP from a script,
- process audio buffers with it,
- extract the information needed to build a GUI that can tweak the DSP's
  internal parameters (represented as the `DspWidget` type).
  
`faust_jit` is related to [rust-faust](https://github.com/Frando/rust-faust),
but `rust-faust` deals only with static compilation of DSP scripts to Rust code.
The `faust_jit` crate is not limited to stereo DSP scripts (only the plugin is).

**`faust_jit_egui`** draws an `egui` GUI from the `DspWidget`s.
