fn main() {
    // Compile controlled semantic icons into the binary so desktop themes cannot replace meaning
    glib_build_tools::compile_resources(
        &["resources"],
        "resources/resources.gresource.xml",
        "unixnotis-popups.gresource",
    );
}
