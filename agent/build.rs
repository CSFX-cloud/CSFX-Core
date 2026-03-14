fn main() {
    if let Ok(v) = std::env::var("CSF_BUILD_VERSION") {
        println!("cargo:rustc-env=CARGO_PKG_VERSION={}", v);
    }
    println!("cargo:rerun-if-env-changed=CSF_BUILD_VERSION");
}
