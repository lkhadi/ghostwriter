use crate::screen_info;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::Duration;

const SOCKET_PATH: &str = "/tmp/ghostwriter_overlay.sock";

pub struct OverlayHelper {
    _process: Mutex<Option<Child>>,
}

impl OverlayHelper {
    fn get_resource_path(rel_path: &str) -> PathBuf {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(macos_dir) = exe_path.parent() {
                if let Some(contents_dir) = macos_dir.parent() {
                    let resources_dir = contents_dir.join("Resources");
                    let full_path = resources_dir.join(rel_path);
                    if full_path.exists() {
                        return full_path;
                    }
                }
            }
        }
        PathBuf::from(rel_path)
    }

    pub fn new() -> Result<Self, String> {
        Self::stop_existing();

        let current_dir = std::env::current_dir().ok();
        if let Some(ref dir) = current_dir {
            println!("Current dir: {}", dir.display());
        }

        let helper_paths = [
            // Production: bundled inside Resources/overlay-helper/ (no ../ traversal)
            Self::get_resource_path("overlay-helper/GhostWriterOverlayHelper.app/Contents/MacOS/GhostWriterOverlayHelper"),
            // Dev: relative to project working directory
            std::path::PathBuf::from("../overlay-helper/GhostWriterOverlayHelper.app/Contents/MacOS/GhostWriterOverlayHelper"),
            std::path::PathBuf::from("./overlay-helper/GhostWriterOverlayHelper.app/Contents/MacOS/GhostWriterOverlayHelper"),
        ];

        println!("Searching for helper app...");
        let mut launched = false;
        for helper_path in helper_paths.iter() {
            let path_str = helper_path.to_string_lossy();
            let exists = helper_path.exists();
            println!("Checking path: {}, exists: {}", path_str, exists);
            if exists {
                println!("Launching helper from: {}", path_str);
                if Self::launch_helper(helper_path).is_ok() {
                    launched = true;
                    break;
                }
            }
        }

        if !launched {
            return Err("Helper app not found in any expected location".to_string());
        }

        let mut attempts = 0;
        while attempts < 50 {
            if Path::new(SOCKET_PATH).exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
            attempts += 1;
        }

        if !Path::new(SOCKET_PATH).exists() {
            return Err("Helper app failed to start (socket not available)".to_string());
        }

        Ok(Self {
            _process: Mutex::new(None),
        })
    }

    fn launch_helper(path: &std::path::Path) -> Result<Child, String> {
        Command::new(path)
            .spawn()
            .map_err(|e| format!("Failed to launch helper: {}", e))
    }

    fn stop_existing() {
        // Send QUIT command gracefully
        if let Ok(mut stream) = UnixStream::connect(SOCKET_PATH) {
            let _ = writeln!(stream, "QUIT");
            let _ = stream.flush();
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }

        // Wait for graceful shutdown
        std::thread::sleep(Duration::from_millis(500));

        // Force kill if still running (cleanup zombie processes)
        let _ = Command::new("pkill")
            .args(["-9", "-f", "GhostWriterOverlayHelper"])
            .status();

        // Remove socket
        let _ = std::fs::remove_file(SOCKET_PATH);

        // Wait to ensure clean shutdown
        std::thread::sleep(Duration::from_millis(200));
    }

    pub fn show(&self, x: i32, y: i32) -> Result<(), String> {
        self.send_command(&format!("SHOW {} {}", x, y))
    }

    pub fn show_centered_bottom(&self) -> Result<(), String> {
        // Calculate center position near bottom of screen
        let screen_width = screen_info::get_screen_width();
        let screen_height = screen_info::get_screen_height();

        // Overlay dimensions
        let overlay_width = 220;
        let overlay_height = 60;

        // Center horizontally, position near bottom (with 100px margin)
        let x = (screen_width - overlay_width) / 2;
        let y = screen_height - overlay_height - 100;

        self.send_command(&format!("SHOW {} {}", x, y))
    }

    pub fn hide(&self) -> Result<(), String> {
        self.send_command("HIDE")
    }

    pub fn set_window_level(&self, level: &str) -> Result<(), String> {
        let cmd = format!("SET_LEVEL {}", level);
        self.send_command(&cmd)
    }

    fn send_command(&self, command: &str) -> Result<(), String> {
        let mut stream = UnixStream::connect(SOCKET_PATH)
            .map_err(|e| format!("Failed to connect to helper: {}", e))?;

        stream
            .write_all(command.as_bytes())
            .map_err(|e| format!("Failed to send command: {}", e))?;

        let _ = stream.flush(); // Ensure command is sent immediately

        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        reader
            .read_line(&mut response)
            .map_err(|e| format!("Failed to read response: {}", e))?;

        if response.trim() != "OK" {
            return Err(format!("Unexpected response: {}", response.trim()));
        }

        Ok(())
    }
}

impl Drop for OverlayHelper {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(SOCKET_PATH);
    }
}
