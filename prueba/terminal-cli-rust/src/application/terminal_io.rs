use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub struct DirectoryEntry {
    pub name: String,
    pub is_directory: bool,
}

pub trait TerminalIo {
    fn read_line(&mut self, prompt: &str) -> io::Result<Option<String>>;
    fn write_output(&mut self, text: &str) -> io::Result<()>;
    fn write_error(&mut self, text: &str) -> io::Result<()>;
    fn current_directory(&self) -> io::Result<PathBuf>;
    fn list_directory(&self, path: &Path) -> io::Result<Vec<DirectoryEntry>>;
    fn change_directory(&mut self, path: &Path) -> io::Result<()>;
    fn read_file(&self, path: &Path) -> io::Result<String>;
    fn clear_screen(&mut self) -> io::Result<()>;
}
