use super::super::generation::GenerationExit;

#[test]
fn generation_exit_keeps_owner_change_connection_loss_and_shutdown_distinct() {
    assert!(matches!(
        GenerationExit::OwnerChanged,
        GenerationExit::OwnerChanged
    ));
    assert!(matches!(
        GenerationExit::ConnectionLost,
        GenerationExit::ConnectionLost
    ));
    assert!(matches!(GenerationExit::Shutdown, GenerationExit::Shutdown));
    assert!(matches!(GenerationExit::Retry, GenerationExit::Retry));
}
