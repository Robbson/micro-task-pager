//! Minimal file logger.
//!
//! Logs are written to `%LOCALAPPDATA%\MicroTaskPager\logs`, one file per day,
//! matching the path the C# application uses. Files older than 14 days are
//! removed during [`init`]. The logger is deliberately small: it opens the file
//! for each message, so there is no background writer thread and no shared
//! mutable state. The application logs infrequently, so the extra open call is
//! not a performance concern.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use windows::Win32::Foundation::SYSTEMTIME;
use windows::Win32::System::SystemInformation::GetLocalTime;

const RETENTION: Duration = Duration::from_secs(14 * 24 * 60 * 60);

/// Creates the log directory, removes expired files, and writes a session
/// marker. Call once at process startup.
pub fn init() {
    let directory = log_directory();
    let _ = fs::create_dir_all(&directory);
    purge_old_files(&directory);
    info("logging initialized");
}

/// Writes an informational message.
pub fn info(message: impl AsRef<str>) {
    write("INFO", message.as_ref());
}

/// Writes a warning.
pub fn warn(message: impl AsRef<str>) {
    write("WARN", message.as_ref());
}

/// Writes an error.
pub fn error(message: impl AsRef<str>) {
    write("ERROR", message.as_ref());
}

/// Appends one line to today's log file. Failures are ignored because logging
/// must never take down the application.
fn write(level: &str, message: &str) {
    let directory = log_directory();
    let _ = fs::create_dir_all(&directory);

    let now = local_time();
    let file_name = format!("log-{:04}-{:02}-{:02}.txt", now.wYear, now.wMonth, now.wDay);

    let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join(file_name))
    else {
        return;
    };

    let _ = writeln!(
        file,
        "[{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}] [{}] {}",
        now.wYear,
        now.wMonth,
        now.wDay,
        now.wHour,
        now.wMinute,
        now.wSecond,
        now.wMilliseconds,
        level,
        message
    );
}

/// Returns the directory used by both this logger and the C# application.
fn log_directory() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("MicroTaskPager")
        .join("logs")
}

/// Reads the local time through Win32 so the log does not need a date/time
/// dependency.
fn local_time() -> SYSTEMTIME {
    unsafe { GetLocalTime() }
}

/// Deletes log files whose modification time is older than the retention
/// window. File age is used instead of parsing the name, so the rule also
/// applies to logs written by the C# application.
fn purge_old_files(directory: &Path) {
    let Some(cutoff) = SystemTime::now().checked_sub(RETENTION) else {
        return;
    };

    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        if modified < cutoff {
            let _ = fs::remove_file(entry.path());
        }
    }
}
