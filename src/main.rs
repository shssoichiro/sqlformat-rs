use std::{env, fs, process};

fn run() -> Result<(), String> {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| String::from("sqlformat"));

    let filename = match (args.next(), args.next()) {
        (Some(filename), None) => filename,
        _ => return Err(format!("Usage: {program} <filename>")),
    };

    let input = fs::read_to_string(&filename)
        .map_err(|err| format!("Error reading '{filename}': {err}"))?;

    let formatted = sqlformat::format(
        &input,
        &sqlformat::QueryParams::None,
        &sqlformat::FormatOptions::default(),
    );

    fs::write(&filename, formatted).map_err(|err| format!("Error writing '{filename}': {err}"))?;

    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        process::exit(1);
    }
}
