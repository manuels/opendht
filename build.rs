extern crate cc;

fn main() {
    println!("cargo:rustc-flags=-lopendht -lgnutls -lssl -lcrypto -lnettle -lpthread -ljsoncpp -largon2 -lhttp_parser");

    cc::Build::new()
        .file("src/wrapper.cpp")
        .cpp(true)
        .flag_if_supported("-std=c++14")
        .compile("dht-wrapper");

    println!("cargo:rustc-link-lib=static=dht-wrapper");
    println!("cargo:rustc-link-lib=stdc++");
}
