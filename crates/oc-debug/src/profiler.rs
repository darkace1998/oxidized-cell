//! Performance profiler for CPU/GPU analysis

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Profile category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProfileCategory {
    /// PPU execution
    PpuExecution,
    /// SPU execution
    SpuExecution,
    /// RSX/GPU execution
    RsxExecution,
    /// Memory access
    Memory,
    /// Syscall handling
    Syscall,
    /// Frame presentation
    FramePresent,
    /// JIT compilation
    JitCompilation,
    /// Other/misc
    Other,
}

impl ProfileCategory {
    /// Get category name
    pub fn name(&self) -> &'static str {
        match self {
            Self::PpuExecution => "PPU Execution",
            Self::SpuExecution => "SPU Execution",
            Self::RsxExecution => "RSX Execution",
            Self::Memory => "Memory Access",
            Self::Syscall => "Syscall Handling",
            Self::FramePresent => "Frame Present",
            Self::JitCompilation => "JIT Compilation",
            Self::Other => "Other",
        }
    }
}

/// Profile entry for a specific section
#[derive(Debug, Clone)]
pub struct ProfileEntry {
    /// Section name
    pub name: String,
    /// Category
    pub category: ProfileCategory,
    /// Total time spent
    pub total_time: Duration,
    /// Number of invocations
    pub call_count: u64,
    /// Minimum time
    pub min_time: Duration,
    /// Maximum time
    pub max_time: Duration,
}

impl ProfileEntry {
    /// Create a new profile entry
    pub fn new(name: &str, category: ProfileCategory) -> Self {
        Self {
            name: name.to_string(),
            category,
            total_time: Duration::ZERO,
            call_count: 0,
            min_time: Duration::MAX,
            max_time: Duration::ZERO,
        }
    }

    /// Record a sample
    pub fn record(&mut self, duration: Duration) {
        self.total_time += duration;
        self.call_count += 1;
        self.min_time = self.min_time.min(duration);
        self.max_time = self.max_time.max(duration);
    }

    /// Get average time
    pub fn average(&self) -> Duration {
        if self.call_count == 0 {
            Duration::ZERO
        } else {
            self.total_time / self.call_count as u32
        }
    }

    /// Get calls per second
    pub fn calls_per_second(&self, elapsed: Duration) -> f64 {
        if elapsed.is_zero() {
            0.0
        } else {
            self.call_count as f64 / elapsed.as_secs_f64()
        }
    }

    /// Reset statistics
    pub fn reset(&mut self) {
        self.total_time = Duration::ZERO;
        self.call_count = 0;
        self.min_time = Duration::MAX;
        self.max_time = Duration::ZERO;
    }
}

/// Hotspot entry
#[derive(Debug, Clone)]
pub struct Hotspot {
    /// Address
    pub address: u64,
    /// Hit count
    pub hit_count: u64,
    /// Time spent
    pub time_spent: Duration,
    /// Percentage of total time
    pub percentage: f64,
}

/// Frame timing info
#[derive(Debug, Clone)]
pub struct FrameTiming {
    /// Frame number
    pub frame: u64,
    /// Total frame time
    pub total_time: Duration,
    /// PPU time
    pub ppu_time: Duration,
    /// SPU time
    pub spu_time: Duration,
    /// RSX time
    pub rsx_time: Duration,
    /// Other time
    pub other_time: Duration,
}

/// Memory access type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryAccessType {
    /// Read access
    Read,
    /// Write access
    Write,
}

/// Memory access pattern entry
#[derive(Debug, Clone)]
pub struct MemoryAccessPattern {
    /// Memory region start
    pub region_start: u64,
    /// Memory region end
    pub region_end: u64,
    /// Read count
    pub read_count: u64,
    /// Write count
    pub write_count: u64,
    /// Bytes read
    pub bytes_read: u64,
    /// Bytes written
    pub bytes_written: u64,
}

/// Memory region hotspot
#[derive(Debug, Clone)]
pub struct MemoryRegionHotspot {
    /// Region base address (4KB page aligned)
    pub page_address: u64,
    /// Total accesses
    pub total_accesses: u64,
    /// Read accesses
    pub read_accesses: u64,
    /// Write accesses
    pub write_accesses: u64,
    /// Access percentage of total memory accesses
    pub percentage: f64,
}

/// Performance profiler
pub struct Profiler {
    /// Is profiling enabled
    pub enabled: bool,
    /// Profile entries by name
    entries: HashMap<String, ProfileEntry>,
    /// Start time of profiling session
    session_start: Instant,
    /// Frame timings
    frame_timings: Vec<FrameTiming>,
    /// Maximum frame history
    max_frame_history: usize,
    /// Current frame number
    current_frame: u64,
    /// Current frame start time
    frame_start: Option<Instant>,
    /// Current frame category times
    frame_category_times: HashMap<ProfileCategory, Duration>,
    /// Address hotspots for PPU
    ppu_hotspots: HashMap<u64, u64>,
    /// Address hotspots for SPU
    spu_hotspots: HashMap<u64, u64>,
    /// Total instructions executed (for percentage calculation)
    total_instructions: u64,
    /// Memory access patterns (by 4KB page)
    memory_access_patterns: HashMap<u64, MemoryAccessPattern>,
    /// Total memory reads
    total_memory_reads: u64,
    /// Total memory writes
    total_memory_writes: u64,
    /// Memory access tracking enabled (can be expensive)
    memory_tracking_enabled: bool,
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Profiler {
    /// Create a new profiler
    pub fn new() -> Self {
        Self {
            enabled: false,
            entries: HashMap::new(),
            session_start: Instant::now(),
            frame_timings: Vec::new(),
            max_frame_history: 300, // 5 seconds at 60fps
            current_frame: 0,
            frame_start: None,
            frame_category_times: HashMap::new(),
            ppu_hotspots: HashMap::new(),
            spu_hotspots: HashMap::new(),
            total_instructions: 0,
            memory_access_patterns: HashMap::new(),
            total_memory_reads: 0,
            total_memory_writes: 0,
            memory_tracking_enabled: false,
        }
    }

    /// Enable profiling
    pub fn enable(&mut self) {
        self.enabled = true;
        self.session_start = Instant::now();
        tracing::info!("Profiler enabled");
    }

    /// Disable profiling
    pub fn disable(&mut self) {
        self.enabled = false;
        tracing::info!("Profiler disabled");
    }

    /// Start a profiled section
    pub fn start_section(&self, _name: &str) -> Option<ProfileScope> {
        if !self.enabled {
            return None;
        }
        Some(ProfileScope {
            start: Instant::now(),
        })
    }

    /// End a profiled section
    pub fn end_section(&mut self, name: &str, category: ProfileCategory, scope: ProfileScope) {
        if !self.enabled {
            return;
        }
        
        let duration = scope.start.elapsed();
        
        let entry = self.entries.entry(name.to_string())
            .or_insert_with(|| ProfileEntry::new(name, category));
        entry.record(duration);
        
        // Also record to current frame category
        *self.frame_category_times.entry(category).or_insert(Duration::ZERO) += duration;
    }

    /// Record instruction execution at address (for hotspot analysis)
    pub fn record_ppu_instruction(&mut self, address: u64) {
        if !self.enabled {
            return;
        }
        *self.ppu_hotspots.entry(address).or_insert(0) += 1;
        self.total_instructions += 1;
    }

    /// Record SPU instruction execution
    pub fn record_spu_instruction(&mut self, spu_id: u32, address: u32) {
        if !self.enabled {
            return;
        }
        // Encode SPU ID in upper bits
        let key = ((spu_id as u64) << 32) | (address as u64);
        *self.spu_hotspots.entry(key).or_insert(0) += 1;
        self.total_instructions += 1;
    }

    /// Start frame timing
    pub fn start_frame(&mut self) {
        if !self.enabled {
            return;
        }
        self.frame_start = Some(Instant::now());
        self.frame_category_times.clear();
    }

    /// End frame timing
    pub fn end_frame(&mut self) {
        if !self.enabled {
            return;
        }
        
        let total_time = self.frame_start.map(|s| s.elapsed()).unwrap_or_default();
        
        let timing = FrameTiming {
            frame: self.current_frame,
            total_time,
            ppu_time: *self.frame_category_times.get(&ProfileCategory::PpuExecution).unwrap_or(&Duration::ZERO),
            spu_time: *self.frame_category_times.get(&ProfileCategory::SpuExecution).unwrap_or(&Duration::ZERO),
            rsx_time: *self.frame_category_times.get(&ProfileCategory::RsxExecution).unwrap_or(&Duration::ZERO),
            other_time: *self.frame_category_times.get(&ProfileCategory::Other).unwrap_or(&Duration::ZERO),
        };
        
        self.frame_timings.push(timing);
        
        // Limit history
        if self.frame_timings.len() > self.max_frame_history {
            self.frame_timings.remove(0);
        }
        
        self.current_frame += 1;
        self.frame_start = None;
    }

    /// Get all profile entries
    pub fn get_entries(&self) -> Vec<&ProfileEntry> {
        let mut entries: Vec<_> = self.entries.values().collect();
        entries.sort_by(|a, b| b.total_time.cmp(&a.total_time));
        entries
    }

    /// Get entries by category
    pub fn get_entries_by_category(&self, category: ProfileCategory) -> Vec<&ProfileEntry> {
        self.entries.values()
            .filter(|e| e.category == category)
            .collect()
    }

    /// Get frame timings
    pub fn get_frame_timings(&self, count: usize) -> &[FrameTiming] {
        let start = self.frame_timings.len().saturating_sub(count);
        &self.frame_timings[start..]
    }

    /// Get average FPS
    pub fn get_average_fps(&self) -> f64 {
        let timings = self.get_frame_timings(60);
        if timings.is_empty() {
            return 0.0;
        }
        
        let total: Duration = timings.iter().map(|t| t.total_time).sum();
        if total.is_zero() {
            return 0.0;
        }
        
        timings.len() as f64 / total.as_secs_f64()
    }

    /// Get average frame time in milliseconds
    pub fn get_average_frame_time_ms(&self) -> f64 {
        let timings = self.get_frame_timings(60);
        if timings.is_empty() {
            return 0.0;
        }
        
        let total: Duration = timings.iter().map(|t| t.total_time).sum();
        (total.as_secs_f64() * 1000.0) / timings.len() as f64
    }

    /// Get PPU hotspots (top N by hit count)
    pub fn get_ppu_hotspots(&self, count: usize) -> Vec<Hotspot> {
        let mut hotspots: Vec<_> = self.ppu_hotspots.iter()
            .map(|(&addr, &hits)| Hotspot {
                address: addr,
                hit_count: hits,
                time_spent: Duration::ZERO, // We don't track per-instruction time
                percentage: if self.total_instructions > 0 {
                    (hits as f64 / self.total_instructions as f64) * 100.0
                } else {
                    0.0
                },
            })
            .collect();
        
        hotspots.sort_by(|a, b| b.hit_count.cmp(&a.hit_count));
        hotspots.truncate(count);
        hotspots
    }

    /// Get SPU hotspots (top N by hit count)
    pub fn get_spu_hotspots(&self, count: usize) -> Vec<Hotspot> {
        let mut hotspots: Vec<_> = self.spu_hotspots.iter()
            .map(|(&key, &hits)| Hotspot {
                address: key,
                hit_count: hits,
                time_spent: Duration::ZERO,
                percentage: if self.total_instructions > 0 {
                    (hits as f64 / self.total_instructions as f64) * 100.0
                } else {
                    0.0
                },
            })
            .collect();
        
        hotspots.sort_by(|a, b| b.hit_count.cmp(&a.hit_count));
        hotspots.truncate(count);
        hotspots
    }

    /// Get session duration
    pub fn session_duration(&self) -> Duration {
        self.session_start.elapsed()
    }

    /// Reset all profiling data
    pub fn reset(&mut self) {
        self.entries.clear();
        self.frame_timings.clear();
        self.ppu_hotspots.clear();
        self.spu_hotspots.clear();
        self.total_instructions = 0;
        self.current_frame = 0;
        self.memory_access_patterns.clear();
        self.total_memory_reads = 0;
        self.total_memory_writes = 0;
        self.session_start = Instant::now();
        tracing::info!("Profiler reset");
    }

    // === Memory Access Pattern Tracking ===

    /// Enable memory access tracking (can be expensive)
    pub fn enable_memory_tracking(&mut self) {
        self.memory_tracking_enabled = true;
        tracing::info!("Memory access tracking enabled");
    }

    /// Disable memory access tracking
    pub fn disable_memory_tracking(&mut self) {
        self.memory_tracking_enabled = false;
        tracing::info!("Memory access tracking disabled");
    }

    /// Record a memory access
    pub fn record_memory_access(&mut self, address: u64, size: u32, access_type: MemoryAccessType) {
        if !self.enabled || !self.memory_tracking_enabled {
            return;
        }
        
        // Track by 4KB page
        let page_address = address & !0xFFF;
        
        let pattern = self.memory_access_patterns.entry(page_address).or_insert_with(|| {
            MemoryAccessPattern {
                region_start: page_address,
                region_end: page_address + 0x1000,
                read_count: 0,
                write_count: 0,
                bytes_read: 0,
                bytes_written: 0,
            }
        });
        
        match access_type {
            MemoryAccessType::Read => {
                pattern.read_count += 1;
                pattern.bytes_read += size as u64;
                self.total_memory_reads += 1;
            }
            MemoryAccessType::Write => {
                pattern.write_count += 1;
                pattern.bytes_written += size as u64;
                self.total_memory_writes += 1;
            }
        }
    }

    /// Get memory hotspots (top N pages by access count)
    pub fn get_memory_hotspots(&self, count: usize) -> Vec<MemoryRegionHotspot> {
        let total_accesses = self.total_memory_reads + self.total_memory_writes;
        
        let mut hotspots: Vec<_> = self.memory_access_patterns.iter()
            .map(|(&page, pattern)| {
                let accesses = pattern.read_count + pattern.write_count;
                MemoryRegionHotspot {
                    page_address: page,
                    total_accesses: accesses,
                    read_accesses: pattern.read_count,
                    write_accesses: pattern.write_count,
                    percentage: if total_accesses > 0 {
                        (accesses as f64 / total_accesses as f64) * 100.0
                    } else {
                        0.0
                    },
                }
            })
            .collect();
        
        hotspots.sort_by(|a, b| b.total_accesses.cmp(&a.total_accesses));
        hotspots.truncate(count);
        hotspots
    }

    /// Get memory access statistics for a specific region
    pub fn get_region_stats(&self, start: u64, end: u64) -> MemoryAccessPattern {
        let mut aggregate = MemoryAccessPattern {
            region_start: start,
            region_end: end,
            read_count: 0,
            write_count: 0,
            bytes_read: 0,
            bytes_written: 0,
        };
        
        // Round to page boundaries
        let start_page = start & !0xFFF;
        let end_page = (end + 0xFFF) & !0xFFF;
        
        for (&page, pattern) in &self.memory_access_patterns {
            if page >= start_page && page < end_page {
                aggregate.read_count += pattern.read_count;
                aggregate.write_count += pattern.write_count;
                aggregate.bytes_read += pattern.bytes_read;
                aggregate.bytes_written += pattern.bytes_written;
            }
        }
        
        aggregate
    }

    /// Get total memory access counts
    pub fn get_memory_access_totals(&self) -> (u64, u64) {
        (self.total_memory_reads, self.total_memory_writes)
    }

    /// Check if memory tracking is enabled
    pub fn is_memory_tracking_enabled(&self) -> bool {
        self.memory_tracking_enabled
    }

    /// Generate profiling report as text
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        
        report.push_str("=== Performance Profile Report ===\n\n");
        
        report.push_str(&format!("Session duration: {:.2}s\n", self.session_duration().as_secs_f64()));
        report.push_str(&format!("Total frames: {}\n", self.current_frame));
        report.push_str(&format!("Average FPS: {:.1}\n", self.get_average_fps()));
        report.push_str(&format!("Average frame time: {:.2}ms\n\n", self.get_average_frame_time_ms()));
        
        report.push_str("--- Top Sections by Time ---\n");
        for entry in self.get_entries().iter().take(10) {
            report.push_str(&format!(
                "{}: {:.2}ms total, {} calls, {:.3}ms avg\n",
                entry.name,
                entry.total_time.as_secs_f64() * 1000.0,
                entry.call_count,
                entry.average().as_secs_f64() * 1000.0
            ));
        }
        
        report.push_str("\n--- PPU Hotspots ---\n");
        for hotspot in self.get_ppu_hotspots(10) {
            report.push_str(&format!(
                "0x{:016X}: {} hits ({:.2}%)\n",
                hotspot.address,
                hotspot.hit_count,
                hotspot.percentage
            ));
        }
        
        report.push_str("\n--- SPU Hotspots ---\n");
        for hotspot in self.get_spu_hotspots(10) {
            let spu_id = (hotspot.address >> 32) as u32;
            let addr = hotspot.address as u32;
            report.push_str(&format!(
                "SPU{} 0x{:08X}: {} hits ({:.2}%)\n",
                spu_id,
                addr,
                hotspot.hit_count,
                hotspot.percentage
            ));
        }
        
        // Add memory access patterns if tracking was enabled or if we have collected data
        // Note: We show data even if tracking is now disabled, since reports should show
        // all collected data. Use reset() to clear stale data if needed.
        if self.memory_tracking_enabled || self.total_memory_reads > 0 || self.total_memory_writes > 0 {
            report.push_str("\n--- Memory Access Patterns ---\n");
            if !self.memory_tracking_enabled && (self.total_memory_reads > 0 || self.total_memory_writes > 0) {
                report.push_str("(Note: memory tracking is currently disabled, showing previously collected data)\n");
            }
            report.push_str(&format!(
                "Total reads: {}, Total writes: {}\n",
                self.total_memory_reads,
                self.total_memory_writes
            ));
            
            report.push_str("\nTop Memory Hotspots (by page):\n");
            for hotspot in self.get_memory_hotspots(10) {
                report.push_str(&format!(
                    "0x{:016X}: {} accesses (R:{} W:{}) ({:.2}%)\n",
                    hotspot.page_address,
                    hotspot.total_accesses,
                    hotspot.read_accesses,
                    hotspot.write_accesses,
                    hotspot.percentage
                ));
            }
        }
        
        report
    }
}

/// Profile scope for RAII timing
pub struct ProfileScope {
    start: Instant,
}

impl ProfileScope {
    /// Get elapsed time
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

/// Macro for scoped profiling
#[macro_export]
macro_rules! profile_scope {
    ($profiler:expr, $name:expr, $category:expr, $code:block) => {{
        let scope = $profiler.start_section($name);
        let result = $code;
        if let Some(s) = scope {
            $profiler.end_section($name, $category, s);
        }
        result
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profiler_creation() {
        let profiler = Profiler::new();
        assert!(!profiler.enabled);
    }

    #[test]
    fn test_profiler_enable_disable() {
        let mut profiler = Profiler::new();
        
        profiler.enable();
        assert!(profiler.enabled);
        
        profiler.disable();
        assert!(!profiler.enabled);
    }

    #[test]
    fn test_profiler_section() {
        let mut profiler = Profiler::new();
        profiler.enable();
        
        if let Some(scope) = profiler.start_section("test") {
            std::thread::sleep(std::time::Duration::from_millis(1));
            profiler.end_section("test", ProfileCategory::Other, scope);
        }
        
        let entries = profiler.get_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "test");
        assert_eq!(entries[0].call_count, 1);
    }

    #[test]
    fn test_frame_timing() {
        let mut profiler = Profiler::new();
        profiler.enable();
        
        profiler.start_frame();
        std::thread::sleep(std::time::Duration::from_millis(1));
        profiler.end_frame();
        
        let timings = profiler.get_frame_timings(10);
        assert_eq!(timings.len(), 1);
        assert!(timings[0].total_time.as_micros() > 0);
    }

    #[test]
    fn test_hotspots() {
        let mut profiler = Profiler::new();
        profiler.enable();
        
        profiler.record_ppu_instruction(0x10000);
        profiler.record_ppu_instruction(0x10000);
        profiler.record_ppu_instruction(0x10004);
        
        let hotspots = profiler.get_ppu_hotspots(10);
        assert_eq!(hotspots.len(), 2);
        assert_eq!(hotspots[0].address, 0x10000);
        assert_eq!(hotspots[0].hit_count, 2);
    }

    #[test]
    fn test_profile_entry_average() {
        let mut entry = ProfileEntry::new("test", ProfileCategory::Other);
        entry.record(Duration::from_millis(10));
        entry.record(Duration::from_millis(20));
        entry.record(Duration::from_millis(30));
        
        assert_eq!(entry.call_count, 3);
        assert_eq!(entry.average(), Duration::from_millis(20));
        assert_eq!(entry.min_time, Duration::from_millis(10));
        assert_eq!(entry.max_time, Duration::from_millis(30));
    }
}
