extern crate cc;
extern crate pkg_config;

fn main() {
    let opendht = pkg_config::Config::new().probe("opendht").unwrap();
    let opendht_version: Vec<&str> = opendht.version.split(".").collect();

    println!("cargo:rustc-flags=-lopendht -lgnutls -lssl -lcrypto -lnettle -lpthread -ljsoncpp -largon2 -lhttp_parser");

    cc::Build::new()
        .file("src/wrapper.cpp")
        .cpp(true)
        .flag_if_supported("-std=c++14")
        .define("OPENDHT_VERSION", opendht.version.as_str())
        .define("OPENDHT_MAJOR_VERSION", opendht_version[0])
        .compile("dht-wrapper");

    println!("cargo:rustc-link-lib=static=dht-wrapper");
    println!("cargo:rustc-link-lib=stdc++");
}
