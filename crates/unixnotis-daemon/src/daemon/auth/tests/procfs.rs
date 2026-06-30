use super::process::read_process_executable_path;

#[cfg(target_os = "linux")]
#[tokio::test]
async fn read_process_executable_path_reads_current_process() {
    let exe = read_process_executable_path(std::process::id())
        .await
        .expect("current process executable should be readable");

    assert!(exe.is_absolute());
}
