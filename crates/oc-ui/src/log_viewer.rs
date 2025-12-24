//! Log viewer panel for displaying emulator logs

use eframe::egui;
use std::collections::VecDeque;
use std::sync::Arc;
use parking_lot::RwLock;

/// Maximum number of log entries to keep
const MAX_LOG_ENTRIES: usize = 10000;

/// Log level for display
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn color(&self) -> egui::Color32 {
        match self {
            LogLevel::Trace => egui::Color32::GRAY,
            LogLevel::Debug => egui::Color32::LIGHT_BLUE,
            LogLevel::Info => egui::Color32::WHITE,
            LogLevel::Warn => egui::Color32::YELLOW,
            LogLevel::Error => egui::Color32::RED,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

/// A single log entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub target: String,
    pub message: String,
    pub timestamp: std::time::Instant,
}

/// Shared log buffer for collecting logs
pub type SharedLogBuffer = Arc<RwLock<VecDeque<LogEntry>>>;

/// Create a new shared log buffer
pub fn create_log_buffer() -> SharedLogBuffer {
    Arc::new(RwLock::new(VecDeque::with_capacity(MAX_LOG_ENTRIES)))
}

/// Add a log entry to the buffer
pub fn add_log_entry(buffer: &SharedLogBuffer, level: LogLevel, target: &str, message: &str) {
    let mut logs = buffer.write();
    if logs.len() >= MAX_LOG_ENTRIES {
        logs.pop_front();
    }
    logs.push_back(LogEntry {
        level,
        target: target.to_string(),
        message: message.to_string(),
        timestamp: std::time::Instant::now(),
    });
}

/// Log viewer panel state
pub struct LogViewer {
    /// Shared log buffer
    log_buffer: SharedLogBuffer,
    /// Filter by log level
    min_level: LogLevel,
    /// Filter by text
    filter_text: String,
    /// Auto-scroll to bottom
    auto_scroll: bool,
    /// Show timestamps
    show_timestamps: bool,
    /// Show targets (module names)
    show_targets: bool,
    /// Clear logs requested
    clear_requested: bool,
}

impl LogViewer {
    /// Create a new log viewer
    pub fn new() -> Self {
        Self {
            log_buffer: create_log_buffer(),
            min_level: LogLevel::Info,
            filter_text: String::new(),
            auto_scroll: true,
            show_timestamps: false,
            show_targets: true,
            clear_requested: false,
        }
    }

    /// Create a new log viewer with a shared buffer
    pub fn with_buffer(buffer: SharedLogBuffer) -> Self {
        Self {
            log_buffer: buffer,
            min_level: LogLevel::Info,
            filter_text: String::new(),
            auto_scroll: true,
            show_timestamps: false,
            show_targets: true,
            clear_requested: false,
        }
    }

    /// Get the shared log buffer for external log collection
    pub fn buffer(&self) -> SharedLogBuffer {
        Arc::clone(&self.log_buffer)
    }

    /// Add a log entry
    pub fn log(&self, level: LogLevel, target: &str, message: &str) {
        add_log_entry(&self.log_buffer, level, target, message);
    }

    /// Show the log viewer panel
    pub fn show(&mut self, ui: &mut egui::Ui) {
        // Handle clear request
        if self.clear_requested {
            self.log_buffer.write().clear();
            self.clear_requested = false;
        }

        // Toolbar
        ui.horizontal(|ui| {
            ui.label("Level:");
            egui::ComboBox::from_id_salt("log_level")
                .selected_text(self.min_level.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.min_level, LogLevel::Trace, "TRACE");
                    ui.selectable_value(&mut self.min_level, LogLevel::Debug, "DEBUG");
                    ui.selectable_value(&mut self.min_level, LogLevel::Info, "INFO");
                    ui.selectable_value(&mut self.min_level, LogLevel::Warn, "WARN");
                    ui.selectable_value(&mut self.min_level, LogLevel::Error, "ERROR");
                });

            ui.separator();

            ui.label("Filter:");
            ui.add(egui::TextEdit::singleline(&mut self.filter_text)
                .desired_width(150.0)
                .hint_text("Search logs..."));

            ui.separator();

            ui.checkbox(&mut self.auto_scroll, "Auto-scroll");
            ui.checkbox(&mut self.show_timestamps, "Timestamps");
            ui.checkbox(&mut self.show_targets, "Targets");

            ui.separator();

            if ui.button("🗑 Clear").clicked() {
                self.clear_requested = true;
            }

            // Show log count
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let count = self.log_buffer.read().len();
                ui.label(format!("{} entries", count));
            });
        });

        ui.separator();

        // Log content
        let logs = self.log_buffer.read();
        let filter_lower = self.filter_text.to_lowercase();
        
        let filtered_logs: Vec<_> = logs
            .iter()
            .filter(|entry| entry.level >= self.min_level)
            .filter(|entry| {
                if self.filter_text.is_empty() {
                    true
                } else {
                    entry.message.to_lowercase().contains(&filter_lower)
                        || entry.target.to_lowercase().contains(&filter_lower)
                }
            })
            .collect();

        let text_style = egui::TextStyle::Monospace;
        let row_height = ui.text_style_height(&text_style);
        
        let scroll_area = egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(self.auto_scroll);

        scroll_area.show_rows(ui, row_height, filtered_logs.len(), |ui, row_range| {
            for row in row_range {
                if let Some(entry) = filtered_logs.get(row) {
                    ui.horizontal(|ui| {
                        // Level badge
                        let level_text = egui::RichText::new(format!("[{}]", entry.level.label()))
                            .color(entry.level.color())
                            .monospace();
                        ui.label(level_text);

                        // Timestamp (optional)
                        if self.show_timestamps {
                            let elapsed = entry.timestamp.elapsed();
                            let timestamp = format!("{:.3}s", elapsed.as_secs_f64());
                            ui.label(egui::RichText::new(timestamp).monospace().weak());
                        }

                        // Target (optional)
                        if self.show_targets && !entry.target.is_empty() {
                            ui.label(egui::RichText::new(format!("[{}]", entry.target))
                                .monospace()
                                .color(egui::Color32::LIGHT_GRAY));
                        }

                        // Message
                        ui.label(egui::RichText::new(&entry.message).monospace());
                    });
                }
            }
        });
    }
}

impl Default for LogViewer {
    fn default() -> Self {
        Self::new()
    }
}
