//! Memory viewer panel for inspecting emulator memory

use eframe::egui;
use oc_memory::MemoryManager;
use std::sync::Arc;

/// Memory viewer panel state
pub struct MemoryViewer {
    /// Memory manager reference (optional, may not be connected yet)
    memory: Option<Arc<MemoryManager>>,
    /// Current address to view
    address: u32,
    /// Address input string
    address_input: String,
    /// Number of rows to display
    rows: usize,
    /// Bytes per row
    bytes_per_row: usize,
    /// Cached memory data
    cached_data: Vec<u8>,
    /// Last address that was read
    last_read_address: u32,
    /// Auto-refresh enabled
    auto_refresh: bool,
    /// Status message
    status_message: String,
    /// Show as big-endian (PS3 native)
    big_endian: bool,
    /// Display format
    display_format: DisplayFormat,
    /// Follow address mode
    follow_address: Option<u32>,
}

/// Display format for memory values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayFormat {
    Hex8,
    Hex16,
    Hex32,
    Hex64,
    Signed32,
    Float32,
    Ascii,
}

impl DisplayFormat {
    fn label(&self) -> &'static str {
        match self {
            DisplayFormat::Hex8 => "Hex (8-bit)",
            DisplayFormat::Hex16 => "Hex (16-bit)",
            DisplayFormat::Hex32 => "Hex (32-bit)",
            DisplayFormat::Hex64 => "Hex (64-bit)",
            DisplayFormat::Signed32 => "Signed (32-bit)",
            DisplayFormat::Float32 => "Float (32-bit)",
            DisplayFormat::Ascii => "ASCII",
        }
    }
}

impl MemoryViewer {
    /// Create a new memory viewer
    pub fn new() -> Self {
        Self {
            memory: None,
            address: 0x10000000, // Default to user memory base
            address_input: String::from("0x10000000"),
            rows: 16,
            bytes_per_row: 16,
            cached_data: vec![0; 16 * 16],
            last_read_address: 0,
            auto_refresh: false,
            status_message: String::from("Memory not connected"),
            big_endian: true,
            display_format: DisplayFormat::Hex8,
            follow_address: None,
        }
    }

    /// Connect to a memory manager
    pub fn connect(&mut self, memory: Arc<MemoryManager>) {
        self.memory = Some(memory);
        self.status_message = String::from("Connected to memory");
    }

    /// Disconnect from memory manager
    pub fn disconnect(&mut self) {
        self.memory = None;
        self.status_message = String::from("Memory not connected");
    }

    /// Check if connected to memory
    pub fn is_connected(&self) -> bool {
        self.memory.is_some()
    }

    /// Set the address to view
    pub fn set_address(&mut self, addr: u32) {
        self.address = addr;
        self.address_input = format!("0x{:08X}", addr);
    }

    /// Follow a specific address (for debugging)
    pub fn follow(&mut self, addr: u32) {
        self.follow_address = Some(addr);
        self.set_address(addr);
    }

    /// Stop following an address
    pub fn stop_following(&mut self) {
        self.follow_address = None;
    }

    /// Refresh memory data from the manager
    fn refresh_memory(&mut self) {
        let size = (self.rows * self.bytes_per_row) as u32;
        
        if let Some(ref memory) = self.memory {
            match memory.read_bytes(self.address, size) {
                Ok(data) => {
                    self.cached_data = data;
                    self.last_read_address = self.address;
                    self.status_message = format!("Read {} bytes at 0x{:08X}", size, self.address);
                }
                Err(e) => {
                    // Fill with zeros on error
                    self.cached_data = vec![0; size as usize];
                    self.status_message = format!("Error: {}", e);
                }
            }
        } else {
            self.cached_data = vec![0; size as usize];
            self.status_message = String::from("Memory not connected");
        }
    }

    /// Parse an address from string
    fn parse_address(s: &str) -> Option<u32> {
        let s = s.trim();
        if s.starts_with("0x") || s.starts_with("0X") {
            u32::from_str_radix(&s[2..], 16).ok()
        } else if s.starts_with("$") {
            u32::from_str_radix(&s[1..], 16).ok()
        } else {
            // Try hex first, then decimal
            u32::from_str_radix(s, 16).ok().or_else(|| s.parse().ok())
        }
    }

    /// Show the memory viewer panel
    pub fn show(&mut self, ui: &mut egui::Ui) {
        // Update follow address if set
        if let Some(addr) = self.follow_address {
            self.address = addr;
            self.address_input = format!("0x{:08X}", addr);
        }

        // Toolbar
        ui.horizontal(|ui| {
            ui.label("Address:");
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.address_input)
                    .desired_width(100.0)
                    .font(egui::TextStyle::Monospace)
            );
            
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Some(addr) = Self::parse_address(&self.address_input) {
                    self.address = addr;
                    self.refresh_memory();
                } else {
                    self.status_message = String::from("Invalid address format");
                }
            }

            if ui.button("Go").clicked() {
                if let Some(addr) = Self::parse_address(&self.address_input) {
                    self.address = addr;
                    self.refresh_memory();
                } else {
                    self.status_message = String::from("Invalid address format");
                }
            }

            ui.separator();

            // Navigation buttons
            if ui.button("◀◀").on_hover_text("Previous page").clicked() {
                self.address = self.address.saturating_sub((self.rows * self.bytes_per_row) as u32);
                self.address_input = format!("0x{:08X}", self.address);
                self.refresh_memory();
            }
            if ui.button("◀").on_hover_text("Previous row").clicked() {
                self.address = self.address.saturating_sub(self.bytes_per_row as u32);
                self.address_input = format!("0x{:08X}", self.address);
                self.refresh_memory();
            }
            if ui.button("▶").on_hover_text("Next row").clicked() {
                self.address = self.address.saturating_add(self.bytes_per_row as u32);
                self.address_input = format!("0x{:08X}", self.address);
                self.refresh_memory();
            }
            if ui.button("▶▶").on_hover_text("Next page").clicked() {
                self.address = self.address.saturating_add((self.rows * self.bytes_per_row) as u32);
                self.address_input = format!("0x{:08X}", self.address);
                self.refresh_memory();
            }

            ui.separator();

            if ui.button("🔄 Refresh").clicked() {
                self.refresh_memory();
            }

            ui.checkbox(&mut self.auto_refresh, "Auto");

            ui.separator();

            // Display format
            egui::ComboBox::from_id_salt("display_format")
                .selected_text(self.display_format.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.display_format, DisplayFormat::Hex8, "Hex (8-bit)");
                    ui.selectable_value(&mut self.display_format, DisplayFormat::Hex16, "Hex (16-bit)");
                    ui.selectable_value(&mut self.display_format, DisplayFormat::Hex32, "Hex (32-bit)");
                    ui.selectable_value(&mut self.display_format, DisplayFormat::Hex64, "Hex (64-bit)");
                    ui.selectable_value(&mut self.display_format, DisplayFormat::Signed32, "Signed (32-bit)");
                    ui.selectable_value(&mut self.display_format, DisplayFormat::Float32, "Float (32-bit)");
                    ui.selectable_value(&mut self.display_format, DisplayFormat::Ascii, "ASCII");
                });

            ui.checkbox(&mut self.big_endian, "BE");
        });

        ui.separator();

        // Auto-refresh
        if self.auto_refresh && self.memory.is_some() {
            self.refresh_memory();
            ui.ctx().request_repaint();
        }

        // Memory content
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                self.show_memory_grid(ui);
            });

        // Status bar
        ui.separator();
        ui.horizontal(|ui| {
            let status_color = if self.memory.is_some() {
                egui::Color32::GREEN
            } else {
                egui::Color32::GRAY
            };
            ui.colored_label(status_color, "●");
            ui.label(&self.status_message);

            if self.follow_address.is_some() {
                ui.separator();
                ui.label(egui::RichText::new("Following").color(egui::Color32::YELLOW));
                if ui.small_button("Stop").clicked() {
                    self.stop_following();
                }
            }
        });
    }

    /// Show the memory grid
    fn show_memory_grid(&mut self, ui: &mut egui::Ui) {
        match self.display_format {
            DisplayFormat::Hex8 => self.show_hex8_grid(ui),
            DisplayFormat::Hex16 => self.show_hex16_grid(ui),
            DisplayFormat::Hex32 => self.show_hex32_grid(ui),
            DisplayFormat::Hex64 => self.show_hex64_grid(ui),
            DisplayFormat::Signed32 => self.show_signed32_grid(ui),
            DisplayFormat::Float32 => self.show_float32_grid(ui),
            DisplayFormat::Ascii => self.show_ascii_grid(ui),
        }
    }

    /// Show hex8 (byte) grid
    fn show_hex8_grid(&self, ui: &mut egui::Ui) {
        // Header
        ui.horizontal(|ui| {
            ui.monospace("Address    ");
            for i in 0..self.bytes_per_row {
                ui.monospace(format!("{:02X} ", i));
            }
            ui.monospace(" ASCII");
        });

        ui.separator();

        // Data rows
        for row in 0..self.rows {
            let row_addr = self.address.wrapping_add((row * self.bytes_per_row) as u32);
            let row_start = row * self.bytes_per_row;
            let row_end = (row_start + self.bytes_per_row).min(self.cached_data.len());

            ui.horizontal(|ui| {
                // Address
                ui.monospace(format!("0x{:08X}  ", row_addr));

                // Hex bytes
                for i in row_start..row_end {
                    let byte = self.cached_data.get(i).copied().unwrap_or(0);
                    let color = if byte == 0 {
                        egui::Color32::GRAY
                    } else {
                        ui.visuals().text_color()
                    };
                    ui.monospace(egui::RichText::new(format!("{:02X} ", byte)).color(color));
                }

                // Padding for incomplete rows
                for _ in row_end..row_start + self.bytes_per_row {
                    ui.monospace("   ");
                }

                ui.monospace(" ");

                // ASCII representation
                let ascii: String = (row_start..row_end)
                    .map(|i| {
                        let byte = self.cached_data.get(i).copied().unwrap_or(0);
                        if byte >= 0x20 && byte <= 0x7E {
                            byte as char
                        } else {
                            '.'
                        }
                    })
                    .collect();
                ui.monospace(ascii);
            });
        }
    }

    /// Show hex16 grid
    fn show_hex16_grid(&self, ui: &mut egui::Ui) {
        let values_per_row = self.bytes_per_row / 2;

        ui.horizontal(|ui| {
            ui.monospace("Address    ");
            for i in 0..values_per_row {
                ui.monospace(format!("{:04X}  ", i * 2));
            }
        });

        ui.separator();

        for row in 0..self.rows {
            let row_addr = self.address.wrapping_add((row * self.bytes_per_row) as u32);
            let row_start = row * self.bytes_per_row;

            ui.horizontal(|ui| {
                ui.monospace(format!("0x{:08X}  ", row_addr));

                for i in 0..values_per_row {
                    let offset = row_start + i * 2;
                    if offset + 1 < self.cached_data.len() {
                        let value = if self.big_endian {
                            ((self.cached_data[offset] as u16) << 8) | (self.cached_data[offset + 1] as u16)
                        } else {
                            ((self.cached_data[offset + 1] as u16) << 8) | (self.cached_data[offset] as u16)
                        };
                        ui.monospace(format!("{:04X}  ", value));
                    } else {
                        ui.monospace("----  ");
                    }
                }
            });
        }
    }

    /// Show hex32 grid
    fn show_hex32_grid(&self, ui: &mut egui::Ui) {
        let values_per_row = self.bytes_per_row / 4;

        ui.horizontal(|ui| {
            ui.monospace("Address    ");
            for i in 0..values_per_row {
                ui.monospace(format!("{:08X}  ", i * 4));
            }
        });

        ui.separator();

        for row in 0..self.rows {
            let row_addr = self.address.wrapping_add((row * self.bytes_per_row) as u32);
            let row_start = row * self.bytes_per_row;

            ui.horizontal(|ui| {
                ui.monospace(format!("0x{:08X}  ", row_addr));

                for i in 0..values_per_row {
                    let offset = row_start + i * 4;
                    if offset + 3 < self.cached_data.len() {
                        let value = if self.big_endian {
                            ((self.cached_data[offset] as u32) << 24)
                                | ((self.cached_data[offset + 1] as u32) << 16)
                                | ((self.cached_data[offset + 2] as u32) << 8)
                                | (self.cached_data[offset + 3] as u32)
                        } else {
                            ((self.cached_data[offset + 3] as u32) << 24)
                                | ((self.cached_data[offset + 2] as u32) << 16)
                                | ((self.cached_data[offset + 1] as u32) << 8)
                                | (self.cached_data[offset] as u32)
                        };
                        ui.monospace(format!("{:08X}  ", value));
                    } else {
                        ui.monospace("--------  ");
                    }
                }
            });
        }
    }

    /// Show hex64 grid
    fn show_hex64_grid(&self, ui: &mut egui::Ui) {
        let values_per_row = self.bytes_per_row / 8;

        ui.horizontal(|ui| {
            ui.monospace("Address    ");
            for i in 0..values_per_row {
                ui.monospace(format!("{:016X}  ", i * 8));
            }
        });

        ui.separator();

        for row in 0..self.rows {
            let row_addr = self.address.wrapping_add((row * self.bytes_per_row) as u32);
            let row_start = row * self.bytes_per_row;

            ui.horizontal(|ui| {
                ui.monospace(format!("0x{:08X}  ", row_addr));

                for i in 0..values_per_row {
                    let offset = row_start + i * 8;
                    if offset + 7 < self.cached_data.len() {
                        let value = if self.big_endian {
                            ((self.cached_data[offset] as u64) << 56)
                                | ((self.cached_data[offset + 1] as u64) << 48)
                                | ((self.cached_data[offset + 2] as u64) << 40)
                                | ((self.cached_data[offset + 3] as u64) << 32)
                                | ((self.cached_data[offset + 4] as u64) << 24)
                                | ((self.cached_data[offset + 5] as u64) << 16)
                                | ((self.cached_data[offset + 6] as u64) << 8)
                                | (self.cached_data[offset + 7] as u64)
                        } else {
                            ((self.cached_data[offset + 7] as u64) << 56)
                                | ((self.cached_data[offset + 6] as u64) << 48)
                                | ((self.cached_data[offset + 5] as u64) << 40)
                                | ((self.cached_data[offset + 4] as u64) << 32)
                                | ((self.cached_data[offset + 3] as u64) << 24)
                                | ((self.cached_data[offset + 2] as u64) << 16)
                                | ((self.cached_data[offset + 1] as u64) << 8)
                                | (self.cached_data[offset] as u64)
                        };
                        ui.monospace(format!("{:016X}  ", value));
                    } else {
                        ui.monospace("----------------  ");
                    }
                }
            });
        }
    }

    /// Show signed 32-bit grid
    fn show_signed32_grid(&self, ui: &mut egui::Ui) {
        let values_per_row = self.bytes_per_row / 4;

        ui.horizontal(|ui| {
            ui.monospace("Address    ");
            for i in 0..values_per_row {
                ui.monospace(format!("{:>12}  ", i * 4));
            }
        });

        ui.separator();

        for row in 0..self.rows {
            let row_addr = self.address.wrapping_add((row * self.bytes_per_row) as u32);
            let row_start = row * self.bytes_per_row;

            ui.horizontal(|ui| {
                ui.monospace(format!("0x{:08X}  ", row_addr));

                for i in 0..values_per_row {
                    let offset = row_start + i * 4;
                    if offset + 3 < self.cached_data.len() {
                        let value = if self.big_endian {
                            ((self.cached_data[offset] as u32) << 24)
                                | ((self.cached_data[offset + 1] as u32) << 16)
                                | ((self.cached_data[offset + 2] as u32) << 8)
                                | (self.cached_data[offset + 3] as u32)
                        } else {
                            ((self.cached_data[offset + 3] as u32) << 24)
                                | ((self.cached_data[offset + 2] as u32) << 16)
                                | ((self.cached_data[offset + 1] as u32) << 8)
                                | (self.cached_data[offset] as u32)
                        };
                        let signed = value as i32;
                        ui.monospace(format!("{:>12}  ", signed));
                    } else {
                        ui.monospace("          --  ");
                    }
                }
            });
        }
    }

    /// Show float32 grid
    fn show_float32_grid(&self, ui: &mut egui::Ui) {
        let values_per_row = self.bytes_per_row / 4;

        ui.horizontal(|ui| {
            ui.monospace("Address    ");
            for i in 0..values_per_row {
                ui.monospace(format!("{:>14}  ", i * 4));
            }
        });

        ui.separator();

        for row in 0..self.rows {
            let row_addr = self.address.wrapping_add((row * self.bytes_per_row) as u32);
            let row_start = row * self.bytes_per_row;

            ui.horizontal(|ui| {
                ui.monospace(format!("0x{:08X}  ", row_addr));

                for i in 0..values_per_row {
                    let offset = row_start + i * 4;
                    if offset + 3 < self.cached_data.len() {
                        let value = if self.big_endian {
                            ((self.cached_data[offset] as u32) << 24)
                                | ((self.cached_data[offset + 1] as u32) << 16)
                                | ((self.cached_data[offset + 2] as u32) << 8)
                                | (self.cached_data[offset + 3] as u32)
                        } else {
                            ((self.cached_data[offset + 3] as u32) << 24)
                                | ((self.cached_data[offset + 2] as u32) << 16)
                                | ((self.cached_data[offset + 1] as u32) << 8)
                                | (self.cached_data[offset] as u32)
                        };
                        let float = f32::from_bits(value);
                        ui.monospace(format!("{:>14.6}  ", float));
                    } else {
                        ui.monospace("            --  ");
                    }
                }
            });
        }
    }

    /// Show ASCII grid
    fn show_ascii_grid(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.monospace("Address    ASCII");
        });

        ui.separator();

        for row in 0..self.rows {
            let row_addr = self.address.wrapping_add((row * self.bytes_per_row) as u32);
            let row_start = row * self.bytes_per_row;
            let row_end = (row_start + self.bytes_per_row).min(self.cached_data.len());

            ui.horizontal(|ui| {
                ui.monospace(format!("0x{:08X}  ", row_addr));

                let ascii: String = (row_start..row_end)
                    .map(|i| {
                        let byte = self.cached_data.get(i).copied().unwrap_or(0);
                        if byte >= 0x20 && byte <= 0x7E {
                            byte as char
                        } else {
                            '.'
                        }
                    })
                    .collect();
                ui.monospace(ascii);
            });
        }
    }
}

impl Default for MemoryViewer {
    fn default() -> Self {
        Self::new()
    }
}
