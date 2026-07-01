//! Trial-run modules for local daemon testing without installing service files

mod build;
mod launch;
mod paths;
mod shim;

pub(crate) use launch::run_trial;

#[cfg(test)]
#[path = "tests/build.rs"]
mod build_tests;
#[cfg(test)]
#[path = "tests/launch.rs"]
mod launch_tests;
#[cfg(test)]
#[path = "tests/paths.rs"]
mod paths_tests;
#[cfg(test)]
#[path = "tests/shim.rs"]
mod shim_tests;
#[cfg(test)]
#[path = "tests/support.rs"]
mod test_support;
