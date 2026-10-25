use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::application::{DirectoryEntry, TerminalIo};

pub struct Console;

impl TerminalIo for Console {
    fn read_line(&mut self, prompt: &str) -> io::Result<Option<String>> {
        print!("{prompt}");
        io::stdout().flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            return Ok(None);
        }

        Ok(Some(input.trim_end().to_owned()))
    }

    fn write_output(&mut self, text: &str) -> io::Result<()> {
        print!("{text}");
        io::stdout().flush()
    }

    fn write_error(&mut self, text: &str) -> io::Result<()> {
        eprint!("{text}");
        io::stderr().flush()
    }

    fn current_directory(&self) -> io::Result<PathBuf> {
        std::env::current_dir()
    }

    fn list_directory(&self, path: &Path) -> io::Result<Vec<DirectoryEntry>> {
        let mut entries = fs::read_dir(path)?
            .map(|entry| {
                let entry = entry?;
                Ok(DirectoryEntry {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    is_directory: entry.file_type()?.is_dir(),
                })
            })
            .collect::<io::Result<Vec<_>>>()?;

        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(entries)
    }

    fn change_directory(&mut self, path: &Path) -> io::Result<()> {
        std::env::set_current_dir(path)
    }

    fn read_file(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }

    fn clear_screen(&mut self) -> io::Result<()> {
        print!("\x1B[2J\x1B[H");
        io::stdout().flush()
    }
}
