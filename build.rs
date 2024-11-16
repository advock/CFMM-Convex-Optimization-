fn main() {
    // Tell cargo to look for shared libraries in the Homebrew directory
    println!("cargo:rustc-link-search=/opt/homebrew/lib");

    // Tell cargo to link these libraries
    println!("cargo:rustc-link-lib=CbcSolver");
    println!("cargo:rustc-link-lib=Cbc");
    println!("cargo:rustc-link-lib=Clp");
    println!("cargo:rustc-link-lib=CoinUtils");
    println!("cargo:rustc-link-lib=Osi");
    println!("cargo:rustc-link-lib=OsiClp");
    println!("cargo:rustc-link-lib=Cgl");

    // Only rebuild if build.rs changes
    println!("cargo:rerun-if-changed=build.rs");
}
