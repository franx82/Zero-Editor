fn main() {
    #[cfg(target_os = "macos")]
    {
        cc::Build::new()
            .file("src/macos.m")
            .flag("-fobjc-arc")
            .compile("macos_open");

        println!("cargo:rustc-link-lib=framework=Cocoa");
        println!("cargo:rerun-if-changed=src/macos.m");
    }
}
