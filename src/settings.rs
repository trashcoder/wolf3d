use std::sync::{
    RwLock,
    atomic::{AtomicBool, AtomicU8, Ordering},
};

static MUSIC_ENABLED: AtomicBool = AtomicBool::new(true);
static EFFECTS_ENABLED: AtomicBool = AtomicBool::new(true);
static DIGITIZED_EFFECTS_ENABLED: AtomicBool = AtomicBool::new(true);
static ADLIB_EFFECTS_ENABLED: AtomicBool = AtomicBool::new(false);
static FULLSCREEN_ENABLED: AtomicBool = AtomicBool::new(false);
static WINDOW_SCALE: AtomicU8 = AtomicU8::new(DEFAULT_WINDOW_SCALE);
static VIEW_SIZE: AtomicU8 = AtomicU8::new(DEFAULT_VIEW_SIZE);
static MOUSE_SENSITIVITY: AtomicU8 = AtomicU8::new(DEFAULT_MOUSE_SENSITIVITY);
static MOUSE_INPUT_ENABLED: AtomicBool = AtomicBool::new(true);
static CONTROLLER_INPUT_ENABLED: AtomicBool = AtomicBool::new(true);
static SHOW_HISTORICAL_SYMBOLS: AtomicBool = AtomicBool::new(false);
static WORLD_RENDERER: AtomicU8 = AtomicU8::new(WorldRenderer::Original as u8);
static RENDER_RESOLUTION: AtomicU8 = AtomicU8::new(RenderResolution::Original as u8);
static SELECTED_AUDIO_DEVICE: RwLock<Option<String>> = RwLock::new(None);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[repr(u8)]
#[serde(rename_all = "snake_case")]
pub enum WorldRenderer {
    #[default]
    Original = 0,
    Supersampling2x = 1,
    Supersampling4x = 2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[repr(u8)]
#[serde(rename_all = "snake_case")]
pub enum RenderResolution {
    #[default]
    Original = 0,
    Scale2x = 1,
    Scale3x = 2,
    Scale4x = 3,
}

impl RenderResolution {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Scale2x,
            2 => Self::Scale3x,
            3 => Self::Scale4x,
            _ => Self::Original,
        }
    }

    pub fn next(self) -> Self {
        Self::from_u8((self as u8 + 1) % 4)
    }

    pub fn scale(self) -> usize {
        match self {
            Self::Original => 1,
            Self::Scale2x => 2,
            Self::Scale3x => 3,
            Self::Scale4x => 4,
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::Original => "320X200",
            Self::Scale2x => "640X400",
            Self::Scale3x => "960X600",
            Self::Scale4x => "1280X800",
        }
    }
}

impl WorldRenderer {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Supersampling2x,
            2 => Self::Supersampling4x,
            _ => Self::Original,
        }
    }

    pub fn next(self) -> Self {
        Self::from_u8((self as u8 + 1) % 3)
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::Original => "ORIGINAL",
            Self::Supersampling2x => "2X",
            Self::Supersampling4x => "4X",
        }
    }

    pub(crate) fn sample_count(self) -> usize {
        match self {
            Self::Original => 1,
            Self::Supersampling2x => 2,
            Self::Supersampling4x => 4,
        }
    }
}

pub const MIN_WINDOW_SCALE: u8 = 2;
pub const DEFAULT_WINDOW_SCALE: u8 = 3;
pub const MAX_WINDOW_SCALE: u8 = 4;
pub const MIN_VIEW_SIZE: u8 = 4;
pub const DEFAULT_VIEW_SIZE: u8 = 15;
pub const MAX_VIEW_SIZE: u8 = 19;
pub const MIN_MOUSE_SENSITIVITY: u8 = 0;
pub const DEFAULT_MOUSE_SENSITIVITY: u8 = 5;
pub const MAX_MOUSE_SENSITIVITY: u8 = 9;

pub const CONTROL_COUNT: usize = 12;
pub const MOVE_DIRECTION_COUNT: usize = 4;
pub const BUTTON_ACTION_RUN: u8 = 4;
pub const BUTTON_ACTION_STRAFE: u8 = 5;
pub const BUTTON_ACTION_USE: u8 = 6;
pub const BUTTON_ACTION_FIRE: u8 = 7;

/// Maps the four DOS-style physical button slots to gameplay action indices.
/// `u8::MAX` leaves a physical button unassigned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ButtonBindings {
    pub mouse: [u8; 4],
    pub controller: [u8; 4],
}

pub const DEFAULT_BUTTON_BINDINGS: ButtonBindings = ButtonBindings {
    mouse: [
        BUTTON_ACTION_FIRE,
        BUTTON_ACTION_STRAFE,
        BUTTON_ACTION_USE,
        u8::MAX,
    ],
    controller: [
        BUTTON_ACTION_FIRE,
        BUTTON_ACTION_STRAFE,
        BUTTON_ACTION_USE,
        BUTTON_ACTION_RUN,
    ],
};

static BUTTON_BINDINGS: RwLock<ButtonBindings> = RwLock::new(DEFAULT_BUTTON_BINDINGS);

/// SDL scancodes for DOS `dirscan`, in north/east/south/west order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct KeyboardMoveBindings {
    pub scancodes: [u16; MOVE_DIRECTION_COUNT],
}

pub const DEFAULT_KEYBOARD_MOVE_BINDINGS: KeyboardMoveBindings = KeyboardMoveBindings {
    scancodes: [82, 79, 81, 80], // Up / Right / Down / Left
};

static KEYBOARD_MOVE_BINDINGS: RwLock<KeyboardMoveBindings> =
    RwLock::new(DEFAULT_KEYBOARD_MOVE_BINDINGS);

/// SDL scancodes for the primary and alternate key assigned to each action.
/// Actions are forward, backward, turn left, turn right, run, strafe modifier,
/// use, fire, then weapon slots 1 through 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct KeyBindings {
    pub scancodes: [[u16; 2]; CONTROL_COUNT],
}

const DEFAULT_CONTROLS: KeyBindings = KeyBindings {
    scancodes: [
        [26, 0],    // W
        [22, 0],    // S
        [4, 0],     // A
        [7, 0],     // D
        [225, 229], // Left Shift / Right Shift
        [226, 230], // Left Alt / Right Alt
        [8, 0],     // E
        [224, 228], // Left Ctrl / Right Ctrl
        [30, 0],    // 1
        [31, 0],    // 2
        [32, 0],    // 3
        [33, 0],    // 4
    ],
};

const LEGACY_DEFAULT_CONTROLS: KeyBindings = KeyBindings {
    scancodes: [
        [26, 82],   // W / Up
        [22, 81],   // S / Down
        [4, 80],    // A / Left
        [7, 79],    // D / Right
        [225, 229], // Left Shift / Right Shift
        [226, 230], // Left Alt / Right Alt
        [8, 0],     // E
        [224, 228], // Left Ctrl / Right Ctrl
        [30, 0],    // 1
        [31, 0],    // 2
        [32, 0],    // 3
        [33, 0],    // 4
    ],
};

static KEY_BINDINGS: RwLock<KeyBindings> = RwLock::new(DEFAULT_CONTROLS);

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct AppSettings {
    #[serde(default = "enabled_by_default")]
    music_enabled: bool,
    #[serde(default = "enabled_by_default")]
    effects_enabled: bool,
    #[serde(default = "enabled_by_default")]
    digitized_effects_enabled: bool,
    #[serde(default)]
    adlib_effects_enabled: bool,
    #[serde(default)]
    fullscreen_enabled: bool,
    #[serde(default = "default_controls")]
    key_bindings: KeyBindings,
    #[serde(default = "default_button_bindings")]
    button_bindings: ButtonBindings,
    #[serde(default = "default_keyboard_move_bindings")]
    keyboard_move_bindings: KeyboardMoveBindings,
    #[serde(default)]
    audio_device: Option<String>,
    #[serde(default = "default_window_scale")]
    window_scale: u8,
    #[serde(default = "default_view_size")]
    view_size: u8,
    #[serde(default = "default_mouse_sensitivity")]
    mouse_sensitivity: u8,
    #[serde(default = "enabled_by_default")]
    mouse_input_enabled: bool,
    #[serde(default = "enabled_by_default")]
    controller_input_enabled: bool,
    #[serde(default)]
    show_historical_symbols: bool,
    #[serde(default)]
    high_resolution_renderer: bool,
    #[serde(default)]
    world_renderer: WorldRenderer,
    #[serde(default)]
    render_resolution: RenderResolution,
}

fn default_controls() -> KeyBindings {
    DEFAULT_CONTROLS
}

fn default_button_bindings() -> ButtonBindings {
    DEFAULT_BUTTON_BINDINGS
}

fn default_keyboard_move_bindings() -> KeyboardMoveBindings {
    DEFAULT_KEYBOARD_MOVE_BINDINGS
}

fn sanitize_button_bindings(mut bindings: ButtonBindings) -> ButtonBindings {
    let mut used_mouse = [false; CONTROL_COUNT];
    for (index, action) in bindings.mouse.iter_mut().enumerate() {
        if ![BUTTON_ACTION_STRAFE, BUTTON_ACTION_USE, BUTTON_ACTION_FIRE].contains(action)
            || index >= 3
            || used_mouse[*action as usize]
        {
            *action = u8::MAX;
        } else {
            used_mouse[*action as usize] = true;
        }
    }
    let mut used_controller = [false; CONTROL_COUNT];
    for action in &mut bindings.controller {
        if ![
            BUTTON_ACTION_RUN,
            BUTTON_ACTION_STRAFE,
            BUTTON_ACTION_USE,
            BUTTON_ACTION_FIRE,
        ]
        .contains(action)
            || used_controller[*action as usize]
        {
            *action = u8::MAX;
        } else {
            used_controller[*action as usize] = true;
        }
    }
    bindings
}

fn enabled_by_default() -> bool {
    true
}

fn default_window_scale() -> u8 {
    DEFAULT_WINDOW_SCALE
}

fn default_view_size() -> u8 {
    DEFAULT_VIEW_SIZE
}

fn default_mouse_sensitivity() -> u8 {
    DEFAULT_MOUSE_SENSITIVITY
}

pub fn load() -> Result<(), Box<dyn std::error::Error>> {
    let path = crate::resources::settings_path();
    let legacy_path = crate::resources::legacy_settings_path();
    let load_path = if path.exists() {
        path
    } else if legacy_path != path && legacy_path.exists() {
        legacy_path
    } else {
        return Ok(());
    };
    let settings = crate::savegame::load::<AppSettings>(&load_path)?;
    MUSIC_ENABLED.store(settings.music_enabled, Ordering::Relaxed);
    EFFECTS_ENABLED.store(settings.effects_enabled, Ordering::Relaxed);
    DIGITIZED_EFFECTS_ENABLED.store(settings.digitized_effects_enabled, Ordering::Relaxed);
    ADLIB_EFFECTS_ENABLED.store(settings.adlib_effects_enabled, Ordering::Relaxed);
    FULLSCREEN_ENABLED.store(settings.fullscreen_enabled, Ordering::Relaxed);
    *BUTTON_BINDINGS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) =
        sanitize_button_bindings(settings.button_bindings);
    *KEYBOARD_MOVE_BINDINGS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = settings.keyboard_move_bindings;
    WINDOW_SCALE.store(
        if (MIN_WINDOW_SCALE..=MAX_WINDOW_SCALE).contains(&settings.window_scale) {
            settings.window_scale
        } else {
            DEFAULT_WINDOW_SCALE
        },
        Ordering::Relaxed,
    );
    VIEW_SIZE.store(
        if (MIN_VIEW_SIZE..=MAX_VIEW_SIZE).contains(&settings.view_size) {
            settings.view_size
        } else {
            DEFAULT_VIEW_SIZE
        },
        Ordering::Relaxed,
    );
    MOUSE_SENSITIVITY.store(
        if (MIN_MOUSE_SENSITIVITY..=MAX_MOUSE_SENSITIVITY).contains(&settings.mouse_sensitivity) {
            settings.mouse_sensitivity
        } else {
            DEFAULT_MOUSE_SENSITIVITY
        },
        Ordering::Relaxed,
    );
    MOUSE_INPUT_ENABLED.store(settings.mouse_input_enabled, Ordering::Relaxed);
    CONTROLLER_INPUT_ENABLED.store(settings.controller_input_enabled, Ordering::Relaxed);
    SHOW_HISTORICAL_SYMBOLS.store(settings.show_historical_symbols, Ordering::Relaxed);
    let world_renderer = if settings.high_resolution_renderer
        && settings.world_renderer == WorldRenderer::Original
    {
        WorldRenderer::Supersampling2x
    } else {
        settings.world_renderer
    };
    WORLD_RENDERER.store(world_renderer as u8, Ordering::Relaxed);
    RENDER_RESOLUTION.store(settings.render_resolution as u8, Ordering::Relaxed);
    *KEY_BINDINGS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) =
        if settings.key_bindings == LEGACY_DEFAULT_CONTROLS {
            DEFAULT_CONTROLS
        } else {
            settings.key_bindings
        };
    *SELECTED_AUDIO_DEVICE
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = settings.audio_device;
    Ok(())
}

pub fn music_enabled() -> bool {
    MUSIC_ENABLED.load(Ordering::Relaxed)
}

pub fn effects_enabled() -> bool {
    EFFECTS_ENABLED.load(Ordering::Relaxed)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundEffectsMode {
    Off,
    Pc,
    Adlib,
}

pub fn sound_effects_mode() -> SoundEffectsMode {
    if !effects_enabled() {
        SoundEffectsMode::Off
    } else if adlib_effects_enabled() {
        SoundEffectsMode::Adlib
    } else {
        SoundEffectsMode::Pc
    }
}

pub fn set_sound_effects_mode(mode: SoundEffectsMode) -> Result<(), Box<dyn std::error::Error>> {
    EFFECTS_ENABLED.store(mode != SoundEffectsMode::Off, Ordering::Relaxed);
    ADLIB_EFFECTS_ENABLED.store(mode == SoundEffectsMode::Adlib, Ordering::Relaxed);
    save()
}

pub fn show_historical_symbols() -> bool {
    SHOW_HISTORICAL_SYMBOLS.load(Ordering::Relaxed)
}

pub fn set_show_historical_symbols(show: bool) -> Result<(), Box<dyn std::error::Error>> {
    SHOW_HISTORICAL_SYMBOLS.store(show, Ordering::Relaxed);
    save()
}

/// Whether archived digitized samples should replace the PC-speaker fallback.
pub fn digitized_effects_enabled() -> bool {
    DIGITIZED_EFFECTS_ENABLED.load(Ordering::Relaxed)
}

pub fn set_digitized_effects_enabled(enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
    DIGITIZED_EFFECTS_ENABLED.store(enabled, Ordering::Relaxed);
    save()
}

/// Select AdLib synthesis instead of PC-speaker tones for non-digitized effects.
pub fn adlib_effects_enabled() -> bool {
    ADLIB_EFFECTS_ENABLED.load(Ordering::Relaxed)
}

pub fn fullscreen_enabled() -> bool {
    FULLSCREEN_ENABLED.load(Ordering::Relaxed)
}

pub fn window_scale() -> u8 {
    WINDOW_SCALE.load(Ordering::Relaxed)
}

pub fn world_renderer() -> WorldRenderer {
    WorldRenderer::from_u8(WORLD_RENDERER.load(Ordering::Relaxed))
}

pub fn set_world_renderer(renderer: WorldRenderer) -> Result<(), Box<dyn std::error::Error>> {
    WORLD_RENDERER.store(renderer as u8, Ordering::Relaxed);
    save()
}

pub fn render_resolution() -> RenderResolution {
    RenderResolution::from_u8(RENDER_RESOLUTION.load(Ordering::Relaxed))
}

pub fn set_render_resolution(
    resolution: RenderResolution,
) -> Result<(), Box<dyn std::error::Error>> {
    RENDER_RESOLUTION.store(resolution as u8, Ordering::Relaxed);
    save()
}

pub fn view_size() -> u8 {
    VIEW_SIZE.load(Ordering::Relaxed)
}

pub fn mouse_sensitivity() -> u8 {
    MOUSE_SENSITIVITY.load(Ordering::Relaxed)
}

pub fn mouse_input_enabled() -> bool {
    MOUSE_INPUT_ENABLED.load(Ordering::Relaxed)
}

pub fn controller_input_enabled() -> bool {
    CONTROLLER_INPUT_ENABLED.load(Ordering::Relaxed)
}

pub fn set_mouse_input_enabled(enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
    MOUSE_INPUT_ENABLED.store(enabled, Ordering::Relaxed);
    save()
}

pub fn set_controller_input_enabled(enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
    CONTROLLER_INPUT_ENABLED.store(enabled, Ordering::Relaxed);
    save()
}

pub fn set_mouse_sensitivity(sensitivity: u8) -> Result<(), Box<dyn std::error::Error>> {
    if !(MIN_MOUSE_SENSITIVITY..=MAX_MOUSE_SENSITIVITY).contains(&sensitivity) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "mouse sensitivity must be between {MIN_MOUSE_SENSITIVITY} and {MAX_MOUSE_SENSITIVITY}"
            ),
        )
        .into());
    }
    MOUSE_SENSITIVITY.store(sensitivity, Ordering::Relaxed);
    save()
}

pub fn set_view_size(size: u8) -> Result<(), Box<dyn std::error::Error>> {
    preview_view_size(size)?;
    save()
}

pub(crate) fn preview_view_size(size: u8) -> Result<(), Box<dyn std::error::Error>> {
    if !(MIN_VIEW_SIZE..=MAX_VIEW_SIZE).contains(&size) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("view size must be between {MIN_VIEW_SIZE} and {MAX_VIEW_SIZE}"),
        )
        .into());
    }
    VIEW_SIZE.store(size, Ordering::Relaxed);
    Ok(())
}

pub(crate) fn save_view_size() -> Result<(), Box<dyn std::error::Error>> {
    save()
}

pub fn set_window_scale(scale: u8) -> Result<(), Box<dyn std::error::Error>> {
    if !(MIN_WINDOW_SCALE..=MAX_WINDOW_SCALE).contains(&scale) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("window scale must be {MIN_WINDOW_SCALE}, {DEFAULT_WINDOW_SCALE}, or {MAX_WINDOW_SCALE}"),
        )
        .into());
    }
    WINDOW_SCALE.store(scale, Ordering::Relaxed);
    save()
}

/// The persisted SDL output-device name, or `None` for SDL's system default.
pub fn audio_device() -> Option<String> {
    SELECTED_AUDIO_DEVICE
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// Select a named SDL audio output device, or `None` for the system default.
pub fn set_audio_device(device: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    if device
        .as_deref()
        .is_some_and(|name| name.is_empty() || name.contains('\0'))
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "audio device name must be non-empty and cannot contain NUL",
        )
        .into());
    }
    *SELECTED_AUDIO_DEVICE
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = device;
    save()
}

pub fn get_controls() -> KeyBindings {
    *KEY_BINDINGS
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn get_keyboard_move_bindings() -> KeyboardMoveBindings {
    *KEYBOARD_MOVE_BINDINGS
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn set_keyboard_move_key(
    direction: usize,
    scancode: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    if direction >= MOVE_DIRECTION_COUNT {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("movement direction {direction} is outside 0..{MOVE_DIRECTION_COUNT}"),
        )
        .into());
    }
    if scancode == 0 || scancode >= 512 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("SDL scancode {scancode} is not a bindable key"),
        )
        .into());
    }
    if (58..=67).contains(&scancode) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "F1-F10 are reserved for game controls and menu shortcuts",
        )
        .into());
    }
    KEYBOARD_MOVE_BINDINGS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .scancodes[direction] = scancode;
    save()
}

pub fn button_bindings() -> ButtonBindings {
    *BUTTON_BINDINGS
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn set_mouse_button_action(
    action: u8,
    physical_button: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    set_button_action(action, physical_button, true)
}

pub fn set_controller_button_action(
    action: u8,
    physical_button: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    set_button_action(action, physical_button, false)
}

fn set_button_action(
    action: u8,
    physical_button: usize,
    mouse: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let valid_action = if mouse {
        [BUTTON_ACTION_STRAFE, BUTTON_ACTION_USE, BUTTON_ACTION_FIRE].contains(&action)
    } else {
        [
            BUTTON_ACTION_RUN,
            BUTTON_ACTION_STRAFE,
            BUTTON_ACTION_USE,
            BUTTON_ACTION_FIRE,
        ]
        .contains(&action)
    };
    if physical_button >= if mouse { 3 } else { 4 } || !valid_action {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid physical button or gameplay action",
        )
        .into());
    }
    let mut bindings = BUTTON_BINDINGS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let actions = if mouse {
        &mut bindings.mouse
    } else {
        &mut bindings.controller
    };
    for binding in actions.iter_mut() {
        if *binding == action {
            *binding = u8::MAX;
        }
    }
    actions[physical_button] = action;
    drop(bindings);
    save()
}

pub fn set_control(index: usize, scancode: u16) -> Result<(), Box<dyn std::error::Error>> {
    if index >= CONTROL_COUNT {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("control index {index} is outside 0..{CONTROL_COUNT}"),
        )
        .into());
    }
    if scancode == 0 || scancode >= 512 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("SDL scancode {scancode} is not a bindable key"),
        )
        .into());
    }
    if (62..=66).contains(&scancode) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "F5-F9 are reserved for save, load, and save-slot shortcuts",
        )
        .into());
    }
    KEY_BINDINGS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .scancodes[index] = [scancode, 0];
    save()
}

pub fn reset_controls() -> Result<(), Box<dyn std::error::Error>> {
    *KEY_BINDINGS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = DEFAULT_CONTROLS;
    *BUTTON_BINDINGS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = DEFAULT_BUTTON_BINDINGS;
    *KEYBOARD_MOVE_BINDINGS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = DEFAULT_KEYBOARD_MOVE_BINDINGS;
    save()
}

pub fn toggle_music() -> Result<(), Box<dyn std::error::Error>> {
    set_music_enabled(!music_enabled())
}

pub fn set_music_enabled(enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
    MUSIC_ENABLED.store(enabled, Ordering::Relaxed);
    save()
}

pub fn toggle_fullscreen() -> Result<(), Box<dyn std::error::Error>> {
    set_fullscreen(!fullscreen_enabled())
}

pub fn set_fullscreen(enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
    FULLSCREEN_ENABLED.store(enabled, Ordering::Relaxed);
    save()
}

fn save() -> Result<(), Box<dyn std::error::Error>> {
    let settings = AppSettings {
        music_enabled: music_enabled(),
        effects_enabled: effects_enabled(),
        digitized_effects_enabled: digitized_effects_enabled(),
        adlib_effects_enabled: adlib_effects_enabled(),
        fullscreen_enabled: fullscreen_enabled(),
        key_bindings: get_controls(),
        button_bindings: button_bindings(),
        keyboard_move_bindings: get_keyboard_move_bindings(),
        audio_device: audio_device(),
        window_scale: window_scale(),
        view_size: view_size(),
        mouse_sensitivity: mouse_sensitivity(),
        mouse_input_enabled: mouse_input_enabled(),
        controller_input_enabled: controller_input_enabled(),
        show_historical_symbols: show_historical_symbols(),
        high_resolution_renderer: world_renderer() != WorldRenderer::Original,
        world_renderer: world_renderer(),
        render_resolution: render_resolution(),
    };
    crate::savegame::save(&crate::resources::settings_path(), &settings)
}
