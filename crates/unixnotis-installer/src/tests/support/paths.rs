impl crate::paths::InstallPaths {
    pub(crate) fn discover() -> anyhow::Result<Self> {
        // Test callers use the same automatic manager selection as the normal CLI
        Self::discover_with_service_manager(None)
    }
}
