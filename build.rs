use std::env;
use std::path::PathBuf;

fn main() {
    let mlt = pkg_config::Config::new()
        .probe("mlt-framework-7")
        .expect("libmlt não encontrado: instale libmlt-dev (pkg-config mlt-framework-7)");

    let mut builder = bindgen::Builder::default()
        .header_contents("mlt_wrapper.h", "#include <framework/mlt.h>")
        .allowlist_function("mlt_.*")
        .allowlist_type("mlt_.*")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));

    for path in &mlt.include_paths {
        builder = builder.clang_arg(format!("-I{}", path.display()));
    }

    let bindings = builder
        .generate()
        .expect("bindgen não conseguiu gerar os bindings do libmlt");

    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR não definido"));
    bindings
        .write_to_file(out.join("mlt_bindings.rs"))
        .expect("não consegui gravar mlt_bindings.rs");

    println!("cargo:rustc-env=MLT_VERSION={}", mlt.version);
}
