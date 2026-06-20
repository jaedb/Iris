fn main() {
    #[cfg(feature = "wayland")]
    {
        println!("cargo:warning=\"wayland\" feature is deprecated, use \"waylandegl\" instead");
    }
    gst_plugin_version_helper::info()
}
