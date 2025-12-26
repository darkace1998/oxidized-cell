//! Guitar Hero and Rock Band Controller Emulation
//!
//! Support for music game peripherals including:
//! - Guitar controllers (5-fret and 6-fret)
//! - Drum kits (4-pad and Pro drums)
//! - DJ Hero turntable (limited)
//! - Microphone passthrough

use std::time::Instant;

/// Guitar fret buttons
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GuitarFrets(pub u8);

impl GuitarFrets {
    /// Green fret (1st)
    pub const GREEN: Self = Self(0x01);
    /// Red fret (2nd)
    pub const RED: Self = Self(0x02);
    /// Yellow fret (3rd)
    pub const YELLOW: Self = Self(0x04);
    /// Blue fret (4th)
    pub const BLUE: Self = Self(0x08);
    /// Orange fret (5th)
    pub const ORANGE: Self = Self(0x10);
    /// 6th fret (GH Live controllers)
    pub const FRET6: Self = Self(0x20);

    pub fn empty() -> Self {
        Self(0)
    }

    pub fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    /// Count pressed frets
    pub fn count(&self) -> u8 {
        self.0.count_ones() as u8
    }
}

/// Guitar special buttons
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GuitarButtons(pub u16);

impl GuitarButtons {
    /// Strum up
    pub const STRUM_UP: Self = Self(0x0001);
    /// Strum down
    pub const STRUM_DOWN: Self = Self(0x0002);
    /// Star power / Overdrive
    pub const STAR_POWER: Self = Self(0x0004);
    /// Start button
    pub const START: Self = Self(0x0008);
    /// Select button
    pub const SELECT: Self = Self(0x0010);
    /// PS/Guide button
    pub const PS: Self = Self(0x0020);
    /// Tilt sensor activated
    pub const TILT: Self = Self(0x0040);
    /// Touch strip (GH World Tour+)
    pub const TOUCH: Self = Self(0x0080);

    pub fn empty() -> Self {
        Self(0)
    }

    pub fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}

/// Guitar controller type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuitarType {
    /// Standard 5-fret guitar (GH, RB)
    FiveFret,
    /// 6-fret guitar (GH Live)
    SixFret,
    /// Bass guitar (same as 5-fret but tagged)
    Bass,
}

/// Guitar controller state
#[derive(Debug, Clone, Default)]
pub struct GuitarState {
    /// Fret buttons pressed
    pub frets: GuitarFrets,
    /// Special buttons pressed
    pub buttons: GuitarButtons,
    /// Whammy bar position (0-255, 128 = center)
    pub whammy: u8,
    /// Tilt angle (0-255, 128 = level)
    pub tilt: u8,
    /// Touch strip position (for supported guitars, 0-127)
    pub touch_position: u8,
    /// Touch strip active
    pub touch_active: bool,
}

impl GuitarState {
    pub fn new() -> Self {
        Self {
            frets: GuitarFrets::empty(),
            buttons: GuitarButtons::empty(),
            whammy: 128,
            tilt: 128,
            touch_position: 0,
            touch_active: false,
        }
    }

    /// Check if currently strumming
    pub fn is_strumming(&self) -> bool {
        self.buttons.contains(GuitarButtons::STRUM_UP) ||
        self.buttons.contains(GuitarButtons::STRUM_DOWN)
    }

    /// Check if star power is tilted
    pub fn is_tilted(&self) -> bool {
        self.tilt > 180 || self.buttons.contains(GuitarButtons::TILT)
    }
}

/// Guitar controller
#[derive(Debug)]
pub struct GuitarController {
    /// Controller index
    pub index: u8,
    /// Guitar type
    pub guitar_type: GuitarType,
    /// Current state
    pub state: GuitarState,
    /// Connected
    pub connected: bool,
    /// Last update
    last_update: Instant,
}

impl GuitarController {
    pub fn new(index: u8, guitar_type: GuitarType) -> Self {
        Self {
            index,
            guitar_type,
            state: GuitarState::new(),
            connected: false,
            last_update: Instant::now(),
        }
    }

    pub fn connect(&mut self) {
        self.connected = true;
        tracing::info!("Guitar {} connected ({:?})", self.index, self.guitar_type);
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
        self.state = GuitarState::new();
        tracing::info!("Guitar {} disconnected", self.index);
    }

    pub fn set_fret(&mut self, fret: GuitarFrets, pressed: bool) {
        if pressed {
            self.state.frets.insert(fret);
        } else {
            self.state.frets.remove(fret);
        }
    }

    pub fn set_button(&mut self, button: GuitarButtons, pressed: bool) {
        if pressed {
            self.state.buttons.insert(button);
        } else {
            self.state.buttons.remove(button);
        }
    }

    pub fn set_whammy(&mut self, value: u8) {
        self.state.whammy = value;
    }

    pub fn set_tilt(&mut self, value: u8) {
        self.state.tilt = value;
        // Auto-set tilt button if above threshold
        if value > 180 {
            self.state.buttons.insert(GuitarButtons::TILT);
        } else {
            self.state.buttons.remove(GuitarButtons::TILT);
        }
    }

    pub fn update(&mut self) {
        self.last_update = Instant::now();
    }
}

/// Drum pad indices
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DrumPads(pub u16);

impl DrumPads {
    /// Red pad (snare position)
    pub const RED: Self = Self(0x0001);
    /// Yellow pad (hi-hat position)
    pub const YELLOW: Self = Self(0x0002);
    /// Blue pad (tom position)
    pub const BLUE: Self = Self(0x0004);
    /// Green pad (floor tom position)
    pub const GREEN: Self = Self(0x0008);
    /// Orange pad (5th pad for RB Pro)
    pub const ORANGE: Self = Self(0x0010);
    /// Kick/Bass pedal
    pub const KICK: Self = Self(0x0020);
    /// Second kick pedal (double bass)
    pub const KICK2: Self = Self(0x0040);
    /// Yellow cymbal (Pro)
    pub const YELLOW_CYMBAL: Self = Self(0x0080);
    /// Blue cymbal (Pro)
    pub const BLUE_CYMBAL: Self = Self(0x0100);
    /// Green cymbal (Pro)
    pub const GREEN_CYMBAL: Self = Self(0x0200);
    /// Hi-hat pedal (Pro)
    pub const HIHAT_PEDAL: Self = Self(0x0400);

    pub fn empty() -> Self {
        Self(0)
    }

    pub fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}

/// Drum controller type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrumType {
    /// Standard 4-pad + kick (GH)
    FourPad,
    /// 4-pad + kick + hi-hat (RB)
    FourPadPro,
    /// Pro drums with cymbals (RB)
    ProDrums,
    /// ION Drum Rocker (premium set)
    IonDrumRocker,
}

/// Drum controller state
#[derive(Debug, Clone, Default)]
pub struct DrumState {
    /// Pads currently hit
    pub pads: DrumPads,
    /// Velocity for each pad (0-127)
    pub velocities: [u8; 12],
    /// Start button
    pub start: bool,
    /// Select button
    pub select: bool,
    /// PS/Guide button
    pub ps_button: bool,
}

impl DrumState {
    pub fn new() -> Self {
        Self {
            pads: DrumPads::empty(),
            velocities: [0; 12],
            start: false,
            select: false,
            ps_button: false,
        }
    }

    /// Hit a pad with velocity
    pub fn hit_pad(&mut self, pad: DrumPads, velocity: u8) {
        self.pads.insert(pad);
        
        // Map pad to velocity index
        let idx = match pad {
            DrumPads::RED => 0,
            DrumPads::YELLOW => 1,
            DrumPads::BLUE => 2,
            DrumPads::GREEN => 3,
            DrumPads::ORANGE => 4,
            DrumPads::KICK => 5,
            DrumPads::KICK2 => 6,
            DrumPads::YELLOW_CYMBAL => 7,
            DrumPads::BLUE_CYMBAL => 8,
            DrumPads::GREEN_CYMBAL => 9,
            DrumPads::HIHAT_PEDAL => 10,
            _ => return,
        };
        self.velocities[idx] = velocity;
    }

    /// Release a pad
    pub fn release_pad(&mut self, pad: DrumPads) {
        self.pads.remove(pad);
    }

    /// Get velocity for a pad
    pub fn get_velocity(&self, pad: DrumPads) -> u8 {
        let idx = match pad {
            DrumPads::RED => 0,
            DrumPads::YELLOW => 1,
            DrumPads::BLUE => 2,
            DrumPads::GREEN => 3,
            DrumPads::ORANGE => 4,
            DrumPads::KICK => 5,
            DrumPads::KICK2 => 6,
            DrumPads::YELLOW_CYMBAL => 7,
            DrumPads::BLUE_CYMBAL => 8,
            DrumPads::GREEN_CYMBAL => 9,
            DrumPads::HIHAT_PEDAL => 10,
            _ => return 0,
        };
        self.velocities[idx]
    }
}

/// Drum controller
#[derive(Debug)]
pub struct DrumController {
    /// Controller index
    pub index: u8,
    /// Drum type
    pub drum_type: DrumType,
    /// Current state
    pub state: DrumState,
    /// Connected
    pub connected: bool,
    /// Has velocity sensitivity
    pub has_velocity: bool,
    /// Last update
    last_update: Instant,
}

impl DrumController {
    pub fn new(index: u8, drum_type: DrumType) -> Self {
        let has_velocity = matches!(drum_type, DrumType::ProDrums | DrumType::IonDrumRocker);
        
        Self {
            index,
            drum_type,
            state: DrumState::new(),
            connected: false,
            has_velocity,
            last_update: Instant::now(),
        }
    }

    pub fn connect(&mut self) {
        self.connected = true;
        tracing::info!("Drums {} connected ({:?})", self.index, self.drum_type);
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
        self.state = DrumState::new();
        tracing::info!("Drums {} disconnected", self.index);
    }

    /// Hit a pad (with auto-release after short time)
    pub fn hit(&mut self, pad: DrumPads, velocity: u8) {
        let vel = if self.has_velocity { velocity } else { 127 };
        self.state.hit_pad(pad, vel);
    }

    /// Release a pad
    pub fn release(&mut self, pad: DrumPads) {
        self.state.release_pad(pad);
    }

    pub fn update(&mut self) {
        self.last_update = Instant::now();
    }
}

/// DJ Hero turntable state
#[derive(Debug, Clone, Default)]
pub struct TurntableState {
    /// Turntable rotation (-127 to 127, 0 = stopped)
    pub rotation: i8,
    /// Crossfader position (0-255, 128 = center)
    pub crossfader: u8,
    /// Effects dial (0-255)
    pub effects_dial: u8,
    /// Green button
    pub green: bool,
    /// Red button
    pub red: bool,
    /// Blue button
    pub blue: bool,
    /// Euphoria button
    pub euphoria: bool,
    /// Start button
    pub start: bool,
    /// Select button
    pub select: bool,
}

/// DJ Hero turntable controller
#[derive(Debug)]
pub struct TurntableController {
    /// Controller index
    pub index: u8,
    /// Current state
    pub state: TurntableState,
    /// Connected
    pub connected: bool,
    /// Last update
    last_update: Instant,
}

impl TurntableController {
    pub fn new(index: u8) -> Self {
        Self {
            index,
            state: TurntableState::default(),
            connected: false,
            last_update: Instant::now(),
        }
    }

    pub fn connect(&mut self) {
        self.connected = true;
        tracing::info!("Turntable {} connected", self.index);
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
        self.state = TurntableState::default();
        tracing::info!("Turntable {} disconnected", self.index);
    }

    pub fn set_rotation(&mut self, rotation: i8) {
        self.state.rotation = rotation;
    }

    pub fn set_crossfader(&mut self, position: u8) {
        self.state.crossfader = position;
    }

    pub fn update(&mut self) {
        self.last_update = Instant::now();
    }
}

/// Instrument type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstrumentType {
    Guitar(GuitarType),
    Drums(DrumType),
    Turntable,
    Microphone,
}

/// Music game instrument manager
pub struct InstrumentManager {
    /// Guitars
    guitars: [Option<GuitarController>; 4],
    /// Drums
    drums: [Option<DrumController>; 4],
    /// Turntables
    turntables: [Option<TurntableController>; 2],
}

impl InstrumentManager {
    pub fn new() -> Self {
        Self {
            guitars: Default::default(),
            drums: Default::default(),
            turntables: Default::default(),
        }
    }

    /// Connect a guitar
    pub fn connect_guitar(&mut self, index: u8, guitar_type: GuitarType) -> Result<(), &'static str> {
        if index >= 4 {
            return Err("Invalid guitar index");
        }
        let mut guitar = GuitarController::new(index, guitar_type);
        guitar.connect();
        self.guitars[index as usize] = Some(guitar);
        Ok(())
    }

    /// Connect drums
    pub fn connect_drums(&mut self, index: u8, drum_type: DrumType) -> Result<(), &'static str> {
        if index >= 4 {
            return Err("Invalid drum index");
        }
        let mut drums = DrumController::new(index, drum_type);
        drums.connect();
        self.drums[index as usize] = Some(drums);
        Ok(())
    }

    /// Connect turntable
    pub fn connect_turntable(&mut self, index: u8) -> Result<(), &'static str> {
        if index >= 2 {
            return Err("Invalid turntable index");
        }
        let mut turntable = TurntableController::new(index);
        turntable.connect();
        self.turntables[index as usize] = Some(turntable);
        Ok(())
    }

    /// Get guitar
    pub fn get_guitar(&self, index: u8) -> Option<&GuitarController> {
        self.guitars.get(index as usize)?.as_ref()
    }

    /// Get mutable guitar
    pub fn get_guitar_mut(&mut self, index: u8) -> Option<&mut GuitarController> {
        self.guitars.get_mut(index as usize)?.as_mut()
    }

    /// Get drums
    pub fn get_drums(&self, index: u8) -> Option<&DrumController> {
        self.drums.get(index as usize)?.as_ref()
    }

    /// Get mutable drums
    pub fn get_drums_mut(&mut self, index: u8) -> Option<&mut DrumController> {
        self.drums.get_mut(index as usize)?.as_mut()
    }

    /// Get turntable
    pub fn get_turntable(&self, index: u8) -> Option<&TurntableController> {
        self.turntables.get(index as usize)?.as_ref()
    }

    /// Get mutable turntable
    pub fn get_turntable_mut(&mut self, index: u8) -> Option<&mut TurntableController> {
        self.turntables.get_mut(index as usize)?.as_mut()
    }

    /// Update all instruments
    pub fn update(&mut self) {
        for guitar in self.guitars.iter_mut().flatten() {
            guitar.update();
        }
        for drums in self.drums.iter_mut().flatten() {
            drums.update();
        }
        for turntable in self.turntables.iter_mut().flatten() {
            turntable.update();
        }
    }

    /// Get total connected instruments
    pub fn connected_count(&self) -> usize {
        self.guitars.iter().filter(|g| g.is_some()).count() +
        self.drums.iter().filter(|d| d.is_some()).count() +
        self.turntables.iter().filter(|t| t.is_some()).count()
    }
}

impl Default for InstrumentManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guitar_frets() {
        let mut frets = GuitarFrets::empty();
        assert!(!frets.contains(GuitarFrets::GREEN));
        
        frets.insert(GuitarFrets::GREEN);
        frets.insert(GuitarFrets::RED);
        assert!(frets.contains(GuitarFrets::GREEN));
        assert_eq!(frets.count(), 2);
    }

    #[test]
    fn test_guitar_state() {
        let mut guitar = GuitarController::new(0, GuitarType::FiveFret);
        guitar.connect();
        
        guitar.set_fret(GuitarFrets::GREEN, true);
        guitar.set_button(GuitarButtons::STRUM_DOWN, true);
        
        assert!(guitar.state.frets.contains(GuitarFrets::GREEN));
        assert!(guitar.state.is_strumming());
    }

    #[test]
    fn test_drum_velocity() {
        let mut drums = DrumController::new(0, DrumType::ProDrums);
        drums.connect();
        
        drums.hit(DrumPads::RED, 100);
        assert!(drums.state.pads.contains(DrumPads::RED));
        assert_eq!(drums.state.get_velocity(DrumPads::RED), 100);
    }

    #[test]
    fn test_manager() {
        let mut manager = InstrumentManager::new();
        
        assert!(manager.connect_guitar(0, GuitarType::FiveFret).is_ok());
        assert!(manager.connect_drums(0, DrumType::FourPad).is_ok());
        
        assert_eq!(manager.connected_count(), 2);
    }
}
