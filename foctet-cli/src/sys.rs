use std::time::Duration;

#[cfg(target_os = "windows")]
pub fn get_os_type() -> String {
    "windows".to_owned()
}

#[cfg(target_os = "linux")]
pub fn get_os_type() -> String {
    "linux".to_owned()
}

#[cfg(target_os = "macos")]
pub fn get_os_type() -> String {
    "macos".to_owned()
}

pub fn get_sysdate() -> String {
    let now = chrono::Local::now();
    now.to_rfc3339()
}

pub fn get_systime() -> String {
    let now = chrono::Local::now();
    now.format("%H:%M:%S").to_string()
}

pub fn ceil_duration_millis(duration: Duration) -> Duration {
    let millis = duration.as_millis();
    if millis % 1000 == 0 {
        duration
    } else {
        Duration::from_millis(millis as u64 + 1)
    }
}
