use std::process::Command;

/// Gets the current system volume (0-100)
fn get_system_volume() -> Result<u32, String> {
    let output = Command::new("osascript")
        .args(["-e", "output volume of (get volume settings)"])
        .output()
        .map_err(|e| format!("Failed to get volume: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "osascript error: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let volume_str = String::from_utf8_lossy(&output.stdout);
    volume_str
        .trim()
        .parse::<u32>()
        .map_err(|e| format!("Failed to parse volume: {}", e))
}

/// Sets the system volume (0-100)
fn set_system_volume(volume: u32) -> Result<(), String> {
    let volume = volume.clamp(0, 100);

    let output = Command::new("osascript")
        .args(["-e", &format!("set volume output volume {}", volume)])
        .output()
        .map_err(|e| format!("Failed to set volume: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "osascript error: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}

/// Public function to mute system audio
/// Returns the previous volume so it can be restored
pub fn mute_system_audio() -> Result<u32, String> {
    let previous_volume = get_system_volume()?;

    // Set volume to 0 to effectively mute
    set_system_volume(0)?;

    Ok(previous_volume)
}

/// Restores the volume captured by `mute_system_audio`.
///
/// Restores exactly what was read. An earlier version floored this at 30,
/// which turned a deliberately-quiet Mac loud after every dictation.
pub fn unmute_system_audio(previous_volume: u32) -> Result<(), String> {
    set_system_volume(previous_volume)
}
