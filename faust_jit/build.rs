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
    for c_file in glob::glob("c_src/**/*").unwrap() {
        println!(
            "cargo:rerun-if-changed={}",
            c_file.unwrap().to_str().unwrap()
        );
    }

    let faust_headers_path =
        env::var("FAUST_HEADERS_PATH").expect("env var FAUST_HEADERS_PATH not found");

    // The bindgen::Builder is the main entry point to bindgen, and lets you
    // build up options for the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("c_src/wrapper.hpp")
        //.clang_arg(format!("-I{}", faust_headers_path))
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .rustified_enum("WWidgetDeclType")
        .rustified_enum("WMidiSyncMsg")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    let mut cc = cc::Build::new();
    cc.cpp(true)
        .std("c++14")
        .include(faust_headers_path)
        .file("c_src/wrapper.cpp");
    #[cfg(feature = "define_faust_static_vars")]
    cc.define("DEFINE_FAUST_STATIC_VARS", "");
    cc.compile("wrapper-lib");
}
