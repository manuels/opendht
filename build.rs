extern crate cc;

fn main() {
    println!("cargo:rustc-flags=-l gnutls -l nettle -l argon2");

    cc::Build::new()
        .file("src/wrapper.cpp")
        .cpp(true)
        .compile("dht-wrapper");

    println!("cargo:rustc-link-lib=static=dht-wrapper");
    println!("cargo:rustc-link-lib=stdc++");
}
