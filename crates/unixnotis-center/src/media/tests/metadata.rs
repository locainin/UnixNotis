use std::collections::HashMap;

use zbus::zvariant::OwnedValue;

use super::metadata_pid;

#[test]
fn metadata_pid_reads_unsigned_kde_pid() {
    let mut metadata = HashMap::new();
    metadata.insert("kde:pid".to_string(), OwnedValue::from(103_380_u32));

    assert_eq!(metadata_pid(&metadata), Some(103_380));
}

#[test]
fn metadata_pid_rejects_negative_kde_pid() {
    let mut metadata = HashMap::new();
    metadata.insert("kde:pid".to_string(), OwnedValue::from(-1_i32));

    assert_eq!(metadata_pid(&metadata), None);
}
