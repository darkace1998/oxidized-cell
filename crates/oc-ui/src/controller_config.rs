//! Controller configuration UI for gamepad and input mapping

use eframe::egui;
use std::collections::HashMap;

/// Maximum number of buttons supported by controllers
const MAX_BUTTONS: usize = 16;

/// Maximum number of axes supported by controllers
const MAX_AXES: usize = 6;

/// Controller type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerType {
    None,
    DualShock3,
    DualSense,
    Xbox,
    Generic,
}

impl ControllerType {
    fn label(&self) -> &'static str {
        match self {
            ControllerType::None => "None",
            ControllerType::DualShock3 => "DualShock 3",
            ControllerType::DualSense => "DualSense",
            ControllerType::Xbox => "Xbox Controller",
            ControllerType::Generic => "Generic Gamepad",
        }
    }

    fn all() -> &'static [ControllerType] {
        &[
            ControllerType::None,
            ControllerType::DualShock3,
            ControllerType::DualSense,
            ControllerType::Xbox,
            ControllerType::Generic,
        ]
    }
}

/// PS3 controller button
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ps3Button {
    Cross,
    Circle,
    Square,
    Triangle,
    L1,
    L2,
    L3,
    R1,
    R2,
    R3,
    Start,
    Select,
    PSButton,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}

impl Ps3Button {
    fn label(&self) -> &'static str {
        match self {
            Ps3Button::Cross => "Cross (✕)",
            Ps3Button::Circle => "Circle (○)",
            Ps3Button::Square => "Square (□)",
            Ps3Button::Triangle => "Triangle (△)",
            Ps3Button::L1 => "L1",
            Ps3Button::L2 => "L2",
            Ps3Button::L3 => "L3 (Left Stick)",
            Ps3Button::R1 => "R1",
            Ps3Button::R2 => "R2",
            Ps3Button::R3 => "R3 (Right Stick)",
            Ps3Button::Start => "Start",
            Ps3Button::Select => "Select",
            Ps3Button::PSButton => "PS Button",
            Ps3Button::DPadUp => "D-Pad Up",
            Ps3Button::DPadDown => "D-Pad Down",
            Ps3Button::DPadLeft => "D-Pad Left",
            Ps3Button::DPadRight => "D-Pad Right",
        }
    }

    fn all() -> &'static [Ps3Button] {
        &[
            Ps3Button::Cross,
            Ps3Button::Circle,
            Ps3Button::Square,
            Ps3Button::Triangle,
            Ps3Button::L1,
            Ps3Button::L2,
            Ps3Button::L3,
            Ps3Button::R1,
            Ps3Button::R2,
            Ps3Button::R3,
            Ps3Button::Start,
            Ps3Button::Select,
            Ps3Button::PSButton,
            Ps3Button::DPadUp,
            Ps3Button::DPadDown,
            Ps3Button::DPadLeft,
            Ps3Button::DPadRight,
        ]
    }
}

/// PS3 controller axis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ps3Axis {
    LeftStickX,
    LeftStickY,
    RightStickX,
    RightStickY,
}

impl Ps3Axis {
    fn label(&self) -> &'static str {
        match self {
            Ps3Axis::LeftStickX => "Left Stick X",
            Ps3Axis::LeftStickY => "Left Stick Y",
            Ps3Axis::RightStickX => "Right Stick X",
            Ps3Axis::RightStickY => "Right Stick Y",
        }
    }

    fn all() -> &'static [Ps3Axis] {
        &[
            Ps3Axis::LeftStickX,
            Ps3Axis::LeftStickY,
            Ps3Axis::RightStickX,
            Ps3Axis::RightStickY,
        ]
    }
}

/// Button mapping (gamepad button index to PS3 button)
#[derive(Debug, Clone)]
pub struct ButtonMapping {
    /// Gamepad button index
    pub gamepad_button: u32,
    /// PS3 button
    pub ps3_button: Ps3Button,
}

/// Axis mapping (gamepad axis index to PS3 axis)
#[derive(Debug, Clone)]
pub struct AxisMapping {
    /// Gamepad axis index
    pub gamepad_axis: u32,
    /// PS3 axis
    pub ps3_axis: Ps3Axis,
    /// Invert axis
    pub inverted: bool,
    /// Dead zone (0.0 to 1.0)
    pub deadzone: f32,
}

/// Controller profile
#[derive(Debug, Clone)]
pub struct ControllerProfile {
    /// Profile name
    pub name: String,
    /// Controller type
    pub controller_type: ControllerType,
    /// Button mappings
    pub button_mappings: HashMap<Ps3Button, u32>,
    /// Axis mappings
    pub axis_mappings: HashMap<Ps3Axis, (u32, bool, f32)>, // (index, inverted, deadzone)
    /// Vibration enabled
    pub vibration_enabled: bool,
    /// Vibration strength (0.0 to 1.0)
    pub vibration_strength: f32,
}

impl Default for ControllerProfile {
    fn default() -> Self {
        let mut button_mappings = HashMap::new();
        // Default Xbox-style mapping
        button_mappings.insert(Ps3Button::Cross, 0);      // A
        button_mappings.insert(Ps3Button::Circle, 1);     // B
        button_mappings.insert(Ps3Button::Square, 2);     // X
        button_mappings.insert(Ps3Button::Triangle, 3);   // Y
        button_mappings.insert(Ps3Button::L1, 4);         // LB
        button_mappings.insert(Ps3Button::R1, 5);         // RB
        button_mappings.insert(Ps3Button::Select, 6);     // Back
        button_mappings.insert(Ps3Button::Start, 7);      // Start
        button_mappings.insert(Ps3Button::L3, 8);         // Left Stick Click
        button_mappings.insert(Ps3Button::R3, 9);         // Right Stick Click
        button_mappings.insert(Ps3Button::DPadUp, 10);
        button_mappings.insert(Ps3Button::DPadDown, 11);
        button_mappings.insert(Ps3Button::DPadLeft, 12);
        button_mappings.insert(Ps3Button::DPadRight, 13);
        button_mappings.insert(Ps3Button::PSButton, 14);  // Guide

        let mut axis_mappings = HashMap::new();
        axis_mappings.insert(Ps3Axis::LeftStickX, (0, false, 0.1));
        axis_mappings.insert(Ps3Axis::LeftStickY, (1, false, 0.1));
        axis_mappings.insert(Ps3Axis::RightStickX, (2, false, 0.1));
        axis_mappings.insert(Ps3Axis::RightStickY, (3, false, 0.1));

        Self {
            name: String::from("Default"),
            controller_type: ControllerType::Generic,
            button_mappings,
            axis_mappings,
            vibration_enabled: true,
            vibration_strength: 1.0,
        }
    }
}

/// Connected controller info
#[derive(Debug, Clone)]
pub struct ConnectedController {
    /// Controller index
    pub index: usize,
    /// Controller name
    pub name: String,
    /// Is connected
    pub connected: bool,
    /// Current button states
    pub buttons: Vec<bool>,
    /// Current axis values
    pub axes: Vec<f32>,
}

/// Controller configuration panel
pub struct ControllerConfig {
    /// Current port being configured (0-7 for PS3)
    current_port: usize,
    /// Controller profiles per port
    profiles: [ControllerProfile; 8],
    /// Connected controllers (detected from system)
    connected_controllers: Vec<ConnectedController>,
    /// Currently binding button (waiting for input)
    binding_button: Option<Ps3Button>,
    /// Currently binding axis
    binding_axis: Option<Ps3Axis>,
    /// Show advanced settings
    _show_advanced: bool,
    /// Status message
    status_message: String,
    /// Test mode active
    test_mode: bool,
}

impl ControllerConfig {
    /// Create a new controller configuration panel
    pub fn new() -> Self {
        Self {
            current_port: 0,
            profiles: std::array::from_fn(|_| ControllerProfile::default()),
            connected_controllers: Vec::new(),
            binding_button: None,
            binding_axis: None,
            _show_advanced: false,
            status_message: String::from("Controller configuration ready"),
            test_mode: false,
        }
    }

    /// Refresh connected controllers
    pub fn refresh_controllers(&mut self) {
        // In a real implementation, this would query the system for connected controllers
        // For now, add a mock controller
        self.connected_controllers = vec![
            ConnectedController {
                index: 0,
                name: String::from("Xbox Controller"),
                connected: true,
                buttons: vec![false; MAX_BUTTONS],
                axes: vec![0.0; MAX_AXES],
            },
        ];
        self.status_message = format!("Found {} controller(s)", self.connected_controllers.len());
    }

    /// Get profile for port
    pub fn get_profile(&self, port: usize) -> Option<&ControllerProfile> {
        self.profiles.get(port)
    }

    /// Get mutable profile for port
    pub fn get_profile_mut(&mut self, port: usize) -> Option<&mut ControllerProfile> {
        self.profiles.get_mut(port)
    }

    /// Show the controller configuration panel
    pub fn show(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        ui.heading("Controller Configuration");
        ui.add_space(5.0);

        // Port selector
        ui.horizontal(|ui| {
            ui.label("Port:");
            for port in 0..4 {
                let label = format!("Player {}", port + 1);
                if ui.selectable_label(self.current_port == port, &label).clicked() {
                    self.current_port = port;
                }
            }

            ui.separator();

            if ui.button("🔄 Refresh Controllers").clicked() {
                self.refresh_controllers();
            }

            ui.checkbox(&mut self.test_mode, "Test Mode");
        });

        ui.separator();

        // Connected controllers list
        ui.collapsing("Connected Controllers", |ui| {
            if self.connected_controllers.is_empty() {
                ui.label("No controllers detected.");
                ui.label("Click 'Refresh Controllers' to scan.");
            } else {
                for controller in &self.connected_controllers {
                    ui.horizontal(|ui| {
                        let status = if controller.connected { "🟢" } else { "🔴" };
                        ui.label(format!("{} {} ({})", status, controller.name, controller.index));
                    });
                }
            }
        });

        ui.separator();

        // Current port configuration
        let profile = &mut self.profiles[self.current_port];

        ui.horizontal(|ui| {
            ui.label("Controller Type:");
            egui::ComboBox::from_id_salt("controller_type")
                .selected_text(profile.controller_type.label())
                .show_ui(ui, |ui| {
                    for ct in ControllerType::all() {
                        if ui.selectable_value(&mut profile.controller_type, *ct, ct.label()).changed() {
                            changed = true;
                        }
                    }
                });
        });

        ui.add_space(10.0);

        // Button mappings
        ui.collapsing("Button Mappings", |ui| {
            egui::Grid::new("button_mappings")
                .num_columns(3)
                .striped(true)
                .spacing([20.0, 4.0])
                .show(ui, |ui| {
                    ui.strong("PS3 Button");
                    ui.strong("Mapped To");
                    ui.strong("Action");
                    ui.end_row();

                    for button in Ps3Button::all() {
                        ui.label(button.label());

                        let mapped = profile.button_mappings.get(button).copied();
                        let mapping_text = match mapped {
                            Some(idx) => format!("Button {}", idx),
                            None => "Not mapped".to_string(),
                        };

                        // Check if we're currently binding this button
                        if self.binding_button == Some(*button) {
                            ui.label(egui::RichText::new("Press button...").color(egui::Color32::YELLOW));
                            if ui.button("Cancel").clicked() {
                                self.binding_button = None;
                            }
                        } else {
                            ui.label(&mapping_text);
                            if ui.button("Bind").clicked() {
                                self.binding_button = Some(*button);
                                self.status_message = format!("Press a button for {}", button.label());
                            }
                        }

                        ui.end_row();
                    }
                });
        });

        ui.add_space(5.0);

        // Axis mappings
        ui.collapsing("Axis Mappings", |ui| {
            egui::Grid::new("axis_mappings")
                .num_columns(5)
                .striped(true)
                .spacing([15.0, 4.0])
                .show(ui, |ui| {
                    ui.strong("PS3 Axis");
                    ui.strong("Mapped To");
                    ui.strong("Inverted");
                    ui.strong("Deadzone");
                    ui.strong("Action");
                    ui.end_row();

                    for axis in Ps3Axis::all() {
                        ui.label(axis.label());

                        let (idx, inverted, deadzone) = profile.axis_mappings
                            .get(axis)
                            .copied()
                            .unwrap_or((0, false, 0.1));

                        ui.label(format!("Axis {}", idx));

                        let mut inv = inverted;
                        if ui.checkbox(&mut inv, "").changed() {
                            profile.axis_mappings.insert(*axis, (idx, inv, deadzone));
                            changed = true;
                        }

                        let mut dz = deadzone;
                        if ui.add(egui::Slider::new(&mut dz, 0.0..=0.5).show_value(true)).changed() {
                            profile.axis_mappings.insert(*axis, (idx, inverted, dz));
                            changed = true;
                        }

                        if self.binding_axis == Some(*axis) {
                            if ui.button("Cancel").clicked() {
                                self.binding_axis = None;
                            }
                        } else {
                            if ui.button("Bind").clicked() {
                                self.binding_axis = Some(*axis);
                                self.status_message = format!("Move an axis for {}", axis.label());
                            }
                        }

                        ui.end_row();
                    }
                });
        });

        ui.add_space(5.0);

        // Vibration settings
        ui.collapsing("Vibration Settings", |ui| {
            if ui.checkbox(&mut profile.vibration_enabled, "Enable Vibration").changed() {
                changed = true;
            }

            ui.horizontal(|ui| {
                ui.label("Strength:");
                if ui.add(egui::Slider::new(&mut profile.vibration_strength, 0.0..=1.0)
                    .show_value(true))
                    .changed()
                {
                    changed = true;
                }
            });

            if ui.button("Test Vibration").clicked() {
                self.status_message = String::from("Vibration test sent");
            }
        });

        // Test mode display
        if self.test_mode {
            ui.add_space(10.0);
            ui.separator();
            ui.label(egui::RichText::new("Test Mode").strong());

            if let Some(controller) = self.connected_controllers.first() {
                ui.horizontal(|ui| {
                    ui.label("Buttons: ");
                    for (i, pressed) in controller.buttons.iter().enumerate() {
                        let color = if *pressed { egui::Color32::GREEN } else { egui::Color32::GRAY };
                        ui.colored_label(color, format!("{}", i));
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Axes: ");
                    for (i, value) in controller.axes.iter().enumerate() {
                        ui.label(format!("A{}: {:.2}", i, value));
                    }
                });

                // Request repaint for live updates
                ui.ctx().request_repaint();
            } else {
                ui.label("No controller connected for testing.");
            }
        }

        // Status bar
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&self.status_message).small());
        });

        changed
    }
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self::new()
    }
}
