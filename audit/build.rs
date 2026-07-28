use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=RANDOMX_LIB_DIR");

    if env::var_os("CARGO_FEATURE_OFFICIAL_RANDOMX").is_none() {
        return;
    }

    let library_dir = env::var("RANDOMX_LIB_DIR")
        .expect("RANDOMX_LIB_DIR must name the directory containing librandomx.a");
    println!("cargo:rustc-link-search=native={library_dir}");
    println!("cargo:rustc-link-lib=static=randomx");
    println!("cargo:rustc-link-lib=dylib=stdc++");
}
