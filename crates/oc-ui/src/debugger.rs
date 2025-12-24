//! Debugger UI

use eframe::egui;

/// Debugger view state
pub struct DebuggerView {
    /// Current debugger tab
    current_tab: DebuggerTab,
    /// Memory viewer address
    memory_address: String,
    /// Memory viewer data (mock data for now)
    memory_data: Vec<u8>,
    /// Disassembly address
    disasm_address: String,
    /// Breakpoints
    breakpoints: Vec<u32>,
    /// New breakpoint address input
    breakpoint_input: String,
}

/// Debugger tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DebuggerTab {
    Registers,
    Memory,
    Disassembly,
    Breakpoints,
}

impl DebuggerView {
    /// Create a new debugger view
    pub fn new() -> Self {
        Self {
            current_tab: DebuggerTab::Registers,
            memory_address: String::from("0x00000000"),
            memory_data: vec![0; 256],
            disasm_address: String::from("0x00000000"),
            breakpoints: Vec::new(),
            breakpoint_input: String::new(),
        }
    }

    /// Show the debugger view
    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.current_tab, DebuggerTab::Registers, "Registers");
            ui.selectable_value(&mut self.current_tab, DebuggerTab::Memory, "Memory");
            ui.selectable_value(&mut self.current_tab, DebuggerTab::Disassembly, "Disassembly");
            ui.selectable_value(&mut self.current_tab, DebuggerTab::Breakpoints, "Breakpoints");
        });

        ui.separator();

        // Control buttons
        ui.horizontal(|ui| {
            if ui.button("▶ Continue").clicked() {
                // TODO: Resume execution
            }
            if ui.button("⏸ Pause").clicked() {
                // TODO: Pause execution
            }
            if ui.button("⏭ Step").clicked() {
                // TODO: Single step
            }
            if ui.button("⏩ Step Over").clicked() {
                // TODO: Step over
            }
        });

        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            match self.current_tab {
                DebuggerTab::Registers => self.show_registers(ui),
                DebuggerTab::Memory => self.show_memory(ui),
                DebuggerTab::Disassembly => self.show_disassembly(ui),
                DebuggerTab::Breakpoints => self.show_breakpoints(ui),
            }
        });
    }

    fn show_registers(&self, ui: &mut egui::Ui) {
        ui.heading("PPU Registers");
        ui.add_space(10.0);

        // General Purpose Registers
        ui.label(egui::RichText::new("General Purpose Registers (GPRs)").strong());
        egui::Grid::new("gpr_grid")
            .striped(true)
            .num_columns(4)
            .show(ui, |ui| {
                for i in 0..32 {
                    if i % 4 == 0 && i > 0 {
                        ui.end_row();
                    }
                    ui.label(format!("R{:02}:", i));
                    ui.label(egui::RichText::new("0x0000000000000000").monospace());
                }
            });

        ui.add_space(10.0);

        // Floating Point Registers
        ui.label(egui::RichText::new("Floating Point Registers (FPRs)").strong());
        egui::Grid::new("fpr_grid")
            .striped(true)
            .num_columns(4)
            .show(ui, |ui| {
                for i in 0..32 {
                    if i % 4 == 0 && i > 0 {
                        ui.end_row();
                    }
                    ui.label(format!("F{:02}:", i));
                    ui.label(egui::RichText::new("0.0").monospace());
                }
            });

        ui.add_space(10.0);

        // Special Registers
        ui.label(egui::RichText::new("Special Registers").strong());
        egui::Grid::new("special_regs")
            .striped(true)
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("PC:");
                ui.label(egui::RichText::new("0x00000000").monospace());
                ui.end_row();

                ui.label("LR:");
                ui.label(egui::RichText::new("0x00000000").monospace());
                ui.end_row();

                ui.label("CTR:");
                ui.label(egui::RichText::new("0x00000000").monospace());
                ui.end_row();

                ui.label("CR:");
                ui.label(egui::RichText::new("0x00000000").monospace());
                ui.end_row();

                ui.label("XER:");
                ui.label(egui::RichText::new("0x00000000").monospace());
                ui.end_row();

                ui.label("FPSCR:");
                ui.label(egui::RichText::new("0x00000000").monospace());
                ui.end_row();
            });
    }

    fn show_memory(&mut self, ui: &mut egui::Ui) {
        ui.heading("Memory Viewer");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Address:");
            ui.text_edit_singleline(&mut self.memory_address);
            if ui.button("Go").clicked() {
                // TODO: Load memory at address
            }
        });

        ui.add_space(10.0);

        // Hex dump display
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.monospace("Address    00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F  ASCII");
            ui.separator();

            for (i, chunk) in self.memory_data.chunks(16).enumerate() {
                let addr = i * 16;
                let hex: String = chunk
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                
                let ascii: String = chunk
                    .iter()
                    .map(|&b| {
                        if b >= 0x20 && b <= 0x7E {
                            b as char
                        } else {
                            '.'
                        }
                    })
                    .collect();

                ui.monospace(format!("0x{:08X}  {:48}  {}", addr, hex, ascii));
            }
        });
    }

    fn show_disassembly(&mut self, ui: &mut egui::Ui) {
        ui.heading("Disassembly");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Address:");
            ui.text_edit_singleline(&mut self.disasm_address);
            if ui.button("Go").clicked() {
                // TODO: Disassemble at address
            }
        });

        ui.add_space(10.0);

        // Disassembly display (mock data)
        egui::Grid::new("disasm_grid")
            .striped(true)
            .num_columns(3)
            .show(ui, |ui| {
                ui.strong("Address");
                ui.strong("Bytes");
                ui.strong("Instruction");
                ui.end_row();

                // Mock disassembly
                let mock_instructions = [
                    ("0x00000000", "7C 08 02 A6", "mflr    r0"),
                    ("0x00000004", "FB E1 FF F8", "std     r31, -8(r1)"),
                    ("0x00000008", "F8 21 FF 91", "stdu    r1, -112(r1)"),
                    ("0x0000000C", "7C 3F 0B 78", "mr      r31, r1"),
                    ("0x00000010", "F8 01 00 80", "std     r0, 128(r1)"),
                    ("0x00000014", "38 60 00 00", "li      r3, 0"),
                    ("0x00000018", "48 00 00 01", "bl      0x0000001C"),
                ];

                for (addr, bytes, inst) in mock_instructions {
                    ui.label(egui::RichText::new(addr).monospace());
                    ui.label(egui::RichText::new(bytes).monospace());
                    ui.label(egui::RichText::new(inst).monospace());
                    ui.end_row();
                }
            });
    }

    fn show_breakpoints(&mut self, ui: &mut egui::Ui) {
        ui.heading("Breakpoints");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Add Breakpoint:");
            ui.text_edit_singleline(&mut self.breakpoint_input);
            if ui.button("Add").clicked() {
                if let Ok(addr) = self.parse_address(&self.breakpoint_input) {
                    self.breakpoints.push(addr);
                    self.breakpoint_input.clear();
                }
            }
        });

        ui.add_space(10.0);

        if self.breakpoints.is_empty() {
            ui.label("No breakpoints set.");
        } else {
            egui::Grid::new("breakpoints_grid")
                .striped(true)
                .num_columns(3)
                .show(ui, |ui| {
                    ui.strong("Address");
                    ui.strong("Enabled");
                    ui.strong("Actions");
                    ui.end_row();

                    let mut to_remove = None;

                    for (i, &addr) in self.breakpoints.iter().enumerate() {
                        ui.label(egui::RichText::new(format!("0x{:08X}", addr)).monospace());
                        ui.checkbox(&mut true, "");
                        if ui.button("Remove").clicked() {
                            to_remove = Some(i);
                        }
                        ui.end_row();
                    }

                    if let Some(idx) = to_remove {
                        self.breakpoints.remove(idx);
                    }
                });
        }
    }

    fn parse_address(&self, s: &str) -> Result<u32, ()> {
        let s = s.trim();
        if s.starts_with("0x") || s.starts_with("0X") {
            u32::from_str_radix(&s[2..], 16).map_err(|_| ())
        } else {
            s.parse().map_err(|_| ())
        }
    }
}

impl Default for DebuggerView {
    fn default() -> Self {
        Self::new()
    }
}
