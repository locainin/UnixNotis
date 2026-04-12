use anyhow::{anyhow, Result};
use std::path::Path;

pub(super) fn sanitize_payload_mode(mode: u32, relative_path: &Path) -> Result<u32> {
    // Keep only permission bits and reject setuid/setgid/sticky flags from preset payloads
    let permission_mode = mode & 0o7777;
    if permission_mode & 0o7000 != 0 {
        return Err(anyhow!(
            "preset payload contains unsupported special permission bits: {}",
            relative_path.display()
        ));
    }
    Ok(permission_mode & 0o777)
}
