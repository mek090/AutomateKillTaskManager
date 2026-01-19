use serde::{Deserialize, Serialize};
use sysinfo::{Disks, System, Signal};
use std::sync::Mutex;
use std::fs;
use std::path::PathBuf;
use chrono::Local;

// ============= Data Structures =============

#[derive(Serialize)]
pub struct ProcRow {
    pid: u32,
    name: String,
    cpu: f32,
    memory_kb: u64,
}

#[derive(Serialize, Clone)]
pub struct ProcessGroup {
    name: String,
    process_count: u32,
    pids: Vec<u32>,
    total_cpu: f32,
    total_memory_kb: u64,
}

#[derive(Serialize)]
pub struct SystemStats {
    cpu_usage: f32,
    memory_total_gb: f64,
    memory_used_gb: f64,
    memory_percent: f32,
    disks: Vec<DiskInfo>,
}

#[derive(Serialize)]
pub struct DiskInfo {
    name: String,
    mount_point: String,
    total_gb: f64,
    used_gb: f64,
    free_gb: f64,
    usage_percent: f32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BlacklistEntry {
    pub name: String,
    pub auto_kill: bool,
    pub cpu_threshold: f32,  // Kill only when CPU > this value (0 = always kill)
    #[serde(default = "default_true")]
    pub log_enabled: bool,
    pub created_at: String,
    pub kill_count: u32,
}

fn default_true() -> bool { true }

#[derive(Serialize, Deserialize, Clone)]
pub struct ActivityLog {
    pub name: String,
    pub pid: u32,
    pub cpu_usage: f32,
    pub detected_at: String,
    pub was_killed: bool,
    pub reason: String,  // "CPU threshold exceeded" or "Detected"
}

#[derive(Serialize, Deserialize, Default)]
pub struct AppState {
    pub blacklist: Vec<BlacklistEntry>,
    pub activity_logs: Vec<ActivityLog>,
}

// Global state
static APP_STATE: Mutex<Option<AppState>> = Mutex::new(None);

fn get_data_path() -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("tauri-app");
    fs::create_dir_all(&path).ok();
    path.push("blacklist_data.json");
    path
}

fn load_state() -> AppState {
    let path = get_data_path();
    if path.exists() {
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(state) = serde_json::from_str(&data) {
                return state;
            }
        }
    }
    AppState::default()
}

fn save_state(state: &AppState) {
    let path = get_data_path();
    if let Ok(data) = serde_json::to_string_pretty(state) {
        fs::write(path, data).ok();
    }
}

fn with_state<F, R>(f: F) -> R
where
    F: FnOnce(&mut AppState) -> R,
{
    let mut guard = APP_STATE.lock().unwrap();
    if guard.is_none() {
        *guard = Some(load_state());
    }
    let state = guard.as_mut().unwrap();
    let result = f(state);
    save_state(state);
    result
}

// ============= System Stats Commands =============

#[tauri::command]
fn get_system_stats() -> SystemStats {
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu_usage = sys.global_cpu_usage();
    let memory_total = sys.total_memory();
    let memory_used = sys.used_memory();
    let memory_total_gb = memory_total as f64 / 1024.0 / 1024.0 / 1024.0;
    let memory_used_gb = memory_used as f64 / 1024.0 / 1024.0 / 1024.0;
    let memory_percent = if memory_total > 0 {
        (memory_used as f32 / memory_total as f32) * 100.0
    } else {
        0.0
    };

    let disks = Disks::new_with_refreshed_list();
    let disk_info: Vec<DiskInfo> = disks
        .iter()
        .map(|d| {
            let total = d.total_space();
            let free = d.available_space();
            let used = total.saturating_sub(free);
            DiskInfo {
                name: d.name().to_string_lossy().to_string(),
                mount_point: d.mount_point().to_string_lossy().to_string(),
                total_gb: total as f64 / 1024.0 / 1024.0 / 1024.0,
                used_gb: used as f64 / 1024.0 / 1024.0 / 1024.0,
                free_gb: free as f64 / 1024.0 / 1024.0 / 1024.0,
                usage_percent: if total > 0 { (used as f32 / total as f32) * 100.0 } else { 0.0 },
            }
        })
        .collect();

    SystemStats {
        cpu_usage,
        memory_total_gb,
        memory_used_gb,
        memory_percent,
        disks: disk_info,
    }
}

// ============= Process Watching Commands =============

#[tauri::command]
fn watched_processes(names: Vec<String>) -> Vec<ProcRow> {
    let watch: Vec<String> = names
        .into_iter()
        .map(|s| resolve_process_name(&s))
        .filter(|s| !s.is_empty())
        .collect();

    if watch.is_empty() {
        return vec![];
    }

    let mut sys = System::new_all();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    
    let cpu_count = sys.cpus().len() as f32;
    let cpu_count = if cpu_count > 0.0 { cpu_count } else { 1.0 };

    sys.processes()
        .iter()
        .filter_map(|(pid, p)| {
            let pname = p.name().to_string_lossy().to_lowercase();
            if watch.iter().any(|w| pname.contains(w) || w == &pname) {
                let normalized_cpu = p.cpu_usage() / cpu_count;
                Some(ProcRow {
                    pid: pid.as_u32(),
                    name: p.name().to_string_lossy().to_string(),
                    cpu: normalized_cpu,
                    memory_kb: p.memory() / 1024,
                })
            } else {
                None
            }
        })
        .collect()
}

#[tauri::command]
fn kill_pid(pid: u32) -> Result<String, String> {
    let mut sys = System::new_all();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let pid = sysinfo::Pid::from_u32(pid);
    let process = sys.process(pid).ok_or("Process not found")?;

    let ok = process.kill_with(Signal::Term).unwrap_or(false) || process.kill();

    if ok {
        Ok(format!("PID {} terminated", pid.as_u32()))
    } else {
        Err("Failed to kill (permission denied?)".into())
    }
}

/// Kill all processes in a group by name
#[tauri::command]
fn kill_process_group(name: String) -> Result<String, String> {
    let mut sys = System::new_all();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    
    let name_lower = name.to_lowercase();
    let mut killed_count = 0;
    let mut failed_count = 0;
    
    for (pid, p) in sys.processes().iter() {
        let pname = p.name().to_string_lossy().to_lowercase();
        if pname.contains(&name_lower) || pname == name_lower {
            let ok = p.kill_with(Signal::Term).unwrap_or(false) || p.kill();
            if ok {
                killed_count += 1;
            } else {
                failed_count += 1;
            }
        }
    }
    
    if killed_count > 0 {
        Ok(format!("Killed {} processes, {} failed", killed_count, failed_count))
    } else if failed_count > 0 {
        Err(format!("Failed to kill {} processes (permission denied?)", failed_count))
    } else {
        Err("No matching processes found".into())
    }
}

/// Get processes grouped by name (like Task Manager)
#[tauri::command]
fn grouped_processes(names: Vec<String>) -> Vec<ProcessGroup> {
    use std::collections::HashMap;
    
    let watch: Vec<String> = names
        .into_iter()
        .map(|s| resolve_process_name(&s))
        .filter(|s| !s.is_empty())
        .collect();

    if watch.is_empty() {
        return vec![];
    }

    let mut sys = System::new_all();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    
    let cpu_count = sys.cpus().len() as f32;
    let cpu_count = if cpu_count > 0.0 { cpu_count } else { 1.0 };
    
    // Group processes by base name (without .exe)
    let mut groups: HashMap<String, ProcessGroup> = HashMap::new();
    
    for (pid, p) in sys.processes().iter() {
        let pname = p.name().to_string_lossy().to_string();
        let pname_lower = pname.to_lowercase();
        
        if watch.iter().any(|w| pname_lower.contains(w) || w == &pname_lower) {
            let normalized_cpu = p.cpu_usage() / cpu_count;
            let memory_kb = p.memory() / 1024;
            
            // Use the original name as key (preserves case)
            let base_name = pname.clone();
            
            let group = groups.entry(base_name.clone()).or_insert(ProcessGroup {
                name: base_name,
                process_count: 0,
                pids: vec![],
                total_cpu: 0.0,
                total_memory_kb: 0,
            });
            
            group.process_count += 1;
            group.pids.push(pid.as_u32());
            group.total_cpu += normalized_cpu;
            group.total_memory_kb += memory_kb;
        }
    }
    
    // Convert to vec and sort by CPU usage (highest first)
    let mut result: Vec<ProcessGroup> = groups.into_values().collect();
    result.sort_by(|a, b| b.total_cpu.partial_cmp(&a.total_cpu).unwrap_or(std::cmp::Ordering::Equal));
    result
}

// ============= Blacklist Commands =============

#[tauri::command]
fn get_blacklist() -> Vec<BlacklistEntry> {
    with_state(|state| state.blacklist.clone())
}

#[tauri::command]
fn add_to_blacklist(name: String, auto_kill: bool, cpu_threshold: f32) -> Result<String, String> {
    let name = resolve_process_name(&name);
    if name.is_empty() {
        return Err("Name cannot be empty".into());
    }

    with_state(|state| {
        if state.blacklist.iter().any(|e| e.name.to_lowercase() == name.to_lowercase()) {
            return Err("Already in blacklist".into());
        }

        state.blacklist.push(BlacklistEntry {
            name: name.clone(),
            auto_kill,
            cpu_threshold,
            log_enabled: true,
            created_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            kill_count: 0,
        });
        Ok(format!("{} added to blacklist", name))
    })
}

#[tauri::command]
fn remove_from_blacklist(name: String) -> Result<String, String> {
    with_state(|state| {
        let len_before = state.blacklist.len();
        state.blacklist.retain(|e| e.name.to_lowercase() != name.to_lowercase());
        if state.blacklist.len() < len_before {
            Ok(format!("{} removed from blacklist", name))
        } else {
            Err("Not found in blacklist".into())
        }
    })
}

#[tauri::command]
fn toggle_auto_kill(name: String) -> Result<bool, String> {
    with_state(|state| {
        for entry in state.blacklist.iter_mut() {
            if entry.name.to_lowercase() == name.to_lowercase() {
                entry.auto_kill = !entry.auto_kill;
                return Ok(entry.auto_kill);
            }
        }
        Err("Not found in blacklist".into())
    })
}

#[tauri::command]
fn toggle_blacklist_log(name: String) -> Result<bool, String> {
    with_state(|state| {
        for entry in state.blacklist.iter_mut() {
            if entry.name.to_lowercase() == name.to_lowercase() {
                entry.log_enabled = !entry.log_enabled;
                return Ok(entry.log_enabled);
            }
        }
        Err("Not found in blacklist".into())
    })
}

#[tauri::command]
fn set_cpu_threshold(name: String, threshold: f32) -> Result<f32, String> {
    with_state(|state| {
        for entry in state.blacklist.iter_mut() {
            if entry.name.to_lowercase() == name.to_lowercase() {
                entry.cpu_threshold = threshold.max(0.0).min(100.0);
                return Ok(entry.cpu_threshold);
            }
        }
        Err("Not found in blacklist".into())
    })
}

#[tauri::command]
fn get_activity_logs() -> Vec<ActivityLog> {
    with_state(|state| {
        // Return last 100 logs, newest first
        let mut logs = state.activity_logs.clone();
        logs.reverse();
        logs.truncate(100);
        logs
    })
}

#[tauri::command]
fn clear_activity_logs() -> String {
    with_state(|state| {
        state.activity_logs.clear();
        "Logs cleared".to_string()
    })
}

#[tauri::command]
fn check_and_kill_blacklist() -> Vec<ActivityLog> {
    let mut sys = System::new_all();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    
    // Get CPU count for normalization
    let cpu_count = sys.cpus().len() as f32;
    let cpu_count = if cpu_count > 0.0 { cpu_count } else { 1.0 };

    let mut new_logs: Vec<ActivityLog> = vec![];

    with_state(|state| {
        let blacklist_info: Vec<(String, bool, f32, bool)> = state.blacklist.iter()
            .map(|e| (e.name.to_lowercase(), e.auto_kill, e.cpu_threshold, e.log_enabled))
            .collect();

        for (pid, p) in sys.processes().iter() {
            let pname = p.name().to_string_lossy().to_lowercase();
            let process_cpu = p.cpu_usage() / cpu_count;  // Normalized CPU

            for (bl_name, auto_kill, cpu_threshold, log_enabled) in &blacklist_info {
                if pname.contains(bl_name) || bl_name == &pname {
                    // Check if CPU exceeds threshold (0 = always kill)
                    let should_kill = *auto_kill && (*cpu_threshold <= 0.0 || process_cpu >= *cpu_threshold);
                    
                    let (was_killed, reason) = if should_kill {
                        let killed = p.kill_with(Signal::Term).unwrap_or(false) || p.kill();
                        if killed {
                            if let Some(entry) = state.blacklist.iter_mut()
                                .find(|e| e.name.to_lowercase() == *bl_name) {
                                entry.kill_count += 1;
                            }
                            (true, format!("Killed (CPU: {:.1}%)", process_cpu))
                        } else {
                            (false, "Kill failed (no permission)".to_string())
                        }
                    } else if *auto_kill && process_cpu < *cpu_threshold {
                        (false, format!("CPU {:.1}% < threshold {:.0}%", process_cpu, cpu_threshold))
                    } else {
                        (false, "Detected".to_string())
                    };

                    let log = ActivityLog {
                        name: p.name().to_string_lossy().to_string(),
                        pid: pid.as_u32(),
                        cpu_usage: process_cpu,
                        detected_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                        was_killed,
                        reason,
                    };

                    if *log_enabled {
                        new_logs.push(log.clone());
                        state.activity_logs.push(log);
                    }
                    break;
                }
            }
        }

        // Keep only last 1000 logs
        if state.activity_logs.len() > 1000 {
            state.activity_logs = state.activity_logs.split_off(state.activity_logs.len() - 1000);
        }
    });

    new_logs
}

fn resolve_process_name(input: &str) -> String {
    let s = input.trim().to_lowercase();
    match s.as_str() {
        "edge" | "microsoft edge" | "msedge" => "msedge".to_string(),
        "chrome" | "google chrome" => "chrome".to_string(),
        "code" | "vscode" | "vs code" => "code".to_string(),
        "calc" | "calculator" => "calculator".to_string(),
        "notepad" => "notepad".to_string(),
        "task manager" | "taskmgr" => "taskmgr".to_string(),
        "cmd" | "command prompt" => "cmd".to_string(),
        "powershell" => "powershell".to_string(),
        _ => s,
    }
}

// ============= App Entry =============

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            watched_processes,
            grouped_processes,
            kill_pid,
            kill_process_group,
            get_system_stats,
            get_blacklist,
            add_to_blacklist,
            remove_from_blacklist,
            toggle_auto_kill,
            toggle_blacklist_log,
            set_cpu_threshold,
            get_activity_logs,
            clear_activity_logs,
            check_and_kill_blacklist
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
