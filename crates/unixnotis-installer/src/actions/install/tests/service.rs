use crate::paths::InstallPaths;

mod backend_artifacts;
mod writes;

fn expected_primary_artifact_contents(paths: &InstallPaths) -> String {
    // Lifecycle tests only care that the selected backend artifact is already current
    // The primary artifact is backend-selected, so tests avoid assuming a systemd unit path
    paths
        .service
        .artifacts(&paths.bin_dir)
        .into_iter()
        .find(|artifact| artifact.path == paths.service.primary_artifact_path())
        .and_then(|artifact| artifact.contents)
        .expect("primary artifact should have rendered contents")
}
