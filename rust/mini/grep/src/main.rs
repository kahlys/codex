use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};

use clap::Parser;
use colored::Colorize;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Substring to search for
    #[arg(short, long)]
    str: String,

    /// Path to the file to read
    #[arg(short, long)]
    path: String,
}

/// Searches for lines containing a substring in a file and prints them with line numbers.
///
/// # Arguments
/// * `s` - The substring to search for.
/// * `path` - The path to the file to search.
///
/// # Errors
/// Returns an error if the file cannot be opened or read.
fn grep_str(s: &str, path: &str) -> Result<(), Box<dyn Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    // let file_content: String = fs::read_to_string(args.path)?;
    let lines = reader
        .lines()
        .enumerate()
        .filter_map(|(i, line)| match line {
            Ok(l) if l.contains(&s) => Some((i + 1, l)),
            _ => None,
        });

    for t in lines {
        println!(
            "{} : {}",
            t.0.to_string().blue(),
            t.1.replacen(&s, &s.red().bold().to_string(), 1)
        )
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    grep_str(&args.str, &args.path)?;

    Ok(())
}
