//! Protected shell-launcher inspection and runtime binding

mod binding;
mod read;
mod syntax;
mod validation;

pub(super) use binding::{inspect_package_shell_launcher, launcher_binding_is_current};

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
