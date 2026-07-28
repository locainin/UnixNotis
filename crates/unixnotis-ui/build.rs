fn main() {
    // Compile security badges once so every UI client renders the same controlled symbols
    glib_build_tools::compile_resources(
        &["resources"],
        "resources/resources.gresource.xml",
        "unixnotis-ui.gresource",
    );
}
