use std::ffi::OsString;

pub(super) fn command_program(program: &str) -> std::io::Result<OsString> {
    // Production resolves backend tools from trusted system directories, not inherited PATH
    crate::system_tools::program_path(program).map(std::path::PathBuf::into_os_string)
}
