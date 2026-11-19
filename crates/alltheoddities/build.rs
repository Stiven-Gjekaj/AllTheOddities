//! Gives the code the target of the build, so `odd update` downloads the
//! binary of the same target.

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    println!("cargo:rustc-env=ODD_TARGET={target}");
}
