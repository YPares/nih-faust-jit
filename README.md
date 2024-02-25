# Nih Faust Stereo Fx Jit

Very much work in progress

## Building

After installing [Rust](https://rustup.rs/), you can compile Nih Faust Stereo Fx Jit as follows:

```shell
cargo xtask bundle nih_faust_stereo_fx_jit --release
```

For now, it requires that Faust is pre-installed on your system (and expects
default Windows paths, to be changed)

## TODO

- Remove hardcoded Faust paths
