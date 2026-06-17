use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn sqlformat_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sqlformat"))
}

fn unique_temp_file_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "sqlformat-cli-{name}-{}-{nanos}.sql",
        std::process::id()
    ))
}

#[test]
fn exits_non_zero_and_prints_usage_for_missing_filename() {
    let output = Command::new(sqlformat_bin())
        .output()
        .expect("binary should execute");

    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage:"));
}

#[test]
fn exits_non_zero_when_input_file_does_not_exist() {
    let missing_path = unique_temp_file_path("missing");
    if missing_path.exists() {
        fs::remove_file(&missing_path).expect("pre-existing temp file should be removable");
    }

    let output = Command::new(sqlformat_bin())
        .arg(&missing_path)
        .output()
        .expect("binary should execute");

    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Error reading"));
    assert!(!missing_path.exists());
}

#[test]
fn formats_file_in_place_with_default_options() {
    let sql_path = unique_temp_file_path("format");
    let input = "SELECT count(*),Column1 FROM Table1;";
    fs::write(&sql_path, input).expect("test input file should be writable");

    let output = Command::new(sqlformat_bin())
        .arg(&sql_path)
        .output()
        .expect("binary should execute");

    assert!(output.status.success());

    let expected = sqlformat::format(
        input,
        &sqlformat::QueryParams::None,
        &sqlformat::FormatOptions::default(),
    );
    let actual = fs::read_to_string(&sql_path).expect("formatted file should be readable");
    assert_eq!(actual, expected);

    fs::remove_file(&sql_path).expect("test output file should be removable");
}
