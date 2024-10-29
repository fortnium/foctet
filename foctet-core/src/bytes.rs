pub const KB: usize = 1024;
pub const MB: usize = KB * 1024;
pub const GB: usize = MB * 1024;

pub fn format_bytes(bytes: usize) -> String {
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
