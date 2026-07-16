use std::io::{self, Write};

use crate::doctor::model::{
    DoctorCheck, DoctorLogResult, DoctorLogSource, DoctorReport, DoctorSeverity,
};

use super::write_report;

#[test]
fn ordered_check_aggregation_keeps_insertion_order_and_error_semantics() {
    let checks = vec![
        DoctorCheck::new("environment", "Environment", DoctorSeverity::Pass, "ready"),
        DoctorCheck::new("logs", "Logs", DoctorSeverity::Note, "unavailable"),
        DoctorCheck::new("dbus", "D-Bus", DoctorSeverity::Error, "missing"),
    ];
    let report = DoctorReport::new(
        checks,
        DoctorLogResult::Unavailable {
            source: DoctorLogSource::Unknown,
            reason: "unknown".to_string(),
            hint: None,
        },
    );

    assert_eq!(report.checks[0].id, "environment");
    assert_eq!(report.checks[1].id, "logs");
    assert_eq!(report.checks[2].id, "dbus");
    assert!(report.has_errors());
}

struct BrokenPipeWriter;

impl Write for BrokenPipeWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "reader closed the pipe",
        ))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct FailedWriter;

impl Write for FailedWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("output device failed"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn doctor_output_treats_a_closed_pipeline_as_normal_shutdown() {
    write_report(BrokenPipeWriter, "report").expect("broken pipe should not fail doctor");
}

#[test]
fn doctor_output_preserves_non_pipe_write_failures() {
    let error = write_report(FailedWriter, "report").expect_err("device failure should propagate");

    assert!(error.to_string().contains("output device failed"));
}
