pub fn require_diagnostic_mode(diagnostic_mode: bool) -> anyhow::Result<()> {
    // The dev namespace is discoverability only and never grants diagnostic access
    if !diagnostic_mode {
        anyhow::bail!("diagnostic notification output requires UNIXNOTIS_DIAGNOSTIC=1");
    }

    Ok(())
}
