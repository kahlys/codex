// Lazy fallback - call expensive function only when needed

use std::thread;

pub fn get_user_settings(key: &str) -> Option<String> {
    match key {
        "theme" => Some("dark".to_string()),
        _ => None,
    }
}

pub fn calculate_default() -> String {
    thread::sleep(std::time::Duration::from_secs(2));
    "default".to_string()
}

#[cfg(test)]
mod test_lazy_fb {
    use super::*;

    #[test]
    fn ex_lazy_fb() {
        let theme_ok = get_user_settings("theme").unwrap_or_else(calculate_default);
        let theme_default = get_user_settings("unknown").unwrap_or_else(calculate_default);

        assert_eq!(theme_ok, "dark");
        assert_eq!(theme_default, "default");
    }
}

// Iterators

pub fn sum_of_odd_numbers() -> i32 {
    let data = ["1", "2", "3", "4", "5", "6", "seven", "9"];
    data.iter()
        .filter_map(|s| s.parse::<i32>().ok())
        .filter(|s| s % 2 == 1)
        .sum()
}

#[cfg(test)]
mod test_iterator {
    use super::*;

    #[test]
    fn ex_iterator() {
        assert_eq!(sum_of_odd_numbers(), 18);
    }
}

// From Trait

#[derive(Debug, PartialEq)]
pub enum AppError {
    BadFormat,
    FileError,
}

impl From<std::io::Error> for AppError {
    fn from(_error: std::io::Error) -> Self {
        AppError::FileError
    }
}

impl From<std::num::ParseIntError> for AppError {
    fn from(_error: std::num::ParseIntError) -> Self {
        AppError::BadFormat
    }
}

pub fn read_config(f: i32) -> Result<i32, std::io::Error> {
    match f {
        0 => Err(std::io::Error::other("Failed to read config")),
        1 => Ok(1),
        _ => Ok(2),
    }
}

pub fn parse_config(c: i32) -> Result<(), std::num::ParseIntError> {
    match c {
        0 | 1 => Err("Failed to parse config".parse::<i32>().unwrap_err()),
        _ => Ok(()),
    }
}

pub fn process_config(f: i32) -> Result<(), AppError> {
    let c = read_config(f)?;
    parse_config(c)?;
    Ok(())
}

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod test_from_trait {
    use super::*;

    #[test]
    fn ex_from_trait() {
        let f_ok = 2;
        let f_err_parse = 1;
        let f_err_file = 0;

        assert_eq!(process_config(f_ok), Ok(()));
        assert_eq!(process_config(f_err_parse), Err(AppError::BadFormat));
        assert_eq!(process_config(f_err_file), Err(AppError::FileError));
    }
}
