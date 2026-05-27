use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::PortCliError;

fn get_project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("", "", "portcli").context("could not determine project directories")
}

pub fn get_log_dir() -> Result<PathBuf> {
    Ok(get_project_dirs()?.data_local_dir().join("logs"))
}

pub fn get_daemon_log_path() -> Result<PathBuf> {
    Ok(get_log_dir()?.join("daemon.log"))
}

pub fn get_rule_log_path(name: &str) -> Result<PathBuf> {
    Ok(get_log_dir()?.join("rules").join(format!("{}.log", name)))
}

pub fn ensure_log_dir() -> Result<()> {
    let dir = get_log_dir()?;
    fs::create_dir_all(&dir)?;
    fs::create_dir_all(dir.join("rules"))?;
    Ok(())
}

pub fn append_log(path: &Path, level: &str, message: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let line = format!("[{}] {} {}\n", timestamp, level, message);
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(line.as_bytes())?;
    Ok(())
}

pub fn read_last_lines(path: &Path, n: usize) -> Result<Vec<String>> {
    if !path.exists() {
        return Err(PortCliError::LogFileNotFound(path.display().to_string()).into());
    }
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines().map_while(Result::ok).collect();
    let start = if lines.len() > n { lines.len() - n } else { 0 };
    Ok(lines[start..].to_vec())
}

pub fn clear_log(path: &Path) -> Result<()> {
    if path.exists() {
        fs::write(path, "")?;
    }
    Ok(())
}

pub fn follow_log(path: &Path, initial_lines: usize) -> Result<()> {
    if !path.exists() {
        return Err(PortCliError::LogFileNotFound(path.display().to_string()).into());
    }

    let lines = read_last_lines(path, initial_lines)?;
    for line in &lines {
        println!("{}", line);
    }

    let mut file = fs::File::open(path)?;
    let mut current_size = file.metadata()?.len();
    file.seek(SeekFrom::End(0))?;

    let poll_interval = Duration::from_millis(500);
    let mut buf = String::new();

    while let Ok(metadata) = fs::metadata(path) {
        let new_size = metadata.len();

        if new_size < current_size {
            current_size = 0;
            file.seek(SeekFrom::Start(0))?;
        }

        if new_size > current_size {
            let to_read = (new_size - current_size) as usize;
            let mut chunk = vec![0u8; to_read];
            file.read_exact(&mut chunk)?;

            buf.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(pos) = buf.find('\n') {
                let line = buf[..=pos].trim_end().to_string();
                println!("{}", line);
                buf = buf[pos + 1..].to_string();
            }

            current_size = new_size;
        }

        std::thread::sleep(poll_interval);
    }

    Ok(())
}
