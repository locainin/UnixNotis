//! Secure preparation, application, rollback, and commit

pub(in crate::preset) mod apply;
pub(in crate::preset) mod commit;
pub(in crate::preset) mod plan;
pub(in crate::preset) mod prepare;

#[cfg(test)]
mod tests;
