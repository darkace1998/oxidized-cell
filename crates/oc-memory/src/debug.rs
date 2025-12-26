//! Memory debugging features for oxidized-cell
//!
//! This module provides debugging and profiling features including:
//! - Memory watchpoints
//! - Self-modifying code detection
//! - Cache simulation
//! - Memory access profiling

use std::collections::{HashMap, HashSet};
use parking_lot::RwLock;
use oc_core::error::AccessKind;

/// Watchpoint type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchpointType {
    /// Break on read
    Read,
    /// Break on write
    Write,
    /// Break on read or write
    ReadWrite,
    /// Break on execute
    Execute,
}

/// Watchpoint information
#[derive(Debug, Clone)]
pub struct Watchpoint {
    /// Address to watch
    pub addr: u32,
    /// Size of the watched region
    pub size: u32,
    /// Type of access to watch for
    pub wp_type: WatchpointType,
    /// Whether the watchpoint is enabled
    pub enabled: bool,
    /// Number of times the watchpoint has been hit
    pub hit_count: u64,
    /// Optional condition (value comparison)
    pub condition: Option<WatchpointCondition>,
}

/// Watchpoint condition for conditional breaks
#[derive(Debug, Clone)]
pub enum WatchpointCondition {
    /// Break when value equals
    Equals(u64),
    /// Break when value changes
    Changed,
    /// Break when value equals old + delta
    Delta(i64),
}

/// Memory watchpoint manager
pub struct WatchpointManager {
    /// Active watchpoints
    watchpoints: RwLock<HashMap<u32, Watchpoint>>,
    /// Quick lookup for addresses with watchpoints
    watched_pages: RwLock<HashSet<u32>>,
}

impl WatchpointManager {
    /// Create a new watchpoint manager
    pub fn new() -> Self {
        Self {
            watchpoints: RwLock::new(HashMap::new()),
            watched_pages: RwLock::new(HashSet::new()),
        }
    }

    /// Add a watchpoint
    pub fn add(&self, addr: u32, size: u32, wp_type: WatchpointType) {
        let wp = Watchpoint {
            addr,
            size,
            wp_type,
            enabled: true,
            hit_count: 0,
            condition: None,
        };

        self.watchpoints.write().insert(addr, wp);

        // Mark affected pages
        let page_size = 0x1000u32;
        let start_page = addr / page_size;
        let end_page = (addr + size.saturating_sub(1)) / page_size;
        
        let mut watched = self.watched_pages.write();
        for page in start_page..=end_page {
            watched.insert(page);
        }
    }

    /// Add a conditional watchpoint
    pub fn add_conditional(&self, addr: u32, size: u32, wp_type: WatchpointType, condition: WatchpointCondition) {
        let wp = Watchpoint {
            addr,
            size,
            wp_type,
            enabled: true,
            hit_count: 0,
            condition: Some(condition),
        };

        self.watchpoints.write().insert(addr, wp);

        // Mark affected pages
        let page_size = 0x1000u32;
        let start_page = addr / page_size;
        let end_page = (addr + size.saturating_sub(1)) / page_size;
        
        let mut watched = self.watched_pages.write();
        for page in start_page..=end_page {
            watched.insert(page);
        }
    }

    /// Remove a watchpoint
    pub fn remove(&self, addr: u32) {
        let wp = self.watchpoints.write().remove(&addr);
        
        if let Some(wp) = wp {
            // Recalculate watched pages
            let page_size = 0x1000u32;
            let start_page = wp.addr / page_size;
            let end_page = (wp.addr + wp.size.saturating_sub(1)) / page_size;

            // Check which pages are still covered by other watchpoints
            let watchpoints = self.watchpoints.read();
            let mut watched = self.watched_pages.write();
            
            for page in start_page..=end_page {
                // Only remove if no other watchpoints cover this page
                let still_covered = watchpoints.values().any(|w| {
                    let wp_start = w.addr / page_size;
                    let wp_end = (w.addr + w.size.saturating_sub(1)) / page_size;
                    page >= wp_start && page <= wp_end
                });
                if !still_covered {
                    watched.remove(&page);
                }
            }
        }
    }

    /// Enable a watchpoint
    pub fn enable(&self, addr: u32) {
        if let Some(wp) = self.watchpoints.write().get_mut(&addr) {
            wp.enabled = true;
        }
    }

    /// Disable a watchpoint
    pub fn disable(&self, addr: u32) {
        if let Some(wp) = self.watchpoints.write().get_mut(&addr) {
            wp.enabled = false;
        }
    }

    /// Clear all watchpoints
    pub fn clear(&self) {
        self.watchpoints.write().clear();
        self.watched_pages.write().clear();
    }

    /// Check if an address is being watched (fast path)
    #[inline]
    pub fn is_watched_page(&self, addr: u32) -> bool {
        let page = addr / 0x1000;
        self.watched_pages.read().contains(&page)
    }

    /// Check if an access should trigger a watchpoint
    pub fn check_access(&self, addr: u32, size: u32, kind: AccessKind) -> Option<u32> {
        // Fast path: check if page is watched
        if !self.is_watched_page(addr) {
            return None;
        }

        let watchpoints = self.watchpoints.read();
        for (wp_addr, wp) in watchpoints.iter() {
            if !wp.enabled {
                continue;
            }

            // Check if access overlaps with watchpoint
            let access_end = addr.saturating_add(size.saturating_sub(1));
            let wp_end = wp.addr.saturating_add(wp.size.saturating_sub(1));

            if addr <= wp_end && access_end >= wp.addr {
                // Check access type
                let matches = match wp.wp_type {
                    WatchpointType::Read => kind == AccessKind::Read,
                    WatchpointType::Write => kind == AccessKind::Write,
                    WatchpointType::ReadWrite => kind == AccessKind::Read || kind == AccessKind::Write,
                    WatchpointType::Execute => kind == AccessKind::Execute,
                };

                if matches {
                    return Some(*wp_addr);
                }
            }
        }

        None
    }

    /// Increment hit count for a watchpoint
    pub fn record_hit(&self, addr: u32) {
        if let Some(wp) = self.watchpoints.write().get_mut(&addr) {
            wp.hit_count += 1;
        }
    }

    /// Get all watchpoints
    pub fn get_all(&self) -> Vec<Watchpoint> {
        self.watchpoints.read().values().cloned().collect()
    }

    /// Get watchpoint count
    pub fn count(&self) -> usize {
        self.watchpoints.read().len()
    }
}

impl Default for WatchpointManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Self-modifying code detector
pub struct SmcDetector {
    /// Set of pages that have been executed
    executed_pages: RwLock<HashSet<u32>>,
    /// Enabled flag
    enabled: bool,
}

impl SmcDetector {
    /// Create a new SMC detector
    pub fn new(enabled: bool) -> Self {
        Self {
            executed_pages: RwLock::new(HashSet::new()),
            enabled,
        }
    }

    /// Mark a page as executed
    #[inline]
    pub fn mark_executed(&self, addr: u32) {
        if !self.enabled {
            return;
        }
        let page = addr / 0x1000;
        self.executed_pages.write().insert(page);
    }

    /// Check if a write is to an executed page (potential SMC)
    #[inline]
    pub fn check_write(&self, addr: u32) -> Option<u32> {
        if !self.enabled {
            return None;
        }
        let page = addr / 0x1000;
        if self.executed_pages.read().contains(&page) {
            Some(addr)
        } else {
            None
        }
    }

    /// Invalidate a page (e.g., after icbi instruction)
    pub fn invalidate(&self, addr: u32) {
        let page = addr / 0x1000;
        self.executed_pages.write().remove(&page);
    }

    /// Clear all tracked pages
    pub fn clear(&self) {
        self.executed_pages.write().clear();
    }

    /// Get count of executed pages
    pub fn executed_page_count(&self) -> usize {
        self.executed_pages.read().len()
    }

    /// Enable or disable the detector
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.clear();
        }
    }

    /// Check if detector is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for SmcDetector {
    fn default() -> Self {
        Self::new(false)
    }
}

/// Cache line state for simulation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CacheLineState {
    #[default]
    Invalid,
    Shared,
    Exclusive,
    Modified,
}

/// Cache simulation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CacheMode {
    #[default]
    Disabled,
    /// L1 data cache simulation
    L1Data,
    /// L1 instruction cache simulation
    L1Instruction,
    /// L2 cache simulation
    L2,
    /// Full cache hierarchy simulation
    Full,
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Number of evictions
    pub evictions: u64,
    /// Number of writebacks
    pub writebacks: u64,
}

impl CacheStats {
    /// Calculate hit rate
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            1.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    /// Reset statistics
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Simple cache simulator (direct-mapped for simplicity)
pub struct CacheSimulator {
    /// Cache mode
    mode: CacheMode,
    /// Cache line size in bytes (typically 128 for Cell)
    line_size: u32,
    /// Number of cache lines
    num_lines: u32,
    /// Cache tags (line -> tag)
    tags: Vec<Option<u32>>,
    /// Cache line states
    states: Vec<CacheLineState>,
    /// Statistics
    stats: CacheStats,
}

impl CacheSimulator {
    /// Create a new cache simulator
    pub fn new(mode: CacheMode) -> Self {
        let (line_size, num_lines) = match mode {
            CacheMode::Disabled => (128, 0),
            CacheMode::L1Data => (128, 256),      // 32KB
            CacheMode::L1Instruction => (128, 256), // 32KB
            CacheMode::L2 => (128, 4096),          // 512KB
            CacheMode::Full => (128, 4096),        // Simplified
        };

        let tags = vec![None; num_lines as usize];
        let states = vec![CacheLineState::Invalid; num_lines as usize];

        Self {
            mode,
            line_size,
            num_lines,
            tags,
            states,
            stats: CacheStats::default(),
        }
    }

    /// Check if cache is enabled
    pub fn is_enabled(&self) -> bool {
        self.mode != CacheMode::Disabled && self.num_lines > 0
    }

    /// Access cache (returns true if hit, false if miss)
    pub fn access(&mut self, addr: u32, write: bool) -> bool {
        if !self.is_enabled() {
            return true; // Disabled = always hit
        }

        let tag = addr / self.line_size;
        let index = (tag % self.num_lines) as usize;
        let expected_tag = tag;

        let hit = self.tags[index] == Some(expected_tag);

        if hit {
            self.stats.hits += 1;
            if write {
                self.states[index] = CacheLineState::Modified;
            }
        } else {
            self.stats.misses += 1;

            // Evict old line if present
            if self.tags[index].is_some() {
                self.stats.evictions += 1;
                if self.states[index] == CacheLineState::Modified {
                    self.stats.writebacks += 1;
                }
            }

            // Install new line
            self.tags[index] = Some(expected_tag);
            self.states[index] = if write {
                CacheLineState::Modified
            } else {
                CacheLineState::Exclusive
            };
        }

        hit
    }

    /// Invalidate a cache line
    pub fn invalidate(&mut self, addr: u32) {
        if !self.is_enabled() {
            return;
        }

        let tag = addr / self.line_size;
        let index = (tag % self.num_lines) as usize;

        if self.tags[index] == Some(tag) {
            if self.states[index] == CacheLineState::Modified {
                self.stats.writebacks += 1;
            }
            self.tags[index] = None;
            self.states[index] = CacheLineState::Invalid;
        }
    }

    /// Flush the entire cache
    pub fn flush(&mut self) {
        for i in 0..self.tags.len() {
            if self.states[i] == CacheLineState::Modified {
                self.stats.writebacks += 1;
            }
            self.tags[i] = None;
            self.states[i] = CacheLineState::Invalid;
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats.reset();
    }

    /// Get cache mode
    pub fn mode(&self) -> CacheMode {
        self.mode
    }
}

impl Default for CacheSimulator {
    fn default() -> Self {
        Self::new(CacheMode::Disabled)
    }
}

/// Memory access profiler
pub struct MemoryProfiler {
    /// Enabled flag
    enabled: bool,
    /// Read counts per page
    read_counts: RwLock<HashMap<u32, u64>>,
    /// Write counts per page
    write_counts: RwLock<HashMap<u32, u64>>,
    /// Execute counts per page
    execute_counts: RwLock<HashMap<u32, u64>>,
    /// Total read bytes
    total_reads: std::sync::atomic::AtomicU64,
    /// Total write bytes
    total_writes: std::sync::atomic::AtomicU64,
}

impl MemoryProfiler {
    /// Create a new memory profiler
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            read_counts: RwLock::new(HashMap::new()),
            write_counts: RwLock::new(HashMap::new()),
            execute_counts: RwLock::new(HashMap::new()),
            total_reads: std::sync::atomic::AtomicU64::new(0),
            total_writes: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Record a memory access
    #[inline]
    pub fn record(&self, addr: u32, size: u32, kind: AccessKind) {
        if !self.enabled {
            return;
        }

        let page = addr / 0x1000;

        match kind {
            AccessKind::Read => {
                *self.read_counts.write().entry(page).or_insert(0) += 1;
                self.total_reads.fetch_add(size as u64, std::sync::atomic::Ordering::Relaxed);
            }
            AccessKind::Write => {
                *self.write_counts.write().entry(page).or_insert(0) += 1;
                self.total_writes.fetch_add(size as u64, std::sync::atomic::Ordering::Relaxed);
            }
            AccessKind::Execute => {
                *self.execute_counts.write().entry(page).or_insert(0) += 1;
            }
        }
    }

    /// Get total reads
    pub fn total_reads(&self) -> u64 {
        self.total_reads.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get total writes
    pub fn total_writes(&self) -> u64 {
        self.total_writes.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get hot pages (most accessed)
    pub fn hot_pages(&self, kind: AccessKind, top_n: usize) -> Vec<(u32, u64)> {
        let counts = match kind {
            AccessKind::Read => self.read_counts.read(),
            AccessKind::Write => self.write_counts.read(),
            AccessKind::Execute => self.execute_counts.read(),
        };

        let mut pages: Vec<_> = counts.iter().map(|(&k, &v)| (k, v)).collect();
        pages.sort_by(|a, b| b.1.cmp(&a.1));
        pages.truncate(top_n);
        pages
    }

    /// Get access count for a specific page
    pub fn page_accesses(&self, page: u32, kind: AccessKind) -> u64 {
        let counts = match kind {
            AccessKind::Read => self.read_counts.read(),
            AccessKind::Write => self.write_counts.read(),
            AccessKind::Execute => self.execute_counts.read(),
        };
        *counts.get(&page).unwrap_or(&0)
    }

    /// Reset all statistics
    pub fn reset(&self) {
        self.read_counts.write().clear();
        self.write_counts.write().clear();
        self.execute_counts.write().clear();
        self.total_reads.store(0, std::sync::atomic::Ordering::Relaxed);
        self.total_writes.store(0, std::sync::atomic::Ordering::Relaxed);
    }

    /// Enable or disable profiling
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.reset();
        }
    }

    /// Check if profiling is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for MemoryProfiler {
    fn default() -> Self {
        Self::new(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watchpoint_add_remove() {
        let manager = WatchpointManager::new();
        
        manager.add(0x1000, 4, WatchpointType::Write);
        assert_eq!(manager.count(), 1);
        assert!(manager.is_watched_page(0x1000));
        
        manager.remove(0x1000);
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_watchpoint_check_access() {
        let manager = WatchpointManager::new();
        
        manager.add(0x1000, 4, WatchpointType::Write);
        
        // Write should trigger
        assert!(manager.check_access(0x1000, 4, AccessKind::Write).is_some());
        
        // Read should not trigger
        assert!(manager.check_access(0x1000, 4, AccessKind::Read).is_none());
        
        // Address outside range should not trigger
        assert!(manager.check_access(0x2000, 4, AccessKind::Write).is_none());
    }

    #[test]
    fn test_smc_detector() {
        let detector = SmcDetector::new(true);
        
        // Mark page as executed
        detector.mark_executed(0x1000);
        
        // Write to executed page should be detected
        assert!(detector.check_write(0x1000).is_some());
        assert!(detector.check_write(0x1FFF).is_some()); // Same page
        
        // Different page should not be detected
        assert!(detector.check_write(0x2000).is_none());
    }

    #[test]
    fn test_cache_simulator() {
        let mut cache = CacheSimulator::new(CacheMode::L1Data);
        
        // First access should miss
        assert!(!cache.access(0x1000, false));
        assert_eq!(cache.stats().misses, 1);
        
        // Second access to same line should hit
        assert!(cache.access(0x1000, false));
        assert_eq!(cache.stats().hits, 1);
        
        // Access to different address in same line should hit
        assert!(cache.access(0x1004, false));
        assert_eq!(cache.stats().hits, 2);
    }

    #[test]
    fn test_memory_profiler() {
        let profiler = MemoryProfiler::new(true);
        
        profiler.record(0x1000, 4, AccessKind::Read);
        profiler.record(0x1000, 4, AccessKind::Read);
        profiler.record(0x2000, 4, AccessKind::Write);
        
        assert_eq!(profiler.total_reads(), 8);
        assert_eq!(profiler.total_writes(), 4);
        
        let hot_reads = profiler.hot_pages(AccessKind::Read, 10);
        assert_eq!(hot_reads.len(), 1);
        assert_eq!(hot_reads[0].1, 2); // 2 reads
    }
}
