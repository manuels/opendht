extern crate cc;

fn main() {
    println!("cargo:rustc-flags=-l gnutls -l nettle -l argon2");

    cc::Build::new()
        .cpp(true)
        .file("src/wrapper.cpp")
        .compile("dht-wrapper");

    println!("cargo:rustc-link-lib=static=dht-wrapper");
}
