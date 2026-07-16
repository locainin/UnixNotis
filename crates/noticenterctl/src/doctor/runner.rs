//! Doctor check orchestration and exit aggregation

use anyhow::{anyhow, Result};

use crate::cli::DoctorServiceManagerArg;

use super::config::inspect_config;
use super::css::inspect_css;
use super::dbus::inspect_bus;
use super::logs::collect_logs;
use super::model::{DoctorCheck, DoctorReport, DoctorSeverity};
use super::render::{render_human, render_json};
use super::service::inspect_service_manager;

pub async fn run(
    json: bool,
    verbose: bool,
    requested_manager: DoctorServiceManagerArg,
) -> Result<()> {
    let config_result = inspect_config();
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
    checks.extend(bus_result.checks);

    // Service status still runs when D-Bus or CSS checks fail
    let service_result = inspect_service_manager(requested_manager, bus_result.control_owned).await;
    checks.extend(service_result.checks);

    // Missing persistent logs are represented as notes and never stop doctor
    let (logs, log_check) = collect_logs(service_result.selected, verbose).await;
    checks.push(log_check);

    let report = DoctorReport::new(checks, logs);
    // Rendering happens before exit aggregation so failed reports remain attachable
    if json {
        println!("{}", render_json(&report)?);
    } else {
        println!("{}", render_human(&report));
    }

    if report.has_errors() {
        // One generic process error avoids duplicating sensitive check details on stderr
        Err(anyhow!("doctor found one or more hard failures"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/runner.rs"]
mod tests;
