use serde::Serialize;
use sysinfo::Disks;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub name: String,
    pub mount_point: String,
    pub file_system: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub removable: bool,
}

/// Mounted volumes that could be an SD card: removable disks, plus anything
/// mounted under /Volumes on macOS (card readers often report non-removable).
pub fn list() -> Vec<Volume> {
    let disks = Disks::new_with_refreshed_list();
    let mut out: Vec<Volume> = disks
        .list()
        .iter()
        .filter_map(|d| {
            let mount = d.mount_point().to_string_lossy().to_string();
            let candidate = d.is_removable() || (cfg!(target_os = "macos") && mount.starts_with("/Volumes/"));
            let excluded = mount == "/"
                || mount.starts_with("/System")
                || mount.starts_with("/private")
                || mount.starts_with("/Library")
                || d.total_space() == 0;
            if !candidate || excluded {
                return None;
            }
            let name = d.name().to_string_lossy().to_string();
            let name = if name.is_empty() || name.starts_with("/dev/") {
                std::path::Path::new(&mount)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| mount.clone())
            } else {
                name
            };
            Some(Volume {
                name,
                mount_point: mount,
                file_system: d.file_system().to_string_lossy().to_string(),
                total_bytes: d.total_space(),
                available_bytes: d.available_space(),
                removable: d.is_removable(),
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out.dedup_by(|a, b| a.mount_point == b.mount_point);
    out
}

pub fn find(mount_point: &str) -> Option<Volume> {
    list().into_iter().find(|v| v.mount_point == mount_point)
}
