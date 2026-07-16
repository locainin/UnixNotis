//! Doctor check orchestration, output, and exit aggregation

use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{anyhow, Result};

use crate::cli::DoctorServiceManagerArg;

use super::super::checks::{inspect_bus, inspect_config, inspect_css, inspect_session_environment};
use super::super::logs::collect_logs;
use super::super::service::inspect_service_manager;
use super::model::{DoctorCheck, DoctorReport, DoctorSeverity};
use super::render::{render_human, render_json};

pub async fn run(
    json: bool,
    verbose: bool,
    requested_manager: DoctorServiceManagerArg,
    requested_config: Option<PathBuf>,
) -> Result<()> {
    let config_result = inspect_config(requested_config);
    let mut checks = config_result.checks;
    if let Some(config_report) = &config_result.report {
        // CSS checks receive the exact accepted config rather than parsing the file again
        checks.extend(inspect_css(
            &config_result.config_path,
            &config_report.config,
        ));
    } else {
        checks.push(DoctorCheck::new(
            "css.validation",
            "CSS validation",
            DoctorSeverity::Note,
            "CSS checks were skipped because configuration was rejected",
        ));
    }

    // Bus checks remain independent of config and service artifact health
    let bus_result = inspect_bus().await;
    checks.push(inspect_session_environment(bus_result.connected));
    checks.extend(bus_result.checks);

    // Service status still runs when D-Bus or CSS checks fail
    let service_result = inspect_service_manager(requested_manager, bus_result.control_owned).await;
    checks.extend(service_result.checks);

    // Missing persistent logs are represented as notes and never stop doctor
    let logs = collect_logs(service_result.selected, verbose).await;

    let report = DoctorReport::new(checks, config_result.diagnostics, logs);
    // Rendering happens before exit aggregation so failed reports remain attachable
    let rendered = if json {
        render_json(&report)?
    } else {
        render_human(&report)
    };
    // Closed pipelines are normal CLI shutdown, while other output failures remain actionable
    write_report(io::stdout().lock(), &rendered)?;

    if report.has_errors() {
        // One generic process error avoids duplicating sensitive check details on stderr
        Err(anyhow!("doctor found one or more hard failures"))
    } else {
        Ok(())
    }
}

pub(super) fn write_report(mut writer: impl Write, report: &str) -> Result<()> {
    match writeln!(writer, "{report}") {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(error.into()),
    }
}
