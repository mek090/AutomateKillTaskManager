use std::collections::HashMap;
use std::sync::Mutex;

#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhOpenQueryW, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE,
};

pub struct GpuMonitor {
    #[cfg(windows)]
    query: isize,
    #[cfg(windows)]
    counter: isize,
    #[cfg(windows)]
    initialized: bool,
}

// Global instance to maintain state (PDH requires state for rate counters)
static GPU_MONITOR: Mutex<Option<GpuMonitor>> = Mutex::new(None);

impl GpuMonitor {
    pub fn new() -> Self {
        #[cfg(windows)]
        unsafe {
            let mut query = 0;
            let mut counter = 0;

            // Open Query (0 = ERROR_SUCCESS)
            if PdhOpenQueryW(None, 0, &mut query) != 0 {
                return Self {
                    query: 0,
                    counter: 0,
                    initialized: false,
                };
            }

            // Add English Counter for GPU Utilization
            // This covers all engines (3D, Video, Copy, etc.) for all processes
            // The wildcard * automatically handles new processes
            let path = "BUFFER_PLACEHOLDER"; // Will encode utf16 locally
                                             // We use PdhAddEnglishCounterW to support non-English Windows
                                             // Path: \GPU Engine(*)\Utilization Percentage
            let mut path_utf16: Vec<u16> = "\\GPU Engine(*)\\Utilization Percentage"
                .encode_utf16()
                .collect();
            path_utf16.push(0);

            let result = PdhAddEnglishCounterW(query, PCWSTR(path_utf16.as_ptr()), 0, &mut counter);

            if result != 0 {
                let _ = PdhCloseQuery(query);
                return Self {
                    query: 0,
                    counter: 0,
                    initialized: false,
                };
            }

            // Initial collect
            let _ = PdhCollectQueryData(query);

            Self {
                query,
                counter,
                initialized: true,
            }
        }

        #[cfg(not(windows))]
        Self {}
    }

    #[cfg(windows)]
    pub fn get_usage(&self) -> HashMap<u32, f32> {
        if !self.initialized {
            return HashMap::new();
        }

        let mut usage_map: HashMap<u32, f32> = HashMap::new();

        unsafe {
            // Collect new data
            if PdhCollectQueryData(self.query) != 0 {
                return usage_map;
            }

            // Prepare to get array
            let mut buffer_size = 0;
            let mut item_count = 0;

            // First call to get buffer size
            // We expect PdhGetFormattedCounterArrayW to return PDH_MORE_DATA (which is non-zero)
            let _ = PdhGetFormattedCounterArrayW(
                self.counter,
                PDH_FMT_DOUBLE,
                &mut buffer_size,
                &mut item_count,
                None,
            );

            if buffer_size == 0 {
                return usage_map;
            }

            let mut buffer = vec![0u8; buffer_size as usize];
            let items_ptr = buffer.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;

            // Second call to get actual data
            if PdhGetFormattedCounterArrayW(
                self.counter,
                PDH_FMT_DOUBLE,
                &mut buffer_size,
                &mut item_count,
                Some(items_ptr),
            ) == 0
            {
                // Iterate over items
                for i in 0..item_count {
                    let item = *items_ptr.offset(i as isize);
                    let name_ptr = item.szName.0;
                    if !name_ptr.is_null() {
                        let name_len = (0..).take_while(|&i| *name_ptr.offset(i) != 0).count();
                        let name_slice = std::slice::from_raw_parts(name_ptr, name_len);
                        let name = String::from_utf16_lossy(name_slice);

                        // Parse instance name to get PID
                        // Format example: "pid_1234_engtype_3D_eng_0"
                        if let Some(pid) = parse_pid_from_instance(&name) {
                            let value = item.FmtValue.Anonymous.doubleValue;
                            *usage_map.entry(pid).or_insert(0.0) += value as f32;
                        }
                    }
                }
            }
        }

        usage_map
    }

    #[cfg(not(windows))]
    pub fn get_usage(&self) -> HashMap<u32, f32> {
        HashMap::new()
    }
}

// Helper to parse PID
fn parse_pid_from_instance(name: &str) -> Option<u32> {
    // Look for "pid_"
    if let Some(idx) = name.find("pid_") {
        let rest = &name[idx + 4..];
        // Find next underscore or end of string
        let end_idx = rest.find('_').unwrap_or(rest.len());
        let pid_str = &rest[..end_idx];
        return pid_str.parse::<u32>().ok();
    }
    None
}

// Public API
pub fn get_gpu_usages() -> HashMap<u32, f32> {
    let mut monitor = GPU_MONITOR.lock().unwrap();
    if monitor.is_none() {
        *monitor = Some(GpuMonitor::new());
    }

    if let Some(mon) = monitor.as_ref() {
        mon.get_usage()
    } else {
        HashMap::new()
    }
}
