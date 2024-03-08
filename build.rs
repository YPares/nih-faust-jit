use std::env;
use std::path::PathBuf;

fn main() {
    // Tell cargo to look for shared libraries in the specified directory
    println!(
        "cargo:rustc-link-search={}",
        env::var("FAUST_LIB_PATH").expect("env var FAUST_LIB_PATH not found")
    );

    // Tell cargo to tell rustc to statically link with libfaust and llvm
    println!(
        "cargo:rustc-link-lib={}",
        env::var("FAUST_LIB").expect("env var FAUST_LIB not found")
    );

    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed=src/wrapper.hpp");

    let faust_headers_path =
        env::var("FAUST_HEADERS_PATH").expect("env var FAUST_HEADERS_PATH not found");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("src/wrapper.hpp")
        .clang_arg(format!("-I{}", faust_headers_path))
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        //.opaque_type("std::.*")
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    cc::Build::new()
        .cpp(true)
        //.std("c++20")
        .include(faust_headers_path)
        .file("src/wrapper.cpp")
        .compile("wrapper-lib");
}
