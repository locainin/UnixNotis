use std::str::FromStr;

use crate::cli::{Command, DndDuration, DndState, DoctorCommand, DoctorServiceManagerArg};

use super::local::handle_local_command;
use super::runner::{run_async, run_command};

#[test]
fn async_doctor_report_fails_closed_in_sync_local_dispatcher() {
    let command = Command::Doctor {
        command: None,
        json: false,
        verbose: false,
        service_manager: DoctorServiceManagerArg::Auto,
        config: None,
    };
    let error = handle_local_command(command)
        .expect_err("async doctor report must fail in synchronous dispatcher");

    assert!(error.to_string().contains("internal routing error"));
}

#[test]
fn run_command_validates_semantics_before_starting_any_runtime_work() {
    let command = Command::Dnd {
        state: DndState::Off,
        for_duration: Some(DndDuration::from_str("30m").expect("valid duration")),
        until: None,
    };
    let error = run_command(command).expect_err("invalid DND command must fail before dispatch");

    assert!(error.to_string().contains("valid only with `dnd on`"));
}

#[tokio::test]
async fn doctor_repair_fails_closed_in_async_dispatcher() {
    let command = Command::Doctor {
        command: Some(DoctorCommand::RepairSession),
        json: false,
        verbose: false,
        service_manager: DoctorServiceManagerArg::Auto,
        config: None,
    };
    let error = run_async(command)
        .await
        .expect_err("synchronous repair must fail in async dispatcher");

    assert!(error.to_string().contains("internal routing error"));
}
