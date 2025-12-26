//! Frame timing and pacing for RSX
//!
//! This module provides frame timing management, including:
//! - Frame rate limiting
//! - VSync configuration
//! - Frame time statistics
//! - Adaptive sync support

use std::time::{Duration, Instant};
use std::collections::VecDeque;

/// VSync mode configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VSyncMode {
    /// VSync disabled - immediate present
    Off,
    /// VSync enabled - wait for vertical blank
    On,
    /// Adaptive VSync - VSync on unless frame rate drops below refresh rate
    Adaptive,
    /// Triple buffering with VSync
    TripleBuffer,
    /// Fast sync - tear only when frame rate exceeds refresh
    Fast,
}

impl Default for VSyncMode {
    fn default() -> Self {
        Self::On
    }
}

/// Frame rate limit configuration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FrameRateLimit {
    /// No limit
    Unlimited,
    /// Sync to display refresh rate
    VSync,
    /// Fixed FPS limit
    Fixed(f64),
    /// Half of display refresh rate
    HalfRefresh,
    /// Quarter of display refresh rate
    QuarterRefresh,
}

impl Default for FrameRateLimit {
    fn default() -> Self {
        Self::VSync
    }
}

impl FrameRateLimit {
    /// Get the target frame time for a limit
    pub fn target_frame_time(&self, refresh_rate: f64) -> Option<Duration> {
        match self {
            Self::Unlimited => None,
            Self::VSync => Some(Duration::from_secs_f64(1.0 / refresh_rate)),
            Self::Fixed(fps) => Some(Duration::from_secs_f64(1.0 / fps)),
            Self::HalfRefresh => Some(Duration::from_secs_f64(2.0 / refresh_rate)),
            Self::QuarterRefresh => Some(Duration::from_secs_f64(4.0 / refresh_rate)),
        }
    }
}

/// Frame timing manager
pub struct FrameTimer {
    /// VSync mode
    vsync_mode: VSyncMode,
    /// Frame rate limit
    frame_rate_limit: FrameRateLimit,
    /// Display refresh rate (Hz)
    refresh_rate: f64,
    /// Last frame timestamp
    last_frame_time: Instant,
    /// Frame time history for averaging
    frame_times: VecDeque<Duration>,
    /// Maximum history size
    history_size: usize,
    /// Target frame time
    target_frame_time: Option<Duration>,
    /// Total frames rendered
    total_frames: u64,
    /// Frames dropped due to slow rendering
    dropped_frames: u64,
    /// Whether frame pacing is enabled
    enabled: bool,
    /// Accumulated sleep debt (for precision)
    sleep_debt: Duration,
}

impl FrameTimer {
    /// Create a new frame timer
    pub fn new() -> Self {
        Self::with_refresh_rate(60.0)
    }

    /// Create a frame timer with specified refresh rate
    pub fn with_refresh_rate(refresh_rate: f64) -> Self {
        let mut timer = Self {
            vsync_mode: VSyncMode::On,
            frame_rate_limit: FrameRateLimit::VSync,
            refresh_rate,
            last_frame_time: Instant::now(),
            frame_times: VecDeque::with_capacity(120),
            history_size: 120,
            target_frame_time: None,
            total_frames: 0,
            dropped_frames: 0,
            enabled: true,
            sleep_debt: Duration::ZERO,
        };
        timer.update_target_frame_time();
        timer
    }

    /// Set VSync mode
    pub fn set_vsync_mode(&mut self, mode: VSyncMode) {
        self.vsync_mode = mode;
    }

    /// Get VSync mode
    pub fn vsync_mode(&self) -> VSyncMode {
        self.vsync_mode
    }

    /// Set frame rate limit
    pub fn set_frame_rate_limit(&mut self, limit: FrameRateLimit) {
        self.frame_rate_limit = limit;
        self.update_target_frame_time();
    }

    /// Get frame rate limit
    pub fn frame_rate_limit(&self) -> FrameRateLimit {
        self.frame_rate_limit
    }

    /// Set display refresh rate
    pub fn set_refresh_rate(&mut self, rate: f64) {
        self.refresh_rate = rate.max(1.0);
        self.update_target_frame_time();
    }

    /// Get display refresh rate
    pub fn refresh_rate(&self) -> f64 {
        self.refresh_rate
    }

    /// Update target frame time based on current settings
    fn update_target_frame_time(&mut self) {
        self.target_frame_time = self.frame_rate_limit.target_frame_time(self.refresh_rate);
    }

    /// Enable/disable frame pacing
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if frame pacing is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Begin a new frame - call at the start of each frame
    pub fn begin_frame(&mut self) {
        self.last_frame_time = Instant::now();
    }

    /// End a frame - call at the end of each frame
    /// Returns true if the frame should be presented, false if dropped
    pub fn end_frame(&mut self) -> bool {
        let now = Instant::now();
        let frame_time = now.duration_since(self.last_frame_time);
        
        // Record frame time
        if self.frame_times.len() >= self.history_size {
            self.frame_times.pop_front();
        }
        self.frame_times.push_back(frame_time);
        
        self.total_frames += 1;

        // Wait if needed for frame pacing
        if self.enabled {
            if let Some(target) = self.target_frame_time {
                let elapsed = now.duration_since(self.last_frame_time);
                if elapsed < target {
                    let sleep_time = target - elapsed + self.sleep_debt;
                    if sleep_time > Duration::from_micros(100) {
                        let sleep_start = Instant::now();
                        std::thread::sleep(sleep_time);
                        let actual_sleep = sleep_start.elapsed();
                        
                        // Track sleep accuracy for compensation
                        if actual_sleep > sleep_time {
                            self.sleep_debt = Duration::ZERO;
                        } else {
                            self.sleep_debt = sleep_time - actual_sleep;
                        }
                    }
                } else if elapsed > target * 2 {
                    // Frame took too long - mark as dropped
                    self.dropped_frames += 1;
                    return false;
                }
            }
        }

        true
    }

    /// Get the average frame time over the history
    pub fn average_frame_time(&self) -> Duration {
        if self.frame_times.is_empty() {
            return Duration::ZERO;
        }
        
        let sum: Duration = self.frame_times.iter().sum();
        sum / self.frame_times.len() as u32
    }

    /// Get the current FPS based on recent frame times
    pub fn current_fps(&self) -> f64 {
        let avg = self.average_frame_time();
        if avg.is_zero() {
            0.0
        } else {
            1.0 / avg.as_secs_f64()
        }
    }

    /// Get the minimum frame time in history
    pub fn min_frame_time(&self) -> Duration {
        self.frame_times.iter().min().copied().unwrap_or(Duration::ZERO)
    }

    /// Get the maximum frame time in history
    pub fn max_frame_time(&self) -> Duration {
        self.frame_times.iter().max().copied().unwrap_or(Duration::ZERO)
    }

    /// Get frame time variance
    pub fn frame_time_variance(&self) -> f64 {
        if self.frame_times.len() < 2 {
            return 0.0;
        }

        let avg = self.average_frame_time().as_secs_f64();
        let variance: f64 = self.frame_times
            .iter()
            .map(|t| {
                let diff = t.as_secs_f64() - avg;
                diff * diff
            })
            .sum::<f64>() / (self.frame_times.len() - 1) as f64;
        
        variance
    }

    /// Get frame time standard deviation
    pub fn frame_time_std_dev(&self) -> Duration {
        Duration::from_secs_f64(self.frame_time_variance().sqrt())
    }

    /// Get the 1% low FPS (99th percentile frame time)
    pub fn percentile_1_low(&self) -> f64 {
        if self.frame_times.is_empty() {
            return 0.0;
        }

        let mut times: Vec<_> = self.frame_times.iter().collect();
        times.sort();
        
        let idx = (times.len() as f64 * 0.99).floor() as usize;
        let idx = idx.min(times.len() - 1);
        
        let frame_time = times[idx].as_secs_f64();
        if frame_time > 0.0 {
            1.0 / frame_time
        } else {
            0.0
        }
    }

    /// Get total frames rendered
    pub fn total_frames(&self) -> u64 {
        self.total_frames
    }

    /// Get dropped frame count
    pub fn dropped_frames(&self) -> u64 {
        self.dropped_frames
    }

    /// Get drop rate percentage
    pub fn drop_rate(&self) -> f64 {
        if self.total_frames == 0 {
            0.0
        } else {
            (self.dropped_frames as f64 / self.total_frames as f64) * 100.0
        }
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.frame_times.clear();
        self.total_frames = 0;
        self.dropped_frames = 0;
        self.sleep_debt = Duration::ZERO;
    }

    /// Get comprehensive frame timing statistics
    pub fn stats(&self) -> FrameTimerStats {
        FrameTimerStats {
            current_fps: self.current_fps(),
            average_frame_time: self.average_frame_time(),
            min_frame_time: self.min_frame_time(),
            max_frame_time: self.max_frame_time(),
            frame_time_std_dev: self.frame_time_std_dev(),
            percentile_1_low: self.percentile_1_low(),
            total_frames: self.total_frames,
            dropped_frames: self.dropped_frames,
            drop_rate: self.drop_rate(),
            target_fps: self.target_frame_time
                .map(|t| 1.0 / t.as_secs_f64())
                .unwrap_or(0.0),
            refresh_rate: self.refresh_rate,
            vsync_mode: self.vsync_mode,
        }
    }
}

impl Default for FrameTimer {
    fn default() -> Self {
        Self::new()
    }
}

/// Comprehensive frame timing statistics
#[derive(Debug, Clone)]
pub struct FrameTimerStats {
    /// Current FPS (based on average frame time)
    pub current_fps: f64,
    /// Average frame time
    pub average_frame_time: Duration,
    /// Minimum frame time in history
    pub min_frame_time: Duration,
    /// Maximum frame time in history
    pub max_frame_time: Duration,
    /// Frame time standard deviation
    pub frame_time_std_dev: Duration,
    /// 1% low FPS
    pub percentile_1_low: f64,
    /// Total frames rendered
    pub total_frames: u64,
    /// Dropped frames count
    pub dropped_frames: u64,
    /// Drop rate percentage
    pub drop_rate: f64,
    /// Target FPS
    pub target_fps: f64,
    /// Display refresh rate
    pub refresh_rate: f64,
    /// Current VSync mode
    pub vsync_mode: VSyncMode,
}

/// Frame time smoother for consistent frame pacing
pub struct FrameSmoother {
    /// Recent frame times for prediction
    recent_times: VecDeque<Duration>,
    /// Smoothing window size
    window_size: usize,
    /// Prediction weight (0.0 = no prediction, 1.0 = full prediction)
    prediction_weight: f32,
}

impl FrameSmoother {
    /// Create a new frame smoother
    pub fn new(window_size: usize) -> Self {
        Self {
            recent_times: VecDeque::with_capacity(window_size),
            window_size,
            prediction_weight: 0.5,
        }
    }

    /// Add a frame time sample
    pub fn add_sample(&mut self, frame_time: Duration) {
        if self.recent_times.len() >= self.window_size {
            self.recent_times.pop_front();
        }
        self.recent_times.push_back(frame_time);
    }

    /// Predict the next frame time
    pub fn predict_next(&self) -> Duration {
        if self.recent_times.is_empty() {
            return Duration::from_millis(16); // ~60fps default
        }

        // Simple weighted average prediction
        let sum: Duration = self.recent_times.iter().sum();
        let avg = sum / self.recent_times.len() as u32;

        // Weight recent samples more heavily
        if let Some(last) = self.recent_times.back() {
            let weighted = Duration::from_secs_f64(
                avg.as_secs_f64() * (1.0 - self.prediction_weight as f64)
                + last.as_secs_f64() * self.prediction_weight as f64
            );
            weighted
        } else {
            avg
        }
    }

    /// Set prediction weight
    pub fn set_prediction_weight(&mut self, weight: f32) {
        self.prediction_weight = weight.clamp(0.0, 1.0);
    }

    /// Clear history
    pub fn clear(&mut self) {
        self.recent_times.clear();
    }
}

impl Default for FrameSmoother {
    fn default() -> Self {
        Self::new(10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vsync_mode_default() {
        assert_eq!(VSyncMode::default(), VSyncMode::On);
    }

    #[test]
    fn test_frame_rate_limit_target_time() {
        let limit = FrameRateLimit::Fixed(60.0);
        let target = limit.target_frame_time(60.0).unwrap();
        assert!((target.as_secs_f64() - 1.0/60.0).abs() < 0.0001);
    }

    #[test]
    fn test_frame_timer_creation() {
        let timer = FrameTimer::new();
        assert_eq!(timer.refresh_rate(), 60.0);
        assert!(timer.is_enabled());
    }

    #[test]
    fn test_frame_timer_custom_refresh() {
        let timer = FrameTimer::with_refresh_rate(144.0);
        assert_eq!(timer.refresh_rate(), 144.0);
    }

    #[test]
    fn test_frame_timer_fps_calculation() {
        let mut timer = FrameTimer::new();
        
        // Simulate some frames at ~60fps
        for _ in 0..10 {
            timer.begin_frame();
            std::thread::sleep(Duration::from_millis(16));
            timer.end_frame();
        }
        
        // FPS should be roughly 60 (allowing for test environment variance)
        let fps = timer.current_fps();
        assert!(fps > 30.0 && fps < 120.0, "FPS was {}", fps);
    }

    #[test]
    fn test_frame_timer_stats() {
        let timer = FrameTimer::new();
        let stats = timer.stats();
        
        assert_eq!(stats.total_frames, 0);
        assert_eq!(stats.dropped_frames, 0);
        assert_eq!(stats.refresh_rate, 60.0);
    }

    #[test]
    fn test_frame_smoother() {
        let mut smoother = FrameSmoother::new(5);
        
        for _ in 0..5 {
            smoother.add_sample(Duration::from_millis(16));
        }
        
        let prediction = smoother.predict_next();
        assert!((prediction.as_millis() as i64 - 16).abs() <= 1);
    }

    #[test]
    fn test_frame_rate_limit_unlimited() {
        let limit = FrameRateLimit::Unlimited;
        assert!(limit.target_frame_time(60.0).is_none());
    }

    #[test]
    fn test_frame_timer_reset() {
        let mut timer = FrameTimer::new();
        
        timer.begin_frame();
        timer.end_frame();
        timer.begin_frame();
        timer.end_frame();
        
        assert!(timer.total_frames() >= 2);
        
        timer.reset_stats();
        assert_eq!(timer.total_frames(), 0);
        assert_eq!(timer.dropped_frames(), 0);
    }
}
