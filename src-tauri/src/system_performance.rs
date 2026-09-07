use chrono::Utc;
use serde::Serialize;
use std::sync::Mutex;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuPerformance {
    pub index: u32,
    pub kind: String,
    pub name: String,
    pub usage_percent: Option<f64>,
    pub temperature_celsius: Option<f64>,
    pub dedicated_used_bytes: Option<u64>,
    pub dedicated_total_bytes: Option<u64>,
    pub shared_used_bytes: Option<u64>,
    pub shared_total_bytes: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemPerformanceSnapshot {
    pub available: bool,
    pub captured_at: String,
    pub cpu_usage_percent: Option<f64>,
    pub cpu_frequency_mhz: Option<f64>,
    pub memory_usage_percent: Option<f64>,
    pub memory_used_bytes: Option<u64>,
    pub memory_total_bytes: Option<u64>,
    pub memory_available_bytes: Option<u64>,
    pub disk_usage_percent: Option<f64>,
    pub disk_read_bytes_per_second: Option<f64>,
    pub disk_write_bytes_per_second: Option<f64>,
    pub network_download_bytes_per_second: Option<f64>,
    pub network_upload_bytes_per_second: Option<f64>,
    pub gpus: Vec<GpuPerformance>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryOptimizationResult {
    pub success: bool,
    pub before_bytes: Option<u64>,
    pub after_bytes: Option<u64>,
    pub released_bytes: u64,
    pub message: String,
}

impl SystemPerformanceSnapshot {
    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            available: false,
            captured_at: Utc::now().to_rfc3339(),
            cpu_usage_percent: None,
            cpu_frequency_mhz: None,
            memory_usage_percent: None,
            memory_used_bytes: None,
            memory_total_bytes: None,
            memory_available_bytes: None,
            disk_usage_percent: None,
            disk_read_bytes_per_second: None,
            disk_write_bytes_per_second: None,
            network_download_bytes_per_second: None,
            network_upload_bytes_per_second: None,
            gpus: Vec::new(),
            message: message.into(),
        }
    }
}

pub struct SystemPerformanceState {
    collector: Mutex<Option<PlatformCollector>>,
}

impl Default for SystemPerformanceState {
    fn default() -> Self {
        Self {
            collector: Mutex::new(None),
        }
    }
}

#[tauri::command]
pub fn system_performance_snapshot(
    state: tauri::State<'_, SystemPerformanceState>,
) -> Result<SystemPerformanceSnapshot, String> {
    let mut collector = state
        .collector
        .lock()
        .map_err(|_| "性能采集器状态不可用".to_string())?;
    if collector.is_none() {
        *collector = Some(PlatformCollector::new());
    }
    Ok(collector
        .as_mut()
        .map(PlatformCollector::sample)
        .unwrap_or_else(|| SystemPerformanceSnapshot::unavailable("性能采集器初始化失败")))
}

#[tauri::command]
pub fn optimize_system_memory() -> MemoryOptimizationResult {
    #[cfg(windows)]
    {
        windows_impl::optimize_memory()
    }
    #[cfg(not(windows))]
    {
        MemoryOptimizationResult {
            success: false,
            before_bytes: None,
            after_bytes: None,
            released_bytes: 0,
            message: "当前系统暂不支持安全内存优化".to_string(),
        }
    }
}

#[cfg(not(windows))]
struct PlatformCollector;

#[cfg(not(windows))]
impl PlatformCollector {
    fn new() -> Self {
        Self
    }

    fn sample(&mut self) -> SystemPerformanceSnapshot {
        SystemPerformanceSnapshot::unavailable("当前系统暂不支持实时性能采集")
    }
}

#[cfg(windows)]
mod windows_impl {
    use super::{GpuPerformance, MemoryOptimizationResult, SystemPerformanceSnapshot};
    use chrono::Utc;
    use nvml_wrapper::{enum_wrappers::device::TemperatureSensor, Nvml};
    use std::{
        collections::{BTreeMap, HashMap},
        mem::{size_of, zeroed},
        ptr::{null, null_mut},
        slice,
    };
    use windows_sys::Win32::System::{
        Performance::{
            PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData,
            PdhGetFormattedCounterArrayW, PdhGetFormattedCounterValue, PdhOpenQueryW,
            PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE,
            PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA,
        },
        ProcessStatus::{EmptyWorkingSet, GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
        SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
        Threading::GetCurrentProcess,
    };

    #[derive(Default)]
    struct GpuSample {
        usage_percent: Option<f64>,
        temperature_celsius: Option<f64>,
        dedicated_used_bytes: Option<u64>,
        dedicated_total_bytes: Option<u64>,
        shared_used_bytes: Option<u64>,
        name: Option<String>,
        nvml: bool,
    }

    struct NvmlSample {
        name: String,
        usage_percent: Option<f64>,
        temperature_celsius: Option<f64>,
        dedicated_used_bytes: Option<u64>,
        dedicated_total_bytes: Option<u64>,
    }

    struct PdhCollector {
        query: usize,
        cpu_usage: usize,
        cpu_frequency: usize,
        disk_usage: usize,
        disk_read: usize,
        disk_write: usize,
        network_download: usize,
        network_upload: usize,
        gpu_usage: usize,
        gpu_dedicated: usize,
        gpu_shared: usize,
    }

    impl PdhCollector {
        fn new() -> Option<Self> {
            let mut query: PDH_HQUERY = null_mut();
            if unsafe { PdhOpenQueryW(null(), 0, &mut query) } != 0 {
                return None;
            }
            let cpu_utility =
                add_counter(query, r"\Processor Information(_Total)\% Processor Utility")
                    .or_else(|| add_counter(query, r"\Processor(_Total)\% Processor Time"));
            let result = Self {
                query: query as usize,
                cpu_usage: handle_value(cpu_utility),
                cpu_frequency: handle_value(add_counter(
                    query,
                    r"\Processor Information(_Total)\Processor Frequency",
                )),
                disk_usage: handle_value(add_counter(query, r"\PhysicalDisk(_Total)\% Disk Time")),
                disk_read: handle_value(add_counter(
                    query,
                    r"\PhysicalDisk(_Total)\Disk Read Bytes/sec",
                )),
                disk_write: handle_value(add_counter(
                    query,
                    r"\PhysicalDisk(_Total)\Disk Write Bytes/sec",
                )),
                network_download: handle_value(add_counter(
                    query,
                    r"\Network Interface(*)\Bytes Received/sec",
                )),
                network_upload: handle_value(add_counter(
                    query,
                    r"\Network Interface(*)\Bytes Sent/sec",
                )),
                gpu_usage: handle_value(add_counter(
                    query,
                    r"\GPU Engine(*)\Utilization Percentage",
                )),
                gpu_dedicated: handle_value(add_counter(
                    query,
                    r"\GPU Adapter Memory(*)\Dedicated Usage",
                )),
                gpu_shared: handle_value(add_counter(
                    query,
                    r"\GPU Adapter Memory(*)\Shared Usage",
                )),
            };
            unsafe {
                PdhCollectQueryData(query);
            }
            Some(result)
        }

        fn collect(&self) -> bool {
            unsafe { PdhCollectQueryData(self.query as PDH_HQUERY) == 0 }
        }

        fn value(&self, counter: usize) -> Option<f64> {
            if counter == 0 {
                return None;
            }
            let mut value = PDH_FMT_COUNTERVALUE::default();
            let status = unsafe {
                PdhGetFormattedCounterValue(
                    counter as PDH_HCOUNTER,
                    PDH_FMT_DOUBLE,
                    null_mut(),
                    &mut value,
                )
            };
            if status != 0 || !valid_counter_status(value.CStatus) {
                return None;
            }
            let number = unsafe { value.Anonymous.doubleValue };
            number.is_finite().then_some(number)
        }

        fn array(&self, counter: usize) -> Vec<(String, f64)> {
            if counter == 0 {
                return Vec::new();
            }
            let counter = counter as PDH_HCOUNTER;
            let mut bytes = 0u32;
            let mut count = 0u32;
            let initial = unsafe {
                PdhGetFormattedCounterArrayW(
                    counter,
                    PDH_FMT_DOUBLE,
                    &mut bytes,
                    &mut count,
                    null_mut(),
                )
            };
            if initial != PDH_MORE_DATA || bytes == 0 {
                return Vec::new();
            }
            let word = size_of::<usize>();
            let mut storage = vec![0usize; (bytes as usize).div_ceil(word)];
            let items = storage.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
            let status = unsafe {
                PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut bytes, &mut count, items)
            };
            if status != 0 {
                return Vec::new();
            }
            unsafe { slice::from_raw_parts(items, count as usize) }
                .iter()
                .filter_map(|item| {
                    if !valid_counter_status(item.FmtValue.CStatus) {
                        return None;
                    }
                    let value = unsafe { item.FmtValue.Anonymous.doubleValue };
                    if !value.is_finite() {
                        return None;
                    }
                    Some((unsafe { wide_string(item.szName) }, value))
                })
                .collect()
        }
    }

    impl Drop for PdhCollector {
        fn drop(&mut self) {
            if self.query != 0 {
                unsafe {
                    PdhCloseQuery(self.query as PDH_HQUERY);
                }
            }
        }
    }

    pub(super) struct PlatformCollector {
        pdh: Option<PdhCollector>,
        nvml: Option<Nvml>,
    }

    impl PlatformCollector {
        pub(super) fn new() -> Self {
            Self {
                pdh: PdhCollector::new(),
                nvml: Nvml::init().ok(),
            }
        }

        pub(super) fn sample(&mut self) -> SystemPerformanceSnapshot {
            let memory = memory_status();
            let pdh_ready = self.pdh.as_ref().is_some_and(PdhCollector::collect);
            let cpu_usage = self
                .pdh
                .as_ref()
                .and_then(|pdh| pdh.value(pdh.cpu_usage))
                .map(percent);
            let cpu_frequency = self
                .pdh
                .as_ref()
                .and_then(|pdh| pdh.value(pdh.cpu_frequency))
                .filter(|value| *value >= 0.0);
            let disk_usage = self
                .pdh
                .as_ref()
                .and_then(|pdh| pdh.value(pdh.disk_usage))
                .map(percent);
            let disk_read = self
                .pdh
                .as_ref()
                .and_then(|pdh| pdh.value(pdh.disk_read))
                .map(non_negative);
            let disk_write = self
                .pdh
                .as_ref()
                .and_then(|pdh| pdh.value(pdh.disk_write))
                .map(non_negative);
            let network_download = self
                .pdh
                .as_ref()
                .map(|pdh| sum_values(pdh.array(pdh.network_download)))
                .filter(|_| pdh_ready);
            let network_upload = self
                .pdh
                .as_ref()
                .map(|pdh| sum_values(pdh.array(pdh.network_upload)))
                .filter(|_| pdh_ready);

            let mut gpu_samples = BTreeMap::<String, GpuSample>::new();
            if let Some(pdh) = &self.pdh {
                for (index, usage) in aggregate_gpu_usage(pdh.array(pdh.gpu_usage)) {
                    gpu_samples.entry(index).or_default().usage_percent = Some(percent(usage));
                }
                for (index, bytes) in aggregate_gpu_memory(pdh.array(pdh.gpu_dedicated)) {
                    gpu_samples.entry(index).or_default().dedicated_used_bytes = Some(bytes);
                }
                for (index, bytes) in aggregate_gpu_memory(pdh.array(pdh.gpu_shared)) {
                    gpu_samples.entry(index).or_default().shared_used_bytes = Some(bytes);
                }
            }

            let nvml_samples = self.read_nvml();
            if !nvml_samples.is_empty() {
                let mut candidates = gpu_samples.keys().cloned().collect::<Vec<_>>();
                candidates.sort_by_key(|key| {
                    std::cmp::Reverse(
                        gpu_samples
                            .get(key)
                            .and_then(|item| item.dedicated_used_bytes)
                            .unwrap_or_default(),
                    )
                });
                if candidates.is_empty() {
                    candidates.extend((0..nvml_samples.len()).map(|index| format!("nvml_{index}")));
                }
                for (offset, nvml) in nvml_samples.into_iter().enumerate() {
                    let key = candidates
                        .get(offset)
                        .cloned()
                        .unwrap_or_else(|| format!("nvml_{offset}"));
                    let sample = gpu_samples.entry(key).or_default();
                    sample.name = Some(nvml.name);
                    sample.nvml = true;
                    sample.usage_percent = nvml.usage_percent.or(sample.usage_percent);
                    sample.temperature_celsius = nvml.temperature_celsius;
                    sample.dedicated_used_bytes =
                        nvml.dedicated_used_bytes.or(sample.dedicated_used_bytes);
                    sample.dedicated_total_bytes = nvml.dedicated_total_bytes;
                }
            }

            let gpu_count = gpu_samples.len();
            let shared_total = memory.map(|item| item.0 / 2);
            let mut gpus = gpu_samples
                .into_iter()
                .map(|(_, sample)| {
                    let kind = if sample.nvml {
                        "discrete"
                    } else if gpu_count > 1 {
                        "integrated"
                    } else {
                        "other"
                    };
                    GpuPerformance {
                        index: 0,
                        kind: kind.to_string(),
                        name: sample.name.unwrap_or_else(|| match kind {
                            "integrated" => "集成显卡".to_string(),
                            "discrete" => "独立显卡".to_string(),
                            _ => "图形处理器".to_string(),
                        }),
                        usage_percent: sample.usage_percent,
                        temperature_celsius: sample.temperature_celsius,
                        dedicated_used_bytes: sample.dedicated_used_bytes,
                        dedicated_total_bytes: sample.dedicated_total_bytes,
                        shared_used_bytes: sample.shared_used_bytes,
                        shared_total_bytes: shared_total,
                    }
                })
                .collect::<Vec<_>>();
            gpus.sort_by_key(|gpu| match gpu.kind.as_str() {
                "integrated" => 0,
                "discrete" => 1,
                _ => 2,
            });
            for (index, gpu) in gpus.iter_mut().enumerate() {
                gpu.index = index as u32;
            }

            let (memory_total, memory_available, memory_used, memory_percent) = memory
                .map(|(total, available, load)| {
                    (
                        Some(total),
                        Some(available),
                        Some(total - available),
                        Some(load),
                    )
                })
                .unwrap_or((None, None, None, None));
            let available =
                memory_total.is_some() || cpu_usage.is_some() || pdh_ready || !gpus.is_empty();
            SystemPerformanceSnapshot {
                available,
                captured_at: Utc::now().to_rfc3339(),
                cpu_usage_percent: cpu_usage,
                cpu_frequency_mhz: cpu_frequency,
                memory_usage_percent: memory_percent,
                memory_used_bytes: memory_used,
                memory_total_bytes: memory_total,
                memory_available_bytes: memory_available,
                disk_usage_percent: disk_usage,
                disk_read_bytes_per_second: disk_read,
                disk_write_bytes_per_second: disk_write,
                network_download_bytes_per_second: network_download,
                network_upload_bytes_per_second: network_upload,
                gpus,
                message: if available {
                    "Windows 实时性能数据".to_string()
                } else {
                    "暂未读取到系统性能数据".to_string()
                },
            }
        }

        fn read_nvml(&self) -> Vec<NvmlSample> {
            let Some(nvml) = &self.nvml else {
                return Vec::new();
            };
            let Ok(count) = nvml.device_count() else {
                return Vec::new();
            };
            (0..count)
                .filter_map(|index| {
                    let device = nvml.device_by_index(index).ok()?;
                    let memory = device.memory_info().ok();
                    Some(NvmlSample {
                        name: device
                            .name()
                            .unwrap_or_else(|_| format!("NVIDIA GPU {index}")),
                        usage_percent: device
                            .utilization_rates()
                            .ok()
                            .map(|value| value.gpu as f64),
                        temperature_celsius: device
                            .temperature(TemperatureSensor::Gpu)
                            .ok()
                            .map(|value| value as f64),
                        dedicated_used_bytes: memory.as_ref().map(|value| value.used),
                        dedicated_total_bytes: memory.as_ref().map(|value| value.total),
                    })
                })
                .collect()
        }
    }

    pub(super) fn optimize_memory() -> MemoryOptimizationResult {
        let process = unsafe { GetCurrentProcess() };
        let before = process_working_set(process);
        let success = unsafe { EmptyWorkingSet(process) } != 0;
        let after = process_working_set(process);
        let released = before
            .zip(after)
            .map(|(left, right)| left.saturating_sub(right))
            .unwrap_or_default();
        MemoryOptimizationResult {
            success,
            before_bytes: before,
            after_bytes: after,
            released_bytes: released,
            message: if success {
                "已请求 Windows 回收工作台暂时不用的内存".to_string()
            } else {
                "Windows 暂未执行内存回收，请稍后重试".to_string()
            },
        }
    }

    fn process_working_set(process: windows_sys::Win32::Foundation::HANDLE) -> Option<u64> {
        let mut counters = PROCESS_MEMORY_COUNTERS::default();
        counters.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        (unsafe { GetProcessMemoryInfo(process, &mut counters, counters.cb) } != 0)
            .then_some(counters.WorkingSetSize as u64)
    }

    fn add_counter(query: PDH_HQUERY, path: &str) -> Option<PDH_HCOUNTER> {
        let wide = path.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let mut counter: PDH_HCOUNTER = null_mut();
        (unsafe { PdhAddEnglishCounterW(query, wide.as_ptr(), 0, &mut counter) } == 0)
            .then_some(counter)
    }

    fn handle_value(handle: Option<PDH_HCOUNTER>) -> usize {
        handle.unwrap_or(null_mut()) as usize
    }

    fn valid_counter_status(status: u32) -> bool {
        status == PDH_CSTATUS_VALID_DATA || status == PDH_CSTATUS_NEW_DATA
    }

    unsafe fn wide_string(pointer: *const u16) -> String {
        if pointer.is_null() {
            return String::new();
        }
        let mut length = 0usize;
        while length < 4096 && unsafe { *pointer.add(length) } != 0 {
            length += 1;
        }
        String::from_utf16_lossy(unsafe { slice::from_raw_parts(pointer, length) })
    }

    fn memory_status() -> Option<(u64, u64, f64)> {
        let mut memory: MEMORYSTATUSEX = unsafe { zeroed() };
        memory.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
        if unsafe { GlobalMemoryStatusEx(&mut memory) } == 0 || memory.ullTotalPhys == 0 {
            return None;
        }
        Some((
            memory.ullTotalPhys,
            memory.ullAvailPhys,
            memory.dwMemoryLoad as f64,
        ))
    }

    fn percent(value: f64) -> f64 {
        value.clamp(0.0, 100.0)
    }

    fn non_negative(value: f64) -> f64 {
        value.max(0.0)
    }

    fn sum_values(items: Vec<(String, f64)>) -> f64 {
        items
            .into_iter()
            .map(|(_, value)| non_negative(value))
            .sum()
    }

    fn parsed_index(name: &str, marker: &str) -> Option<u32> {
        let start = name.find(marker)? + marker.len();
        let digits = name[start..]
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .collect::<String>();
        (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
    }

    fn gpu_adapter_key(name: &str) -> Option<String> {
        let start = name.find("luid_")?;
        let physical = name[start..].find("_phys_")? + start;
        let digits = name[physical + 6..]
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .count();
        (digits > 0).then(|| name[start..physical + 6 + digits].to_ascii_lowercase())
    }

    fn aggregate_gpu_usage(items: Vec<(String, f64)>) -> BTreeMap<String, f64> {
        let mut engines = HashMap::<(String, u32), f64>::new();
        for (name, value) in items {
            let Some(adapter) = gpu_adapter_key(&name) else {
                continue;
            };
            let engine = parsed_index(&name, "eng_").unwrap_or(0);
            *engines.entry((adapter, engine)).or_default() += non_negative(value);
        }
        let mut adapters = BTreeMap::<String, f64>::new();
        for ((adapter, _), value) in engines {
            let current = adapters.entry(adapter).or_default();
            *current = current.max(value);
        }
        adapters
    }

    fn aggregate_gpu_memory(items: Vec<(String, f64)>) -> BTreeMap<String, u64> {
        let mut adapters = BTreeMap::<String, u64>::new();
        for (name, value) in items {
            let Some(adapter) = gpu_adapter_key(&name) else {
                continue;
            };
            let bytes = non_negative(value).round() as u64;
            let current = adapters.entry(adapter).or_default();
            *current = (*current).max(bytes);
        }
        adapters
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn gpu_engines_are_summed_per_engine_then_take_busiest_engine() {
            let values = aggregate_gpu_usage(vec![
                ("pid_1_luid_0x0_phys_0_eng_0_engtype_3D".into(), 12.0),
                ("pid_2_luid_0x0_phys_0_eng_0_engtype_3D".into(), 8.0),
                ("pid_1_luid_0x0_phys_0_eng_1_engtype_Copy".into(), 4.0),
                ("pid_1_luid_0x1_phys_0_eng_0_engtype_3D".into(), 31.0),
            ]);
            assert_eq!(values.get("luid_0x0_phys_0"), Some(&20.0));
            assert_eq!(values.get("luid_0x1_phys_0"), Some(&31.0));
        }

        #[test]
        fn gpu_memory_is_grouped_by_physical_adapter() {
            let values = aggregate_gpu_memory(vec![
                ("luid_0x0_phys_0".into(), 1024.0),
                ("luid_0x1_phys_1".into(), 4096.0),
            ]);
            assert_eq!(values.get("luid_0x0_phys_0"), Some(&1024));
            assert_eq!(values.get("luid_0x1_phys_1"), Some(&4096));
        }

        #[test]
        fn live_windows_sample_has_memory_and_bounded_percentages() {
            let mut collector = PlatformCollector::new();
            std::thread::sleep(std::time::Duration::from_millis(250));
            let snapshot = collector.sample();
            assert!(snapshot.memory_total_bytes.unwrap_or_default() > 0);
            if let Some(cpu) = snapshot.cpu_usage_percent {
                assert!((0.0..=100.0).contains(&cpu));
            }
            if let Some(disk) = snapshot.disk_usage_percent {
                assert!((0.0..=100.0).contains(&disk));
            }
        }
    }
}

#[cfg(windows)]
use windows_impl::PlatformCollector;
