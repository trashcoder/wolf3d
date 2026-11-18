use std::{
    borrow::Cow,
    cell::RefCell,
    cell::UnsafeCell,
    collections::VecDeque,
    error::Error,
    ffi::CString,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::{
    actors::{ActorKind, ActorSpawn, Difficulty, EnemyRuntime, ItemPickup, PickupKind, SpawnMode},
    audio::{AudioArchive, OUTPUT_RATE},
    doors::DoorSystem,
    ending,
    map::Map,
    pushwalls::PushWallSystem,
    resources,
    sprites::SpriteAtlas,
    vga::BitmapFont,
    vga::Picture,
    vswap::Vswap,
    weapons::{Arsenal, Weapon, WeaponAttack},
    world::{self, HEIGHT, Player, WIDTH},
};

const QUIT_CONFIRMATION_MESSAGES: [&str; 9] = [
    "Dost thou wish to\nleave with such hasty\nabandon?",
    "Chickening out...\nalready?",
    "Press N for more carnage.\nPress Y to be a weenie.",
    "So, you think you can\nquit this easily, huh?",
    "Press N to save the world.\nPress Y to abandon it in\nits hour of need.",
    "Press N if you are brave.\nPress Y to cower in shame.",
    "Heroes, press N.\nWimps, press Y.",
    "You are at an intersection.\nA sign says, 'Press Y to quit.'\n>",
    "For guns and glory, press N.\nFor work and worry, press Y.",
];

const PAUSE_CONTROL_LABELS: [&str; crate::settings::CONTROL_COUNT] = [
    "FORWARD",
    "BACKWARD",
    "TURN LEFT",
    "TURN RIGHT",
    "RUN",
    "STRAFE MODIFIER",
    "USE",
    "FIRE",
    "SELECT KNIFE",
    "SELECT PISTOL",
    "SELECT MACHINE GUN",
    "SELECT CHAINGUN",
];
const PAUSE_MOVE_LABELS: [&str; crate::settings::MOVE_DIRECTION_COUNT] =
    ["MOVE NORTH", "MOVE EAST", "MOVE SOUTH", "MOVE WEST"];
const MOUSE_BUTTON_ACTIONS: [u8; 3] = [
    crate::settings::BUTTON_ACTION_FIRE,
    crate::settings::BUTTON_ACTION_STRAFE,
    crate::settings::BUTTON_ACTION_USE,
];
const CONTROLLER_BUTTON_ACTIONS: [u8; 4] = [
    crate::settings::BUTTON_ACTION_FIRE,
    crate::settings::BUTTON_ACTION_STRAFE,
    crate::settings::BUTTON_ACTION_USE,
    crate::settings::BUTTON_ACTION_RUN,
];
const PAUSE_CONTROL_SETTINGS_COUNT: usize = 8;

pub fn window_size() -> (i32, i32) {
    let width = (WIDTH as i32) * i32::from(crate::settings::window_scale());
    (width, width * 3 / 4)
}

fn window_width() -> i32 {
    window_size().0
}

fn window_height() -> i32 {
    window_size().1
}

#[cfg_attr(target_os = "windows", link(name = "SDL2"))]
#[cfg_attr(target_os = "linux", link(name = "SDL2"))]
unsafe extern "C" {
    #[link_name = "SDL_Init"]
    fn SDL_InitNative(flags: u32) -> i32;
    fn SDL_SetMainReady();
    fn SDL_InitSubSystem(flags: u32) -> i32;
    fn SDL_QuitSubSystem(flags: u32);
    #[link_name = "SDL_Quit"]
    fn SDL_QuitNative();
    fn SDL_WasInit(flags: u32) -> u32;
    fn SDL_GetError() -> *const i8;
    fn SDL_GetScancodeName(scancode: i32) -> *const i8;
    #[link_name = "SDL_CreateWindow"]
    fn SDL_CreateWindowNative(
        title: *const i8,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        flags: u32,
    ) -> *mut SdlWindow;
    fn SDL_GetWindowFlags(window: *mut SdlWindow) -> u32;
    fn SDL_SetWindowFullscreen(window: *mut SdlWindow, flags: u32) -> i32;
    fn SDL_SetWindowSize(window: *mut SdlWindow, width: i32, height: i32);
    fn SDL_GetWindowSize(window: *mut SdlWindow, width: *mut i32, height: *mut i32);
    fn SDL_SetWindowTitle(window: *mut SdlWindow, title: *const i8);
    #[link_name = "SDL_DestroyWindow"]
    fn SDL_DestroyWindowNative(window: *mut SdlWindow);
    #[link_name = "SDL_CreateRenderer"]
    fn SDL_CreateRendererNative(window: *mut SdlWindow, index: i32, flags: u32)
    -> *mut SdlRenderer;
    #[link_name = "SDL_DestroyRenderer"]
    fn SDL_DestroyRendererNative(renderer: *mut SdlRenderer);
    #[link_name = "SDL_CreateTexture"]
    fn SDL_CreateTextureNative(
        renderer: *mut SdlRenderer,
        format: u32,
        access: i32,
        w: i32,
        h: i32,
    ) -> *mut SdlTexture;
    #[link_name = "SDL_DestroyTexture"]
    fn SDL_DestroyTextureNative(texture: *mut SdlTexture);
    fn SDL_SetTextureScaleMode(texture: *mut SdlTexture, scale_mode: i32) -> i32;
    #[link_name = "SDL_UpdateTexture"]
    fn SDL_UpdateTextureNative(
        texture: *mut SdlTexture,
        rect: *const (),
        pixels: *const u32,
        pitch: i32,
    ) -> i32;
    fn SDL_RenderClear(renderer: *mut SdlRenderer) -> i32;
    fn SDL_GetRendererOutputSize(
        renderer: *mut SdlRenderer,
        width: *mut i32,
        height: *mut i32,
    ) -> i32;
    fn SDL_RenderCopy(
        renderer: *mut SdlRenderer,
        texture: *mut SdlTexture,
        src: *const (),
        dst: *const SdlRect,
    ) -> i32;
    fn SDL_RenderPresent(renderer: *mut SdlRenderer);
    fn SDL_PollEvent(event: *mut SdlEvent) -> i32;
    fn SDL_StartTextInput();
    fn SDL_StopTextInput();
    fn SDL_GetKeyboardState(count: *mut i32) -> *const u8;
    fn SDL_SetRelativeMouseMode(enabled: i32) -> i32;
    fn SDL_Delay(ms: u32);
    fn SDL_OpenAudioDevice(
        device: *const i8,
        is_capture: i32,
        desired: *const SdlAudioSpec,
        obtained: *mut SdlAudioSpec,
        allowed_changes: i32,
    ) -> u32;
    fn SDL_GetNumAudioDevices(is_capture: i32) -> i32;
    fn SDL_GetAudioDeviceName(index: i32, is_capture: i32) -> *const i8;
    fn SDL_PauseAudioDevice(device: u32, pause_on: i32);
    fn SDL_LockAudioDevice(device: u32);
    fn SDL_UnlockAudioDevice(device: u32);
    fn SDL_CloseAudioDevice(device: u32);
    fn SDL_NumJoysticks() -> i32;
    fn SDL_IsGameController(joystick_index: i32) -> i32;
    fn SDL_GameControllerOpen(joystick_index: i32) -> *mut SdlGameController;
    fn SDL_GameControllerClose(controller: *mut SdlGameController);
    fn SDL_GameControllerGetAttached(controller: *mut SdlGameController) -> i32;
    fn SDL_GameControllerGetJoystick(controller: *mut SdlGameController) -> *mut SdlJoystick;
    fn SDL_GameControllerGetAxis(controller: *mut SdlGameController, axis: i32) -> i16;
    fn SDL_GameControllerGetButton(controller: *mut SdlGameController, button: i32) -> u8;
    fn SDL_JoystickInstanceID(joystick: *mut SdlJoystick) -> i32;
    fn SDL_PushEvent(event: *mut SdlEvent) -> i32;
}

fn initialize_sdl(flags: u32) -> i32 {
    // This Rust binary owns its platform entry point and does not use SDL_main.
    // SDL requires readiness to be declared before SDL_Init on Windows and macOS.
    unsafe {
        SDL_SetMainReady();
        initialize_subsystem_with(SDL_InitNative, flags)
    }
}

fn initialize_subsystem(flags: u32) -> i32 {
    unsafe { initialize_subsystem_with(SDL_InitSubSystem, flags) }
}

unsafe fn initialize_subsystem_with(
    initialize: unsafe extern "C" fn(u32) -> i32,
    flags: u32,
) -> i32 {
    let missing = flags & !unsafe { SDL_WasInit(flags) };
    if missing == 0 {
        0
    } else {
        unsafe { initialize(missing) }
    }
}

#[repr(C)]
struct SdlWindow {
    _private: [u8; 0],
}
#[repr(C)]
struct SdlRenderer {
    _private: [u8; 0],
}
#[repr(C)]
struct SdlTexture {
    _private: [u8; 0],
}

struct SharedVideo {
    window: *mut SdlWindow,
    renderer: *mut SdlRenderer,
    texture: *mut SdlTexture,
    high_resolution_texture: *mut SdlTexture,
}

// All screens render through this application-lifetime SDL video context.
thread_local! {
    static SHARED_VIDEO: RefCell<SharedVideo> = const { RefCell::new(SharedVideo {
        window: std::ptr::null_mut(),
        renderer: std::ptr::null_mut(),
        texture: std::ptr::null_mut(),
        high_resolution_texture: std::ptr::null_mut(),
    }) };
}

unsafe fn SDL_CreateWindow(
    title: *const i8,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    flags: u32,
) -> *mut SdlWindow {
    SHARED_VIDEO.with(|shared| {
        let mut shared = shared.borrow_mut();
        if shared.window.is_null() {
            shared.window = unsafe { SDL_CreateWindowNative(title, x, y, width, height, flags) };
        } else {
            unsafe {
                SDL_SetWindowTitle(shared.window, title);
                let fullscreen = flags & SDL_WINDOW_FULLSCREEN_DESKTOP;
                let current_flags = SDL_GetWindowFlags(shared.window);
                let is_fullscreen = current_flags & SDL_WINDOW_FULLSCREEN_DESKTOP != 0;
                if is_fullscreen != (fullscreen != 0) {
                    let _ = SDL_SetWindowFullscreen(shared.window, fullscreen);
                }
            }
        }
        shared.window
    })
}

// Screen functions retain their old local structure, but these shared
// resources are released together by shutdown_sdl after the application loop.
unsafe fn SDL_DestroyWindow(_window: *mut SdlWindow) {}

unsafe fn SDL_CreateRenderer(window: *mut SdlWindow, index: i32, flags: u32) -> *mut SdlRenderer {
    SHARED_VIDEO.with(|shared| {
        let mut shared = shared.borrow_mut();
        if shared.renderer.is_null() {
            shared.renderer = unsafe { SDL_CreateRendererNative(window, index, flags) };
        }
        shared.renderer
    })
}

unsafe fn SDL_DestroyRenderer(_renderer: *mut SdlRenderer) {}

unsafe fn SDL_CreateTexture(
    renderer: *mut SdlRenderer,
    format: u32,
    access: i32,
    width: i32,
    height: i32,
) -> *mut SdlTexture {
    SHARED_VIDEO.with(|shared| {
        let mut shared = shared.borrow_mut();
        if shared.texture.is_null() {
            shared.texture = unsafe {
                SDL_CreateTextureNative(renderer, format, access, width, height)
            };
        }
        if !shared.texture.is_null() {
            unsafe { SDL_SetTextureScaleMode(shared.texture, 0) };
        }
        shared.texture
    })
}

unsafe fn high_resolution_texture(renderer: *mut SdlRenderer) -> *mut SdlTexture {
    SHARED_VIDEO.with(|shared| {
        let mut shared = shared.borrow_mut();
        if shared.high_resolution_texture.is_null() {
            shared.high_resolution_texture = unsafe {
                SDL_CreateTextureNative(
                    renderer,
                    SDL_PIXELFORMAT_ARGB8888,
                    SDL_TEXTUREACCESS_STREAMING,
                    (crate::world::WIDTH * 4) as i32,
                    (crate::world::HEIGHT * 4) as i32,
                )
            };
            if !shared.high_resolution_texture.is_null() {
                unsafe { SDL_SetTextureScaleMode(shared.high_resolution_texture, 0) };
            }
        }
        shared.high_resolution_texture
    })
}

unsafe fn SDL_DestroyTexture(_texture: *mut SdlTexture) {}

unsafe fn SDL_UpdateTexture(
    texture: *mut SdlTexture,
    rect: *const (),
    pixels: *const u32,
    _pitch: i32,
) -> i32 {
    if !rect.is_null() || pixels.is_null() {
        return -1;
    }
    let source =
        unsafe { std::slice::from_raw_parts(pixels, crate::world::WIDTH * crate::world::HEIGHT) };
    unsafe {
        SDL_UpdateTextureNative(
            texture,
            std::ptr::null(),
            source.as_ptr(),
            (crate::world::WIDTH * std::mem::size_of::<u32>()) as i32,
        )
    }
}

pub fn shutdown_sdl() {
    SHARED_VIDEO.with(|shared| {
        let mut shared = shared.borrow_mut();
        unsafe {
            if !shared.texture.is_null() {
                SDL_DestroyTextureNative(shared.texture);
            }
            if !shared.high_resolution_texture.is_null() {
                SDL_DestroyTextureNative(shared.high_resolution_texture);
            }
            if !shared.renderer.is_null() {
                SDL_DestroyRendererNative(shared.renderer);
            }
            if !shared.window.is_null() {
                SDL_DestroyWindowNative(shared.window);
            }
            shared.texture = std::ptr::null_mut();
            shared.high_resolution_texture = std::ptr::null_mut();
            shared.renderer = std::ptr::null_mut();
            shared.window = std::ptr::null_mut();
            SDL_QuitNative();
        }
    });
}
#[repr(C)]
struct SdlGameController {
    _private: [u8; 0],
}
#[repr(C)]
struct SdlJoystick {
    _private: [u8; 0],
}
struct SdlController {
    raw: *mut SdlGameController,
}

#[derive(Clone, Copy)]
struct ControllerInput {
    actions: [bool; crate::settings::CONTROL_COUNT],
    horizontal: f64,
    forward: f64,
}

impl Default for ControllerInput {
    fn default() -> Self {
        Self {
            actions: [false; crate::settings::CONTROL_COUNT],
            horizontal: 0.0,
            forward: 0.0,
        }
    }
}

impl SdlController {
    fn new() -> Self {
        Self {
            raw: std::ptr::null_mut(),
        }
    }

    fn open_device(&mut self, index: i32) {
        if self.raw.is_null() && unsafe { SDL_IsGameController(index) != 0 } {
            self.raw = unsafe { SDL_GameControllerOpen(index) };
        }
    }

    fn open_first(&mut self) {
        if !self.raw.is_null() {
            return;
        }
        for index in 0..unsafe { SDL_NumJoysticks() }.max(0) {
            self.open_device(index);
            if !self.raw.is_null() {
                break;
            }
        }
    }

    fn device_added(&mut self, index: i32) {
        if self.raw.is_null() {
            self.open_device(index);
            if self.raw.is_null() {
                self.open_first();
            }
        }
    }

    fn device_removed(&mut self, instance_id: i32) {
        if self.raw.is_null() {
            return;
        }
        let joystick = unsafe { SDL_GameControllerGetJoystick(self.raw) };
        if !joystick.is_null() && unsafe { SDL_JoystickInstanceID(joystick) } == instance_id {
            unsafe { SDL_GameControllerClose(self.raw) };
            self.raw = std::ptr::null_mut();
            self.open_first();
        }
    }

    fn controls(&mut self) -> ControllerInput {
        if self.raw.is_null() {
            return ControllerInput::default();
        }
        if unsafe { SDL_GameControllerGetAttached(self.raw) } == 0 {
            unsafe { SDL_GameControllerClose(self.raw) };
            self.raw = std::ptr::null_mut();
            self.open_first();
        }
        if self.raw.is_null() {
            return ControllerInput::default();
        }
        let axis_x = unsafe { SDL_GameControllerGetAxis(self.raw, SDL_CONTROLLER_AXIS_LEFTX) };
        let axis_y = unsafe { SDL_GameControllerGetAxis(self.raw, SDL_CONTROLLER_AXIS_LEFTY) };
        let button = |index| unsafe { SDL_GameControllerGetButton(self.raw, index) != 0 };
        let mut controls = ControllerInput {
            horizontal: controller_axis_value(axis_x),
            forward: -controller_axis_value(axis_y),
            ..ControllerInput::default()
        };
        // DOS stores four physical buttons independently from their actions.
        // Keep the SDL standardized layout in DOS order: A, shoulder, B, X.
        let physical = [
            button(SDL_CONTROLLER_BUTTON_A),
            button(SDL_CONTROLLER_BUTTON_LEFTSHOULDER),
            button(SDL_CONTROLLER_BUTTON_B),
            button(SDL_CONTROLLER_BUTTON_X),
        ];
        for (index, action) in crate::settings::button_bindings()
            .controller
            .into_iter()
            .enumerate()
        {
            if let Some(control) = controls.actions.get_mut(action as usize) {
                *control |= physical[index];
            }
        }
        controls.actions[8] = button(SDL_CONTROLLER_BUTTON_DPAD_LEFT);
        controls.actions[9] = button(SDL_CONTROLLER_BUTTON_DPAD_DOWN);
        controls.actions[10] = button(SDL_CONTROLLER_BUTTON_DPAD_RIGHT);
        controls.actions[11] = button(SDL_CONTROLLER_BUTTON_DPAD_UP);
        controls
    }

    fn direction_held(&self, scancode: i32) -> bool {
        if self.raw.is_null() {
            return false;
        }
        let button = match scancode {
            value if value == SDL_SCANCODE_UP as i32 => SDL_CONTROLLER_BUTTON_DPAD_UP,
            value if value == SDL_SCANCODE_DOWN as i32 => SDL_CONTROLLER_BUTTON_DPAD_DOWN,
            SDL_SCANCODE_LEFT => SDL_CONTROLLER_BUTTON_DPAD_LEFT,
            SDL_SCANCODE_RIGHT => SDL_CONTROLLER_BUTTON_DPAD_RIGHT,
            _ => return false,
        };
        unsafe { SDL_GameControllerGetButton(self.raw, button) != 0 }
    }
}

fn controller_axis_value(value: i16) -> f64 {
    const DEADZONE: f64 = 8_000.0;
    let value = f64::from(value);
    let normalized = if value > DEADZONE {
        (value - DEADZONE) / (f64::from(i16::MAX) - DEADZONE)
    } else if value < -DEADZONE {
        (value + DEADZONE) / (f64::from(i16::MAX) - DEADZONE)
    } else {
        0.0
    };
    normalized.clamp(-1.0, 1.0)
}

impl Drop for SdlController {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe { SDL_GameControllerClose(self.raw) };
        }
    }
}

struct SdlAcknowledgementInput {
    controller: SdlController,
}

impl SdlAcknowledgementInput {
    fn new() -> Self {
        initialize_subsystem(SDL_INIT_GAMECONTROLLER);
        let mut controller = SdlController::new();
        controller.open_first();
        Self { controller }
    }

    fn handles(&mut self, event: &SdlEvent) -> bool {
        let event_type = u32::from_ne_bytes(event.bytes[0..4].try_into().unwrap());
        match event_type {
            SDL_CONTROLLERDEVICEADDED => {
                let device_index = i32::from_ne_bytes(event.bytes[8..12].try_into().unwrap());
                self.controller.device_added(device_index);
            }
            SDL_CONTROLLERDEVICEREMOVED => {
                let instance_id = i32::from_ne_bytes(event.bytes[8..12].try_into().unwrap());
                self.controller.device_removed(instance_id);
            }
            _ => {}
        }
        enabled_user_input_event(event_type)
    }
}

fn enabled_user_input_event(event_type: u32) -> bool {
    match event_type {
        SDL_KEYDOWN => true,
        SDL_MOUSEBUTTONDOWN => crate::settings::mouse_input_enabled(),
        SDL_CONTROLLERBUTTONDOWN => crate::settings::controller_input_enabled(),
        _ => false,
    }
}
#[repr(C)]
struct SdlRect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}
#[repr(C)]
struct SdlAudioSpec {
    frequency: i32,
    format: u16,
    channels: u8,
    silence: u8,
    samples: u16,
    padding: u16,
    size: u32,
    callback: Option<unsafe extern "C" fn(*mut (), *mut u8, i32)>,
    userdata: *mut (),
}

struct ActiveEffect {
    samples: Vec<i16>,
    cursor: usize,
    adlib: Option<crate::opl2::AdLibEffectPlayer>,
    priority: u16,
    sound: usize,
    position: Option<(f64, f64)>,
}

impl ActiveEffect {
    fn next_sample(&mut self) -> Option<i16> {
        let sample = *self.samples.get(self.cursor)?;
        self.cursor += 1;
        Some(sample)
    }

    fn is_playing(&self) -> bool {
        self.cursor < self.samples.len()
            || self
                .adlib
                .as_ref()
                .is_some_and(crate::opl2::AdLibEffectPlayer::is_playing)
    }

    fn service_opl(&mut self, synth: &mut crate::opl2::Opl2Synth) {
        if self
            .adlib
            .as_mut()
            .is_some_and(|effect| !effect.service(synth))
        {
            self.adlib = None;
        }
    }
}

/// Audio state used exclusively by SDL's locked callback device.
struct AudioMixer {
    music: Option<crate::opl2::ImfPlayer>,
    opl2: crate::opl2::Opl2Synth,
    synthesized_effect: Option<ActiveEffect>,
    digitized_effect: Option<ActiveEffect>,
    listener: (f64, f64, f64),
}

impl AudioMixer {
    fn stop_synthesized_effect(&mut self) {
        if self
            .synthesized_effect
            .as_ref()
            .is_some_and(|effect| effect.adlib.is_some())
        {
            // SDL_ALStopSound keys off the shared channel-zero voice.
            self.opl2.write_register(0xb0, 0);
        }
        self.synthesized_effect = None;
    }

    fn stop_music(&mut self) -> Option<crate::opl2::ImfPlayer> {
        let music = self.music.take()?;
        let effect_owns_channel_zero = self
            .synthesized_effect
            .as_ref()
            .is_some_and(|effect| effect.adlib.is_some());
        for channel in 0..9 {
            if music.channel_mask() & (1 << channel) != 0
                && !(channel == 0 && effect_owns_channel_zero)
            {
                // SD_MusicOff keys off the track voices while leaving an
                // active SDL_AL sound on the shared effects channel alone.
                self.opl2.write_register(0xb0 + channel as u8, 0);
            }
        }
        Some(music)
    }
}

/// OPL and IMF playback state carried between consecutive startup screens.
pub struct IntroMusic {
    player: Option<crate::opl2::ImfPlayer>,
    synth: crate::opl2::Opl2Synth,
}

impl IntroMusic {
    pub fn new(player: Option<crate::opl2::ImfPlayer>) -> Self {
        Self {
            player,
            synth: new_music_synth(),
        }
    }
}

#[derive(Clone, Copy)]
enum PauseTransitionPhase {
    FadeOut,
    FadeIn,
}

pub struct HelpArticles {
    pages: Vec<ending::EndingPage>,
    frame: [Picture; 4],
}

struct PauseTransition {
    started: Instant,
    phase: PauseTransitionPhase,
    frame: Vec<u32>,
    resume_audio: bool,
    start_control_panel_music: bool,
    start_help_music: bool,
    start_high_score_music: bool,
    exit_after_fade_out: bool,
    fade_in_delay: Duration,
    fade_out_duration: Duration,
    fade_in_duration: Duration,
}

impl PauseTransition {
    fn new(frame: &[u32], resume_audio: bool) -> Self {
        let ten_tics = Duration::from_secs_f64(10.0 / 70.0);
        let fade_in_duration = if resume_audio {
            Duration::from_secs_f64(30.0 / 70.0)
        } else {
            ten_tics
        };
        Self::with_durations(frame, resume_audio, ten_tics, fade_in_duration)
    }

    fn with_durations(
        frame: &[u32],
        resume_audio: bool,
        fade_out_duration: Duration,
        fade_in_duration: Duration,
    ) -> Self {
        Self {
            started: Instant::now(),
            phase: PauseTransitionPhase::FadeOut,
            frame: frame.to_vec(),
            resume_audio,
            start_control_panel_music: false,
            start_help_music: false,
            start_high_score_music: false,
            exit_after_fade_out: false,
            fade_in_delay: Duration::ZERO,
            fade_out_duration,
            fade_in_duration,
        }
    }

    fn enter_control_panel(frame: &[u32]) -> Self {
        let mut transition = Self::with_durations(
            frame,
            false,
            Duration::from_secs_f64(30.0 / 70.0),
            Duration::from_secs_f64(10.0 / 70.0),
        );
        transition.start_control_panel_music = true;
        transition
    }

    fn enter_help_articles(frame: &[u32]) -> Self {
        let mut transition = Self::with_durations(
            frame,
            false,
            Duration::from_secs_f64(30.0 / 70.0),
            Duration::from_secs_f64(10.0 / 70.0),
        );
        transition.start_help_music = true;
        transition
    }

    fn enter_high_scores(frame: &[u32]) -> Self {
        let mut transition = Self::new(frame, false);
        transition.start_high_score_music = true;
        transition
    }

    fn exit_application(frame: &[u32]) -> Self {
        let mut transition = Self::new(frame, false);
        transition.exit_after_fade_out = true;
        transition
    }
}

struct PauseHighScoreArt {
    title: Option<Picture>,
    columns: [Option<Picture>; 3],
    shareware: bool,
}

struct PauseNewGameArt {
    background: Picture,
    episode_options: Vec<String>,
    episode_illustrations: Vec<Picture>,
    difficulty_options: Vec<String>,
    difficulty_illustrations: Vec<Picture>,
}

impl PauseNewGameArt {
    fn load(graphics: &crate::vga::VgaArchive, data_variant: &str) -> Result<Self, Box<dyn Error>> {
        let shareware = data_variant.eq_ignore_ascii_case("WL1");
        let episode_count = if shareware { 1 } else { 6 };
        let background = graphics
            .picture(if shareware { 29 } else { 18 })?
            .ok_or("VGA menu background picture chunk is unexpectedly sparse")?;
        let cursor = graphics
            .picture(if shareware { 22 } else { 11 })?
            .ok_or("VGA menu cursor picture chunk is unexpectedly sparse")?;
        let episode_names = [
            "Episode 1:\nEscape from Wolfenstein",
            "Episode 2:\nOperation: Eisenfaust",
            "Episode 3:\nDie, Fuehrer, Die!",
            "Episode 4:\nA Dark Secret",
            "Episode 5:\nTrail of the Madman",
            "Episode 6:\nConfrontation",
        ];
        let episode_options = episode_names[..episode_count]
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        let episode_base = if shareware { 41 } else { 30 };
        let mut episode_illustrations = if episode_count > 1 {
            (0..episode_count)
                .map(|index| {
                    graphics.picture(episode_base + index)?.ok_or_else(|| {
                        Box::<dyn Error>::from("VGA episode picture chunk is unexpectedly sparse")
                    })
                })
                .collect::<Result<Vec<_>, Box<dyn Error>>>()?
        } else {
            Vec::new()
        };
        episode_illustrations.push(cursor.clone());

        let difficulty_names = [
            "Can I play, Daddy?",
            "Don't hurt me.",
            "Bring 'em on!",
            "I am Death incarnate!",
        ];
        let difficulty_options = difficulty_names.map(str::to_owned).to_vec();
        let difficulty_base = if shareware { 30 } else { 19 };
        let mut difficulty_illustrations = (0..difficulty_options.len())
            .map(|index| {
                graphics.picture(difficulty_base + index)?.ok_or_else(|| {
                    Box::<dyn Error>::from("VGA difficulty picture chunk is unexpectedly sparse")
                })
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        difficulty_illustrations.push(cursor);

        Ok(Self {
            background,
            episode_options,
            episode_illustrations,
            difficulty_options,
            difficulty_illustrations,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PauseNewGameStep {
    Episode,
    Confirm,
    Difficulty,
}

struct PauseNewGameSelection {
    step: PauseNewGameStep,
    episode: usize,
    selected: usize,
}

impl AudioMixer {
    fn accepts_effect(&self, digitized: bool, priority: u16) -> bool {
        let effect = if digitized {
            &self.digitized_effect
        } else {
            &self.synthesized_effect
        };
        !effect
            .as_ref()
            .is_some_and(|active| active.is_playing() && active.priority > priority)
    }

    fn play_effect(
        &mut self,
        samples: Vec<i16>,
        adlib: Option<crate::opl2::AdLibEffectPlayer>,
        priority: u16,
        sound: usize,
        position: Option<(f64, f64)>,
        digitized: bool,
    ) {
        let effect = if digitized {
            &mut self.digitized_effect
        } else {
            &mut self.synthesized_effect
        };
        if effect
            .as_ref()
            .is_some_and(|active| active.is_playing() && active.priority > priority)
        {
            return;
        }
        *effect = Some(ActiveEffect {
            samples,
            cursor: 0,
            adlib,
            priority,
            sound,
            position,
        });
    }
}

fn effect_channel_gains(listener: (f64, f64, f64), position: Option<(f64, f64)>) -> (f64, f64) {
    position
        .map(|(source_x, source_y)| {
            let attenuation = crate::audio_position::channel_attenuation(
                listener.0, listener.1, listener.2, source_x, source_y,
            );
            (
                gain_for_attenuation(attenuation.0),
                gain_for_attenuation(attenuation.1),
            )
        })
        .unwrap_or((1.0, 1.0))
}

unsafe extern "C" fn level_audio_callback(userdata: *mut (), stream: *mut u8, len: i32) {
    if stream.is_null() || len <= 0 {
        return;
    }
    unsafe { std::ptr::write_bytes(stream, 0, len as usize) };
    let output_samples = len as usize / std::mem::size_of::<i16>();
    let output = unsafe { std::slice::from_raw_parts_mut(stream.cast::<i16>(), output_samples) };
    if userdata.is_null() || output.len() < 2 {
        return;
    }
    let mixer_cell = unsafe { &*userdata.cast::<UnsafeCell<AudioMixer>>() };
    let mixer = unsafe { &mut *mixer_cell.get() };
    let music_gain = if crate::settings::music_enabled() {
        1.0
    } else {
        0.0
    };
    let synthesized_gain = if crate::settings::effects_enabled() {
        1.0
    } else {
        0.0
    };
    let digitized_gain = if crate::settings::digitized_effects_enabled() {
        1.0
    } else {
        0.0
    };
    if synthesized_gain == 0.0 {
        mixer.stop_synthesized_effect();
    } else if !crate::settings::adlib_effects_enabled()
        && mixer
            .synthesized_effect
            .as_ref()
            .is_some_and(|effect| effect.adlib.is_some())
    {
        mixer.stop_synthesized_effect();
    }
    if digitized_gain == 0.0 {
        mixer.digitized_effect = None;
    }
    let listener = mixer.listener;
    let AudioMixer {
        music,
        opl2,
        synthesized_effect,
        digitized_effect,
        ..
    } = mixer;
    for output_chunk in output.chunks_mut(2_048) {
        let frame_count = output_chunk.len() / 2;
        let (left_gain, right_gain) = digitized_effect
            .as_ref()
            .map(|active| effect_channel_gains(listener, active.position))
            .unwrap_or((1.0, 1.0));
        for frame in 0..frame_count {
            if synthesized_gain > 0.0 {
                if let Some(effect) = synthesized_effect.as_mut() {
                    effect.service_opl(opl2);
                }
            }
            if music_gain > 0.0 {
                if let Some(player) = music.as_mut() {
                    player.service(opl2);
                }
            }
            let channel_mask = if music_gain > 0.0 {
                0x01ff
            } else if synthesized_gain > 0.0
                && synthesized_effect
                    .as_ref()
                    .is_some_and(|effect| effect.adlib.is_some())
            {
                0x0001
            } else {
                0
            };
            let opl_sample = opl2.next_sample_masked(channel_mask) as i32;
            let synth_pcm_sample = synthesized_effect
                .as_mut()
                .and_then(ActiveEffect::next_sample)
                .unwrap_or_default() as f64
                * synthesized_gain;
            let digital_sample = digitized_effect
                .as_mut()
                .and_then(ActiveEffect::next_sample)
                .unwrap_or_default() as f64
                * digitized_gain;
            let effect_left = (synth_pcm_sample + digital_sample * left_gain).round() as i32;
            let effect_right = (synth_pcm_sample + digital_sample * right_gain).round() as i32;
            let output_index = frame * 2;
            output_chunk[output_index] =
                (opl_sample + effect_left).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            output_chunk[output_index + 1] =
                (opl_sample + effect_right).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        }
    }
}

fn new_music_synth() -> crate::opl2::Opl2Synth {
    let mut synth = crate::opl2::Opl2Synth::new(OUTPUT_RATE);
    synth.write_register(0x01, 0x20);
    synth
}

unsafe fn start_screen_music(music: Option<crate::opl2::ImfPlayer>) -> ScreenMusic {
    unsafe { start_screen_music_with_state(IntroMusic::new(music)) }
}

unsafe fn start_screen_music_with_state(state: IntroMusic) -> ScreenMusic {
    let IntroMusic { player, synth } = state;
    let mixer = Box::new(UnsafeCell::new(AudioMixer {
        music: player,
        opl2: synth,
        synthesized_effect: None,
        digitized_effect: None,
        listener: (0.0, 0.0, 0.0),
    }));
    if initialize_subsystem(SDL_INIT_AUDIO) != 0 {
        return ScreenMusic {
            device: SdlAudioDeviceGuard(0),
            mixer,
        };
    }
    let audio_spec = SdlAudioSpec {
        frequency: crate::audio::OUTPUT_RATE as i32,
        format: SDL_AUDIO_S16LSB,
        channels: 2,
        silence: 0,
        samples: 1_024,
        padding: 0,
        size: 0,
        callback: Some(level_audio_callback),
        userdata: (&*mixer as *const UnsafeCell<AudioMixer>).cast_mut().cast(),
    };
    let device = open_audio_device(crate::settings::audio_device(), &audio_spec);
    if device != 0 {
        unsafe { SDL_PauseAudioDevice(device, 0) };
    }
    ScreenMusic {
        device: SdlAudioDeviceGuard(device),
        mixer,
    }
}

unsafe fn take_intro_music(audio_device: u32, mixer: &UnsafeCell<AudioMixer>) -> IntroMusic {
    if audio_device != 0 {
        unsafe {
            SDL_PauseAudioDevice(audio_device, 1);
            SDL_LockAudioDevice(audio_device);
        }
    }
    let (player, synth) = unsafe {
        let mixer = &mut *mixer.get();
        (
            mixer.music.take(),
            std::mem::replace(&mut mixer.opl2, new_music_synth()),
        )
    };
    if audio_device != 0 {
        unsafe { SDL_UnlockAudioDevice(audio_device) };
    }
    IntroMusic { player, synth }
}

unsafe fn take_screen_music(
    audio_device: u32,
    mixer: &UnsafeCell<AudioMixer>,
) -> Option<crate::opl2::ImfPlayer> {
    if audio_device != 0 {
        unsafe {
            SDL_PauseAudioDevice(audio_device, 1);
            SDL_LockAudioDevice(audio_device);
        }
    }
    let music = unsafe { (&mut *mixer.get()).music.take() };
    if audio_device != 0 {
        unsafe { SDL_UnlockAudioDevice(audio_device) };
    }
    music
}
#[repr(C, align(8))]
struct SdlEvent {
    bytes: [u8; 56],
}

// SDL2 defines SDL_Event as 56 bytes and eight-byte aligned on 64-bit
// Windows, Linux, and macOS. These checks protect the hand-written FFI ABI.
#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(std::mem::size_of::<SdlEvent>() == 56);
    assert!(std::mem::align_of::<SdlEvent>() == 8);
    assert!(std::mem::size_of::<SdlAudioSpec>() == 32);
    assert!(std::mem::align_of::<SdlAudioSpec>() == 8);
    assert!(std::mem::size_of::<SdlRect>() == 16);
};

const SDL_INIT_VIDEO: u32 = 0x20;
const SDL_INIT_AUDIO: u32 = 0x10;
const SDL_INIT_GAMECONTROLLER: u32 = 0x2000;
const SDL_WINDOWPOS_CENTERED: i32 = 0x2FFF0000u32 as i32;
const SDL_WINDOW_SHOWN: u32 = 0x4;
const SDL_WINDOW_RESIZABLE: u32 = 0x20;
const SDL_WINDOW_ALLOW_HIGHDPI: u32 = 0x2000;
const SDL_WINDOW_FULLSCREEN_DESKTOP: u32 = 0x0000_1001;
const SDL_RENDERER_ACCELERATED: u32 = 0x2;
const SDL_RENDERER_PRESENTVSYNC: u32 = 0x4;
const SDL_TEXTUREACCESS_STREAMING: i32 = 1;
const SDL_PIXELFORMAT_ARGB8888: u32 = 372645892;
const SDL_QUIT: u32 = 0x100;
const SDL_WINDOWEVENT: u32 = 0x200;
const SDL_MOUSEMOTION: u32 = 0x400;
const SDL_MOUSEBUTTONDOWN: u32 = 0x401;
const SDL_MOUSEBUTTONUP: u32 = 0x402;
const PAUSE_VIEW_SIZE_MOUSE_THRESHOLD: i32 = 24;
const SDL_CONTROLLERBUTTONDOWN: u32 = 0x651;
const SDL_CONTROLLERDEVICEADDED: u32 = 0x653;
const SDL_CONTROLLERDEVICEREMOVED: u32 = 0x654;
const SDL_KEYDOWN: u32 = 0x300;
const SDL_KEYUP: u32 = 0x301;
// Value 1 is SDL's native repeat event; 2 tags the repeat events Rust schedules.
const SDL_SYNTHETIC_KEY_REPEAT: u8 = 2;
const SDL_TEXTINPUT: u32 = 0x303;
const SDL_SCANCODE_ESCAPE: usize = 41;
const SDL_SCANCODE_RETURN: i32 = 40;
const SDL_SCANCODE_N: i32 = 17;
const SDL_SCANCODE_Y: i32 = 28;
const SDL_SCANCODE_PAUSE: i32 = 72;
const SDL_SCANCODE_F1: i32 = 58;
const SDL_SCANCODE_F2: i32 = 59;
const SDL_SCANCODE_F3: i32 = 60;
const SDL_SCANCODE_F4: i32 = 61;
const SDL_SCANCODE_RIGHT: i32 = 79;
const SDL_SCANCODE_LEFT: i32 = 80;
const SDL_SCANCODE_BACKSPACE: i32 = 42;
const SDL_CONTROLLER_AXIS_LEFTX: i32 = 0;
const SDL_CONTROLLER_AXIS_LEFTY: i32 = 1;
const SDL_CONTROLLER_BUTTON_A: i32 = 0;
const SDL_CONTROLLER_BUTTON_B: i32 = 1;
const SDL_CONTROLLER_BUTTON_X: i32 = 2;
const SDL_CONTROLLER_BUTTON_START: i32 = 6;
const SDL_CONTROLLER_BUTTON_LEFTSHOULDER: i32 = 9;
const SDL_CONTROLLER_BUTTON_DPAD_UP: i32 = 11;
const SDL_CONTROLLER_BUTTON_DPAD_DOWN: i32 = 12;
const SDL_CONTROLLER_BUTTON_DPAD_LEFT: i32 = 13;
const SDL_CONTROLLER_BUTTON_DPAD_RIGHT: i32 = 14;
fn controller_menu_scancode(button: u8) -> Option<i32> {
    match i32::from(button) {
        SDL_CONTROLLER_BUTTON_A => Some(SDL_SCANCODE_RETURN),
        SDL_CONTROLLER_BUTTON_B | SDL_CONTROLLER_BUTTON_START => Some(SDL_SCANCODE_ESCAPE as i32),
        SDL_CONTROLLER_BUTTON_DPAD_UP => Some(SDL_SCANCODE_UP as i32),
        SDL_CONTROLLER_BUTTON_DPAD_DOWN => Some(SDL_SCANCODE_DOWN as i32),
        SDL_CONTROLLER_BUTTON_DPAD_LEFT => Some(SDL_SCANCODE_LEFT),
        SDL_CONTROLLER_BUTTON_DPAD_RIGHT => Some(SDL_SCANCODE_RIGHT),
        _ => None,
    }
}

fn controller_gameplay_button_slot(button: u8) -> Option<usize> {
    match i32::from(button) {
        SDL_CONTROLLER_BUTTON_A => Some(0),
        SDL_CONTROLLER_BUTTON_LEFTSHOULDER => Some(1),
        SDL_CONTROLLER_BUTTON_B => Some(2),
        SDL_CONTROLLER_BUTTON_X => Some(3),
        _ => None,
    }
}

unsafe fn push_keydown_scancode(scancode: i32) {
    let mut event = SdlEvent { bytes: [0; 56] };
    event.bytes[0..4].copy_from_slice(&SDL_KEYDOWN.to_ne_bytes());
    event.bytes[12] = 1;
    event.bytes[16..20].copy_from_slice(&scancode.to_ne_bytes());
    unsafe { SDL_PushEvent(&mut event) };
}

unsafe fn push_repeated_keydown_scancode(scancode: i32) {
    let mut event = SdlEvent { bytes: [0; 56] };
    event.bytes[0..4].copy_from_slice(&SDL_KEYDOWN.to_ne_bytes());
    event.bytes[12] = 1;
    event.bytes[13] = SDL_SYNTHETIC_KEY_REPEAT;
    event.bytes[16..20].copy_from_slice(&scancode.to_ne_bytes());
    unsafe { SDL_PushEvent(&mut event) };
}
const SDL_SCANCODE_PAGEUP: i32 = 75;
const SDL_SCANCODE_PAGEDOWN: i32 = 78;
const SDL_SCANCODE_UP: usize = 82;
const SDL_SCANCODE_DOWN: usize = 81;
const SDL_SCANCODE_F5: i32 = 62;
const SDL_SCANCODE_F6: i32 = 63;
const SDL_SCANCODE_F7: i32 = 64;
const SDL_SCANCODE_F8: i32 = 65;
const SDL_SCANCODE_F9: usize = 66;
const SDL_SCANCODE_F10: i32 = 67;
const SDL_MOUSE_LEFT_MASK: u8 = 1 << 0;
const SDL_MOUSE_MIDDLE_MASK: u8 = 1 << 1;
const SDL_MOUSE_RIGHT_MASK: u8 = 1 << 2;
const SDL_WINDOWEVENT_FOCUS_GAINED: u8 = 12;
const SDL_WINDOWEVENT_FOCUS_LOST: u8 = 13;

fn window_creation_flags() -> u32 {
    SDL_WINDOW_SHOWN
        | SDL_WINDOW_RESIZABLE
        | SDL_WINDOW_ALLOW_HIGHDPI
        | if crate::settings::fullscreen_enabled() {
            SDL_WINDOW_FULLSCREEN_DESKTOP
        } else {
            0
        }
}

fn set_gameplay_mouse_capture() {
    let enabled = crate::settings::mouse_input_enabled();
    unsafe {
        let _ = SDL_SetRelativeMouseMode(i32::from(enabled));
    }
}

pub fn audio_output_devices() -> Vec<String> {
    unsafe {
        let audio_was_initialized = SDL_WasInit(SDL_INIT_AUDIO) != 0;
        if initialize_subsystem(SDL_INIT_AUDIO) != 0 {
            return Vec::new();
        }
        let count = SDL_GetNumAudioDevices(0).max(0);
        let mut devices = Vec::with_capacity(count as usize);
        for index in 0..count {
            let name = SDL_GetAudioDeviceName(index, 0);
            if !name.is_null() {
                let name = std::ffi::CStr::from_ptr(name)
                    .to_string_lossy()
                    .into_owned();
                if !name.is_empty() && !devices.contains(&name) {
                    devices.push(name);
                }
            }
        }
        if !audio_was_initialized {
            SDL_QuitSubSystem(SDL_INIT_AUDIO);
        }
        devices
    }
}

fn open_audio_device(name: Option<String>, spec: &SdlAudioSpec) -> u32 {
    unsafe {
        if let Some(name) = name {
            if let Ok(name) = CString::new(name) {
                let device = SDL_OpenAudioDevice(name.as_ptr(), 0, spec, std::ptr::null_mut(), 0);
                if device != 0 {
                    return device;
                }
            }
        }
        SDL_OpenAudioDevice(std::ptr::null(), 0, spec, std::ptr::null_mut(), 0)
    }
}

/// Draw the 320x200 VGA image with the 4:3 display aspect used by DOS PCs.
/// Fullscreen modes wider than 4:3 retain black side bars instead of stretching.
fn fade_frame(pixels: &mut [u32], brightness: f64) {
    let brightness = brightness.clamp(0.0, 1.0);
    for pixel in pixels.iter_mut().take(WIDTH * HEIGHT) {
        let red = (((*pixel >> 16) & 0xff) as f64 * brightness) as u32;
        let green = (((*pixel >> 8) & 0xff) as f64 * brightness) as u32;
        let blue = ((*pixel & 0xff) as f64 * brightness) as u32;
        *pixel = 0xff00_0000 | (red << 16) | (green << 8) | blue;
    }
}

fn dos_menu_cursor_frame(elapsed: Duration) -> usize {
    let tics = (elapsed.as_secs_f64() * 70.0) as u64;
    usize::from(matches!(tics % 89, 9..18))
}

unsafe fn present_preload_screen(
    renderer: *mut SdlRenderer,
    texture: *mut SdlTexture,
    pixels: &mut [u32],
    picture: &Picture,
    status_bar: &Picture,
) -> Result<bool, Box<dyn Error>> {
    const PRELOAD_WAIT: Duration = Duration::from_secs(1);
    world::render_preload_screen(pixels, picture, status_bar, 0);
    let mut full_frame = pixels.to_vec();
    let mut event = SdlEvent { bytes: [0; 56] };
    let fade_steps = 30u32;
    let mut quit = false;
    let mut skip_wait = false;

    let present = |pixels: &[u32]| -> Result<(), Box<dyn Error>> {
        if SDL_UpdateTexture(
            texture,
            std::ptr::null(),
            pixels.as_ptr(),
            (WIDTH * 4) as i32,
        ) != 0
            || SDL_RenderClear(renderer) != 0
            || render_frame(renderer, texture) != 0
        {
            return Err(sdl_error());
        }
        SDL_RenderPresent(renderer);
        Ok(())
    };

    for step in 0..=fade_steps {
        while SDL_PollEvent(&mut event) != 0 {
            match u32::from_ne_bytes(event.bytes[0..4].try_into().unwrap()) {
                SDL_QUIT => quit = true,
                event_type if enabled_user_input_event(event_type) => skip_wait = true,
                _ => {}
            }
        }
        if quit {
            return Ok(false);
        }
        pixels.copy_from_slice(&full_frame);
        fade_frame(pixels, f64::from(step) / f64::from(fade_steps));
        present(pixels)?;
        SDL_Delay(14);
    }

    let wait_started = Instant::now();
    while !skip_wait && wait_started.elapsed() < PRELOAD_WAIT {
        while SDL_PollEvent(&mut event) != 0 {
            match u32::from_ne_bytes(event.bytes[0..4].try_into().unwrap()) {
                SDL_QUIT => quit = true,
                event_type if enabled_user_input_event(event_type) => skip_wait = true,
                _ => {}
            }
        }
        if quit || skip_wait {
            break;
        }
        let progress = (wait_started.elapsed().as_millis() * 1000 / PRELOAD_WAIT.as_millis().max(1))
            .min(1000) as usize;
        world::render_preload_screen(pixels, picture, status_bar, progress);
        full_frame.copy_from_slice(pixels);
        present(&full_frame)?;
        SDL_Delay(14);
    }

    for step in 0..=fade_steps {
        while SDL_PollEvent(&mut event) != 0 {
            if u32::from_ne_bytes(event.bytes[0..4].try_into().unwrap()) == SDL_QUIT {
                quit = true;
            }
        }
        if quit {
            return Ok(false);
        }
        pixels.copy_from_slice(&full_frame);
        fade_frame(pixels, 1.0 - f64::from(step) / f64::from(fade_steps));
        present(pixels)?;
        SDL_Delay(14);
    }
    Ok(true)
}

fn render_frame(renderer: *mut SdlRenderer, texture: *mut SdlTexture) -> i32 {
    let (mut output_width, mut output_height) = (0, 0);
    if unsafe { SDL_GetRendererOutputSize(renderer, &mut output_width, &mut output_height) } != 0
        || output_width <= 0
        || output_height <= 0
    {
        return -1;
    }
    let (width, height) = if i64::from(output_width) * 3 <= i64::from(output_height) * 4 {
        (output_width, output_width * 3 / 4)
    } else {
        (output_height * 4 / 3, output_height)
    };
    let destination = SdlRect {
        x: (output_width - width) / 2,
        y: (output_height - height) / 2,
        w: width,
        h: height,
    };
    unsafe { SDL_RenderCopy(renderer, texture, std::ptr::null(), &destination) }
}

fn render_frame_region(
    renderer: *mut SdlRenderer,
    texture: *mut SdlTexture,
    source_width: usize,
    source_height: usize,
) -> i32 {
    let (mut output_width, mut output_height) = (0, 0);
    if unsafe { SDL_GetRendererOutputSize(renderer, &mut output_width, &mut output_height) } != 0
        || output_width <= 0
        || output_height <= 0
    {
        return -1;
    }
    let (width, height) = if i64::from(output_width) * 3 <= i64::from(output_height) * 4 {
        (output_width, output_width * 3 / 4)
    } else {
        (output_height * 4 / 3, output_height)
    };
    let source = SdlRect {
        x: 0,
        y: 0,
        w: source_width as i32,
        h: source_height as i32,
    };
    let destination = SdlRect {
        x: (output_width - width) / 2,
        y: (output_height - height) / 2,
        w: width,
        h: height,
    };
    unsafe {
        SDL_RenderCopy(
            renderer,
            texture,
            &source as *const SdlRect as *const (),
            &destination,
        )
    }
}

fn menu_pointer_coordinates(
    window: *mut SdlWindow,
    renderer: *mut SdlRenderer,
    window_x: i32,
    window_y: i32,
) -> Option<(usize, usize)> {
    let (mut window_width, mut window_height) = (0, 0);
    let (mut output_width, mut output_height) = (0, 0);
    unsafe {
        SDL_GetWindowSize(window, &mut window_width, &mut window_height);
        if SDL_GetRendererOutputSize(renderer, &mut output_width, &mut output_height) != 0 {
            return None;
        }
    }
    if window_width <= 0
        || window_height <= 0
        || output_width <= 0
        || output_height <= 0
        || window_x < 0
        || window_y < 0
    {
        return None;
    }
    let output_x = i64::from(window_x) * i64::from(output_width) / i64::from(window_width);
    let output_y = i64::from(window_y) * i64::from(output_height) / i64::from(window_height);
    let (width, height) = if i64::from(output_width) * 3 <= i64::from(output_height) * 4 {
        (output_width, output_width * 3 / 4)
    } else {
        (output_height * 4 / 3, output_height)
    };
    let left = (output_width - width) / 2;
    let top = (output_height - height) / 2;
    if output_x < i64::from(left)
        || output_y < i64::from(top)
        || output_x >= i64::from(left + width)
        || output_y >= i64::from(top + height)
    {
        return None;
    }
    let x = ((output_x - i64::from(left)) * WIDTH as i64 / i64::from(width)) as usize;
    let y = ((output_y - i64::from(top)) * HEIGHT as i64 / i64::from(height)) as usize;
    Some((x, y))
}

fn menu_pointer_selection(
    window: *mut SdlWindow,
    renderer: *mut SdlRenderer,
    layout: world::ChoiceMenuLayout,
    options_len: usize,
    selected: usize,
    window_x: i32,
    window_y: i32,
) -> Option<usize> {
    let (x, y) = menu_pointer_coordinates(window, renderer, window_x, window_y)?;
    world::choice_menu_selection_at(layout, options_len, selected, x, y)
}

#[derive(Clone, Copy)]
enum PauseMenuMouseTarget {
    Root,
    Confirm,
    Slot,
    Sound,
    Controls,
    Display,
    NewGameEpisode,
    NewGameDifficulty,
}

fn pause_menu_selection_at(
    target: PauseMenuMouseTarget,
    options_len: usize,
    x: usize,
    y: usize,
) -> Option<usize> {
    let (left, right, first_y, row_height, rows) = match target {
        PauseMenuMouseTarget::Root => (68, 246, 47, 10, options_len),
        PauseMenuMouseTarget::Confirm => (100, 220, 102, 14, 2),
        PauseMenuMouseTarget::Slot => (75, 250, 55, 13, options_len),
        PauseMenuMouseTarget::Controls => (
            4,
            316,
            if options_len >= 20 {
                40
            } else if options_len > 17 {
                44
            } else {
                50
            },
            8,
            options_len,
        ),
        PauseMenuMouseTarget::Display => (32, 288, 62, 12, options_len),
        PauseMenuMouseTarget::NewGameEpisode => {
            return world::choice_menu_selection_at(
                world::ChoiceMenuLayout::Episode,
                options_len,
                0,
                x,
                y,
            );
        }
        PauseMenuMouseTarget::NewGameDifficulty => {
            return world::choice_menu_selection_at(
                world::ChoiceMenuLayout::Difficulty,
                options_len,
                0,
                x,
                y,
            );
        }
        PauseMenuMouseTarget::Sound => {
            if !(48..290).contains(&x) {
                return None;
            }
            let index = match y {
                20..33 => 0,
                33..46 => 1,
                46..59 => 2,
                85..98 => 3,
                98..111 => 4,
                150..163 => 5,
                163..176 => 6,
                176..189 => 7,
                _ => return None,
            };
            return (index < options_len).then_some(index);
        }
    };
    if x < left || x >= right || y < first_y {
        return None;
    }
    let row = (y - first_y) / row_height;
    (row < rows && row < options_len).then_some(row)
}

fn active_pause_menu_mouse_target(
    confirmation_open: bool,
    new_game_step: Option<PauseNewGameStep>,
    new_game_option_counts: (usize, usize),
    slot_mode: Option<bool>,
    sound_open: bool,
    controls_open: bool,
    controls_button_mapping: Option<bool>,
    controls_movement_mapping: bool,
    controls_capturing: bool,
    display_open: bool,
    slot_count: usize,
) -> Option<(PauseMenuMouseTarget, usize)> {
    if confirmation_open {
        Some((PauseMenuMouseTarget::Confirm, 2))
    } else if let Some(step) = new_game_step {
        match step {
            PauseNewGameStep::Episode => Some((
                PauseMenuMouseTarget::NewGameEpisode,
                new_game_option_counts.0,
            )),
            PauseNewGameStep::Difficulty => Some((
                PauseMenuMouseTarget::NewGameDifficulty,
                new_game_option_counts.1,
            )),
            PauseNewGameStep::Confirm => Some((PauseMenuMouseTarget::Confirm, 2)),
        }
    } else if slot_mode.is_some() {
        Some((PauseMenuMouseTarget::Slot, slot_count))
    } else if sound_open {
        Some((PauseMenuMouseTarget::Sound, 8))
    } else if controls_open && !controls_capturing {
        let count = if controls_movement_mapping {
            crate::settings::MOVE_DIRECTION_COUNT + 1
        } else {
            controls_button_mapping.map_or(
                crate::settings::CONTROL_COUNT + PAUSE_CONTROL_SETTINGS_COUNT,
                |mouse| {
                    (if mouse {
                        MOUSE_BUTTON_ACTIONS.len()
                    } else {
                        CONTROLLER_BUTTON_ACTIONS.len()
                    }) + 1
                },
            )
        };
        Some((PauseMenuMouseTarget::Controls, count))
    } else if display_open {
        Some((PauseMenuMouseTarget::Display, 8))
    } else {
        Some((PauseMenuMouseTarget::Root, 9))
    }
}

fn present_pause_io_progress(
    renderer: *mut SdlRenderer,
    texture: *mut SdlTexture,
    pixels: &mut [u32],
    font: Option<&BitmapFont>,
    pictures: Option<&world::PauseSoundMenuPictures>,
    loading: bool,
    animation_frame: u8,
) -> Result<(), Box<dyn Error>> {
    let (Some(font), Some(pictures)) = (font, pictures) else {
        return Ok(());
    };
    world::render_pause_io_progress_overlay(pixels, font, pictures, loading, animation_frame);
    if unsafe {
        SDL_UpdateTexture(
            texture,
            std::ptr::null(),
            pixels.as_ptr(),
            (WIDTH * 4) as i32,
        )
    } != 0
        || unsafe { SDL_RenderClear(renderer) } != 0
        || render_frame(renderer, texture) != 0
    {
        return Err(sdl_error());
    }
    unsafe { SDL_RenderPresent(renderer) };
    unsafe { SDL_Delay(80) };
    Ok(())
}

fn hold_pause_io_progress_visible(
    renderer: *mut SdlRenderer,
    texture: *mut SdlTexture,
    pixels: &mut [u32],
    font: Option<&BitmapFont>,
    pictures: Option<&world::PauseSoundMenuPictures>,
    loading: bool,
    animation_frame: &mut u8,
    visible_until: Instant,
) -> Result<(), Box<dyn Error>> {
    while Instant::now() < visible_until {
        *animation_frame ^= 1;
        present_pause_io_progress(
            renderer,
            texture,
            pixels,
            font,
            pictures,
            loading,
            *animation_frame,
        )?;
    }
    Ok(())
}
const SDL_AUDIO_S16LSB: u16 = 0x8010;

struct SdlAudioDeviceGuard(u32);

impl Drop for SdlAudioDeviceGuard {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe { SDL_CloseAudioDevice(self.0) };
        }
    }
}

struct ScreenMusic {
    device: SdlAudioDeviceGuard,
    mixer: Box<UnsafeCell<AudioMixer>>,
}

impl ScreenMusic {
    fn take_music(&mut self) -> Option<crate::opl2::ImfPlayer> {
        let device = self.device.0;
        if device != 0 {
            unsafe { SDL_LockAudioDevice(device) };
        }
        let music = unsafe { (&mut *self.mixer.get()).music.take() };
        if device != 0 {
            unsafe { SDL_UnlockAudioDevice(device) };
        }
        music
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CampaignState {
    health: u8,
    arsenal: Arsenal,
    score: u32,
    next_extra_life_score: u32,
    kills: u16,
    lives: u8,
    keys_owned: u8,
    last_level_stats: Option<LevelStats>,
    level_ratios: [Option<LevelStats>; 10],
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct LevelStats {
    pub elapsed_seconds: u32,
    pub kills: u16,
    pub total_kills: u16,
    pub secrets: u16,
    pub total_secrets: u16,
    pub treasures: u16,
    pub total_treasures: u16,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct LevelSaveState {
    pub(crate) map_number: usize,
    pub(crate) source_map_signature: u64,
    pub(crate) data_variant: String,
    pub(crate) app_version: String,
    #[serde(default)]
    pub(crate) save_name: String,
    pub(crate) difficulty: Difficulty,
    pub(crate) map: Map,
    pub(crate) doors: DoorSystem,
    pub(crate) item_pickups: Vec<crate::actors::ItemPickup>,
    pub(crate) active_actors: Vec<crate::actors::ActorSpawn>,
    pub(crate) enemies: crate::actors::EnemyRuntime,
    pub(crate) pushwalls: PushWallSystem,
    pub(crate) player: Player,
    pub(crate) campaign: CampaignState,
    pub(crate) weapon_attack: WeaponAttack,
    pub(crate) keys_owned: u8,
    pub(crate) health: u8,
    pub(crate) arsenal: Arsenal,
    pub(crate) score: u32,
    pub(crate) next_extra_life_score: u32,
    pub(crate) kills: u16,
    pub(crate) lives: u8,
    pub(crate) level_kills: u16,
    pub(crate) treasures_found: u16,
    pub(crate) elapsed_seconds: f64,
    pub(crate) simulation_accumulator: f64,
}

fn source_map_signature(map: &Map) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut mix = |word: u16| {
        for byte in word.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    mix(map.number as u16);
    mix(map.width as u16);
    mix(map.height as u16);
    for plane in &map.planes {
        for &tile in plane {
            mix(tile);
        }
    }
    hash
}

fn runtime_save_slot_labels() -> Vec<String> {
    (0..crate::resources::SAVE_SLOT_COUNT)
        .map(|slot| {
            let name = crate::savegame::display_name(&crate::resources::save_slot_path(slot))
                .filter(|name| !name.is_empty());
            match name {
                Some(name) => format!(
                    "SLOT {}: {}",
                    slot + 1,
                    name.chars().take(22).collect::<String>()
                ),
                None => format!("SLOT {}: EMPTY", slot + 1),
            }
        })
        .collect()
}

fn runtime_pause_control_labels(
    capturing: Option<usize>,
    button_mapping: Option<bool>,
    button_capture: Option<(bool, u8)>,
    movement_mapping: bool,
    move_key_capture: Option<usize>,
) -> Vec<String> {
    if let Some(mouse) = button_mapping {
        let settings = crate::settings::button_bindings();
        let (actions, bindings, names): (&[u8], &[u8], &[&str]) = if mouse {
            (
                &MOUSE_BUTTON_ACTIONS,
                &settings.mouse,
                &["LEFT", "MIDDLE", "RIGHT", "BUTTON 4"],
            )
        } else {
            (
                &CONTROLLER_BUTTON_ACTIONS,
                &settings.controller,
                &["A", "LB", "B", "X"],
            )
        };
        let mut options = actions
            .iter()
            .map(|&action| {
                let label = button_action_name(action);
                if button_capture == Some((mouse, action)) {
                    format!("{label}: PRESS BUTTON")
                } else {
                    let assigned = bindings
                        .iter()
                        .position(|&bound_action| bound_action == action)
                        .map_or("UNBOUND", |button| names[button]);
                    format!("{label}: {assigned}")
                }
            })
            .collect::<Vec<_>>();
        options.push("BACK".to_owned());
        return options;
    }
    if movement_mapping {
        let bindings = crate::settings::get_keyboard_move_bindings();
        let mut options = PAUSE_MOVE_LABELS
            .iter()
            .enumerate()
            .map(|(index, label)| {
                if move_key_capture == Some(index) {
                    format!("{label}: PRESS A KEY")
                } else {
                    format!("{label}: {}", scancode_name(bindings.scancodes[index]))
                }
            })
            .collect::<Vec<_>>();
        options.push("BACK".to_owned());
        return options;
    }
    let controls = crate::settings::get_controls();
    let mut options = PAUSE_CONTROL_LABELS
        .iter()
        .enumerate()
        .map(|(index, label)| {
            if capturing == Some(index) {
                format!("{label}: PRESS A KEY")
            } else {
                let keys = controls.scancodes[index];
                let alternate = if keys[1] == 0 {
                    String::new()
                } else {
                    format!(" / {}", scancode_name(keys[1]))
                };
                format!("{label}: {}{alternate}", scancode_name(keys[0]))
            }
        })
        .collect::<Vec<_>>();
    options.push(format!(
        "MOUSE SENSITIVITY: {} < >",
        crate::settings::mouse_sensitivity()
    ));
    options.push(format!(
        "MOUSE INPUT: {}",
        if crate::settings::mouse_input_enabled() {
            "ON"
        } else {
            "OFF"
        }
    ));
    options.push(format!(
        "CONTROLLER INPUT: {}",
        if crate::settings::controller_input_enabled() {
            "ON"
        } else {
            "OFF"
        }
    ));
    options.push("CUSTOMIZE MOUSE BUTTONS".to_owned());
    options.push("CUSTOMIZE CONTROLLER BUTTONS".to_owned());
    options.push("CUSTOMIZE MOVEMENT KEYS".to_owned());
    options.push("RESTORE KEY DEFAULTS".to_owned());
    options.push("BACK".to_owned());
    options
}

fn button_action_name(action: u8) -> &'static str {
    match action {
        crate::settings::BUTTON_ACTION_RUN => "RUN",
        crate::settings::BUTTON_ACTION_STRAFE => "STRAFE",
        crate::settings::BUTTON_ACTION_USE => "USE",
        crate::settings::BUTTON_ACTION_FIRE => "FIRE",
        _ => "UNBOUND",
    }
}

fn runtime_pause_display_options() -> [String; 7] {
    let fullscreen = if crate::settings::fullscreen_enabled() {
        "ON"
    } else {
        "OFF"
    };
    let (width, height) = window_size();
    [
        format!("FULLSCREEN: {fullscreen}"),
        format!("WINDOW SIZE: {width} X {height}"),
        format!("VIEW SIZE: {}", crate::settings::view_size()),
        format!(
            "WORLD AA: {}",
            crate::settings::world_renderer().short_label()
        ),
        format!(
            "RENDER RESOLUTION: {}",
            crate::settings::render_resolution().short_label()
        ),
        format!(
            "SWASTIKA SYMBOLS: {}",
            if crate::settings::show_historical_symbols() {
                "SHOW"
            } else {
                "HIDE"
            }
        ),
        "BACK".to_owned(),
    ]
}

impl LevelSaveState {
    pub(crate) fn is_compatible_with(
        &self,
        source_map: &Map,
        source_doors: &DoorSystem,
        difficulty: Difficulty,
        data_variant: &str,
    ) -> bool {
        let width = self.map.width;
        let height = self.map.height;
        self.map_number == source_map.number
            && self.map.number == source_map.number
            && self.source_map_signature == source_map_signature(source_map)
            && self.data_variant == data_variant
            && self.app_version == env!("CARGO_PKG_VERSION")
            && self.difficulty == difficulty
            && self.map.name == source_map.name
            && self.map.planes.iter().all(|plane| plane.len() == 64 * 64)
            && (1..=64).contains(&width)
            && (1..=64).contains(&height)
            && self.doors.is_valid_for_map(&self.map, source_doors)
            && self.item_pickups.len() <= 64 * 64
            && self
                .item_pickups
                .iter()
                .all(|pickup| pickup.x < width && pickup.y < height)
            && self.active_actors.len() <= 64 * 64
            && self
                .active_actors
                .iter()
                .all(|actor| actor.x < width && actor.y < height)
            && self.enemies.is_valid_for_map(&self.map, difficulty)
            && self.pushwalls.is_valid_for_map(&self.map)
            && self.player.x.is_finite()
            && self.player.y.is_finite()
            && self.player.x >= 0.0
            && self.player.y >= 0.0
            && self.player.x < width as f64
            && self.player.y < height as f64
            && self.player.angle.is_finite()
            && (-19..=19).contains(&self.player.turn_remainder)
            && self.campaign.is_valid()
            && self.weapon_attack.is_valid()
            && self.arsenal.is_valid()
            && self.arsenal.matches(&self.campaign.arsenal)
            && self.health <= 100
            && self.lives <= 9
            && self.keys_owned & !0x0f == 0
            && self.score == self.campaign.score
            && self.next_extra_life_score == self.campaign.next_extra_life_score
            && (self.next_extra_life_score > self.score || self.next_extra_life_score == u32::MAX)
            && self.kills == self.campaign.kills
            && self.lives == self.campaign.lives
            && self.health == self.campaign.health
            && self.keys_owned == self.campaign.keys_owned
            && self.level_kills <= 64 * 64
            && self.treasures_found <= 64 * 64
            && self.elapsed_seconds.is_finite()
            && self.elapsed_seconds >= 0.0
            && self.simulation_accumulator.is_finite()
            && (0.0..1.0 / 70.0).contains(&self.simulation_accumulator)
    }
}

impl Default for CampaignState {
    fn default() -> Self {
        Self {
            health: 100,
            arsenal: Arsenal::default(),
            score: 0,
            next_extra_life_score: 40_000,
            kills: 0,
            lives: 3,
            keys_owned: 0,
            last_level_stats: None,
            level_ratios: [None; 10],
        }
    }
}

impl CampaignState {
    fn is_valid(&self) -> bool {
        self.health <= 100
            && self.lives <= 9
            && self.keys_owned & !0x0f == 0
            && self.arsenal.is_valid()
            && self
                .last_level_stats
                .iter()
                .chain(self.level_ratios.iter().flatten())
                .all(|stats| {
                    stats.kills <= stats.total_kills
                        && stats.secrets <= stats.total_secrets
                        && stats.treasures <= stats.total_treasures
                })
    }

    pub fn clear_keys(&mut self) {
        self.keys_owned = 0;
    }
}

fn award_points(points: u32, score: &mut u32, next_extra_life: &mut u32, lives: &mut u8) -> usize {
    *score = score.saturating_add(points);
    let mut extra_life_sounds = 0usize;
    while *score >= *next_extra_life {
        *next_extra_life = next_extra_life.saturating_add(40_000);
        if *lives < 9 {
            *lives += 1;
        }
        extra_life_sounds = extra_life_sounds.saturating_add(1);
        if *next_extra_life == u32::MAX {
            break;
        }
    }
    extra_life_sounds
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelResult {
    Quit,
    NewGame {
        map_number: usize,
        difficulty: Difficulty,
    },
    Complete,
    Secret,
    Victory,
    GameOver,
    DemoFinished,
    DemoAborted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttractResult {
    Quit,
    SkipRemaining,
    TimedOut,
}

fn run_faded_attract_picture(
    renderer: *mut SdlRenderer,
    texture: *mut SdlTexture,
    pixels: &[u32],
    wait_duration: Duration,
    ack_input: &mut SdlAcknowledgementInput,
) -> Result<AttractResult, Box<dyn Error>> {
    let fade_duration = Duration::from_secs_f64(30.0 / 70.0);
    let screen_started = Instant::now();
    let input_enabled_at = screen_started + fade_duration;
    let deadline = input_enabled_at + wait_duration;
    let mut exit_fade_started = None;
    let mut timed_out = false;
    let mut event = SdlEvent { bytes: [0; 56] };
    let mut frame = pixels.to_vec();
    loop {
        let now = Instant::now();
        while unsafe { SDL_PollEvent(&mut event) } != 0 {
            let event_type = u32::from_ne_bytes(event.bytes[0..4].try_into().unwrap());
            if event_type == SDL_QUIT {
                return Ok(AttractResult::Quit);
            }
            if now >= input_enabled_at && exit_fade_started.is_none() && ack_input.handles(&event) {
                exit_fade_started = Some(now);
            }
        }
        let now = Instant::now();
        if exit_fade_started.is_none() && now >= deadline {
            timed_out = true;
            exit_fade_started = Some(now);
        }
        let brightness = if let Some(started) = exit_fade_started {
            1.0 - now.duration_since(started).as_secs_f64() / fade_duration.as_secs_f64()
        } else {
            now.duration_since(screen_started).as_secs_f64() / fade_duration.as_secs_f64()
        };
        frame.copy_from_slice(pixels);
        fade_frame(&mut frame, brightness);
        if unsafe {
            SDL_UpdateTexture(
                texture,
                std::ptr::null(),
                frame.as_ptr(),
                (WIDTH * 4) as i32,
            )
        } != 0
            || unsafe { SDL_RenderClear(renderer) } != 0
            || render_frame(renderer, texture) != 0
        {
            return Err(sdl_error());
        }
        unsafe { SDL_RenderPresent(renderer) };
        if exit_fade_started.is_some_and(|started| now.duration_since(started) >= fade_duration) {
            break;
        }
        unsafe { SDL_Delay(16) };
    }
    Ok(if timed_out {
        AttractResult::TimedOut
    } else {
        AttractResult::SkipRemaining
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoiceMenuResult {
    Selected(usize),
    KeyPressed(u16),
    Cancelled,
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonCaptureResult {
    Captured(usize),
    Cancelled,
    Quit,
}

pub fn scancode_name(scancode: u16) -> String {
    let name = unsafe { SDL_GetScancodeName(scancode as i32) };
    if name.is_null() {
        format!("Key {scancode}")
    } else {
        let name = unsafe { std::ffi::CStr::from_ptr(name) }.to_string_lossy();
        if name.is_empty() || name == "Unknown" {
            format!("Key {scancode}")
        } else {
            name.replacen("Left ", "L", 1).replacen("Right ", "R", 1)
        }
    }
}

/// Loads a save from the startup selector while keeping the original loading
/// card visible. The short hold makes the disk animation visible even for the
/// port's much smaller JSON save files.
pub fn load_save_with_progress<T: serde::de::DeserializeOwned>(
    path: &std::path::Path,
    font: &BitmapFont,
    pictures: &world::PauseSoundMenuPictures,
    selected_slot: usize,
) -> Result<T, Box<dyn Error>> {
    unsafe {
        if initialize_sdl(SDL_INIT_VIDEO) != 0 {
            return Err(sdl_error());
        }
        let title = CString::new("Wolfenstein 3D — Loading")?;
        let window = SDL_CreateWindow(
            title.as_ptr(),
            SDL_WINDOWPOS_CENTERED,
            SDL_WINDOWPOS_CENTERED,
            window_width(),
            window_height(),
            window_creation_flags(),
        );
        if window.is_null() {
            return Err(sdl_error());
        }
        let mut renderer = SDL_CreateRenderer(
            window,
            -1,
            SDL_RENDERER_ACCELERATED | SDL_RENDERER_PRESENTVSYNC,
        );
        if renderer.is_null() {
            renderer = SDL_CreateRenderer(window, -1, 0);
        }
        if renderer.is_null() {
            SDL_DestroyWindow(window);
            return Err(sdl_error());
        }
        let texture = SDL_CreateTexture(
            renderer,
            SDL_PIXELFORMAT_ARGB8888,
            SDL_TEXTUREACCESS_STREAMING,
            WIDTH as i32,
            HEIGHT as i32,
        );
        if texture.is_null() {
            SDL_DestroyRenderer(renderer);
            SDL_DestroyWindow(window);
            return Err(sdl_error());
        }

        let result = (|| {
            let mut pixels = vec![0xff000000; WIDTH * HEIGHT];
            let labels = runtime_save_slot_labels();
            world::render_pause_slot_menu(
                &mut pixels,
                font,
                Some(pictures),
                true,
                selected_slot,
                &labels,
            );
            let mut animation_frame = 0;
            present_pause_io_progress(
                renderer,
                texture,
                &mut pixels,
                Some(font),
                Some(pictures),
                true,
                animation_frame,
            )?;
            let minimum_visible_until = Instant::now() + Duration::from_millis(350);
            let load_result = crate::savegame::load_with_progress(path, || {
                animation_frame ^= 1;
                present_pause_io_progress(
                    renderer,
                    texture,
                    &mut pixels,
                    Some(font),
                    Some(pictures),
                    true,
                    animation_frame,
                )
            });
            while Instant::now() < minimum_visible_until {
                animation_frame ^= 1;
                present_pause_io_progress(
                    renderer,
                    texture,
                    &mut pixels,
                    Some(font),
                    Some(pictures),
                    true,
                    animation_frame,
                )?;
            }
            load_result
        })();

        SDL_DestroyTexture(texture);
        SDL_DestroyRenderer(renderer);
        SDL_DestroyWindow(window);
        result
    }
}

pub fn run_title_screen(
    title_picture: &crate::vga::Picture,
    intro_music: IntroMusic,
) -> Result<(AttractResult, IntroMusic), Box<dyn Error>> {
    unsafe {
        if initialize_sdl(SDL_INIT_VIDEO) != 0 {
            return Err(sdl_error());
        }
        let mut ack_input = SdlAcknowledgementInput::new();
        let screen_music = start_screen_music_with_state(intro_music);
        let title = CString::new("Wolfenstein 3D")?;
        let window = SDL_CreateWindow(
            title.as_ptr(),
            SDL_WINDOWPOS_CENTERED,
            SDL_WINDOWPOS_CENTERED,
            window_width(),
            window_height(),
            window_creation_flags(),
        );
        if window.is_null() {
            return Err(sdl_error());
        }
        let mut renderer = SDL_CreateRenderer(
            window,
            -1,
            SDL_RENDERER_ACCELERATED | SDL_RENDERER_PRESENTVSYNC,
        );
        if renderer.is_null() {
            renderer = SDL_CreateRenderer(window, -1, 0);
        }
        if renderer.is_null() {
            SDL_DestroyWindow(window);
            return Err(sdl_error());
        }
        let texture = SDL_CreateTexture(
            renderer,
            SDL_PIXELFORMAT_ARGB8888,
            SDL_TEXTUREACCESS_STREAMING,
            WIDTH as i32,
            HEIGHT as i32,
        );
        if texture.is_null() {
            SDL_DestroyRenderer(renderer);
            SDL_DestroyWindow(window);
            return Err(sdl_error());
        }
        let mut pixels = vec![0u32; WIDTH * HEIGHT];
        world::render_title_screen(&mut pixels, title_picture);
        let result = run_faded_attract_picture(
            renderer,
            texture,
            &pixels,
            Duration::from_secs(15),
            &mut ack_input,
        );

        SDL_DestroyTexture(texture);
        SDL_DestroyRenderer(renderer);
        SDL_DestroyWindow(window);
        let result = result?;
        let intro_music = take_intro_music(screen_music.device.0, &screen_music.mixer);
        return Ok((result, intro_music));
    }
}

/// Displays the attract-mode credits page for ten seconds. Any key skips the
/// remaining attract screens; SDL_QUIT exits the application.
pub fn run_credits_screen(
    picture: &crate::vga::Picture,
    intro_music: IntroMusic,
) -> Result<(AttractResult, IntroMusic), Box<dyn Error>> {
    unsafe {
        if initialize_sdl(SDL_INIT_VIDEO) != 0 {
            return Err(sdl_error());
        }
        let mut ack_input = SdlAcknowledgementInput::new();
        let screen_music = start_screen_music_with_state(intro_music);
        let title = CString::new("Wolfenstein 3D — Credits")?;
        let window = SDL_CreateWindow(
            title.as_ptr(),
            SDL_WINDOWPOS_CENTERED,
            SDL_WINDOWPOS_CENTERED,
            window_width(),
            window_height(),
            window_creation_flags(),
        );
        if window.is_null() {
            return Err(sdl_error());
        }
        let mut renderer = SDL_CreateRenderer(
            window,
            -1,
            SDL_RENDERER_ACCELERATED | SDL_RENDERER_PRESENTVSYNC,
        );
        if renderer.is_null() {
            renderer = SDL_CreateRenderer(window, -1, 0);
        }
        if renderer.is_null() {
            SDL_DestroyWindow(window);
            return Err(sdl_error());
        }
        let texture = SDL_CreateTexture(
            renderer,
            SDL_PIXELFORMAT_ARGB8888,
            SDL_TEXTUREACCESS_STREAMING,
            WIDTH as i32,
            HEIGHT as i32,
        );
        if texture.is_null() {
            SDL_DestroyRenderer(renderer);
            SDL_DestroyWindow(window);
            return Err(sdl_error());
        }
        let mut pixels = vec![0u32; WIDTH * HEIGHT];
        pixels.fill(0xff000000);
        world::draw_picture(picture, 0, 0, &mut pixels);
        let result = run_faded_attract_picture(
            renderer,
            texture,
            &pixels,
            Duration::from_secs(10),
            &mut ack_input,
        );
        SDL_DestroyTexture(texture);
        SDL_DestroyRenderer(renderer);
        SDL_DestroyWindow(window);
        let result = result?;
        let intro_music = take_intro_music(screen_music.device.0, &screen_music.mixer);
        return Ok((result, intro_music));
    }
}

pub fn run_attract_high_scores(
    font: &BitmapFont,
    audio: &AudioArchive,
    graphics: &crate::vga::VgaArchive,
    extension: &str,
    intro_music: IntroMusic,
) -> Result<AttractResult, Box<dyn Error>> {
    let mut table = crate::highscores::Table::load(&resources::high_score_path());
    run_high_score_screen(
        &mut table,
        None,
        font,
        audio,
        graphics,
        extension,
        true,
        Some(intro_music),
    )
}

pub fn run_high_scores_menu(
    font: &BitmapFont,
    audio: &AudioArchive,
    graphics: &crate::vga::VgaArchive,
    extension: &str,
) -> Result<bool, Box<dyn Error>> {
    let path = resources::high_score_path();
    let mut table = crate::highscores::Table::load(&path);
    let result = run_high_score_screen(
        &mut table, None, font, audio, graphics, extension, false, None,
    )?;
    table.save(&path)?;
    Ok(result != AttractResult::Quit)
}

pub fn run_startup_quit_confirmation(
    background: &Picture,
    font: &BitmapFont,
    menu_music: Option<crate::opl2::ImfPlayer>,
    audio: &AudioArchive,
) -> Result<(bool, Option<crate::opl2::ImfPlayer>), Box<dyn Error>> {
    let message_index = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as usize
        % QUIT_CONFIRMATION_MESSAGES.len();
    let options = ["No".to_owned(), "Yes".to_owned()];
    let (selection, menu_music) = run_choice_menu(
        QUIT_CONFIRMATION_MESSAGES[message_index],
        &options,
        world::ChoiceMenuLayout::Main,
        background,
        &[],
        font,
        false,
        menu_music,
        audio,
    )?;
    let confirmed = matches!(
        selection,
        ChoiceMenuResult::Selected(1) | ChoiceMenuResult::Quit
    );
    Ok((confirmed, menu_music))
}

pub fn run_pg13_screen(
    picture: &Picture,
    intro_music: IntroMusic,
) -> Result<(bool, IntroMusic), Box<dyn Error>> {
    let mut quit = false;
    unsafe {
        if initialize_sdl(SDL_INIT_VIDEO) != 0 {
            return Err(sdl_error());
        }
        let mut ack_input = SdlAcknowledgementInput::new();
        let screen_music = start_screen_music_with_state(intro_music);
        let title = CString::new("Wolfenstein 3D")?;
        let window = SDL_CreateWindow(
            title.as_ptr(),
            SDL_WINDOWPOS_CENTERED,
            SDL_WINDOWPOS_CENTERED,
            window_width(),
            window_height(),
            window_creation_flags(),
        );
        if window.is_null() {
            return Err(sdl_error());
        }
        let mut renderer = SDL_CreateRenderer(
            window,
            -1,
            SDL_RENDERER_ACCELERATED | SDL_RENDERER_PRESENTVSYNC,
        );
        if renderer.is_null() {
            renderer = SDL_CreateRenderer(window, -1, 0);
        }
        if renderer.is_null() {
            SDL_DestroyWindow(window);
            return Err(sdl_error());
        }
        let texture = SDL_CreateTexture(
            renderer,
            SDL_PIXELFORMAT_ARGB8888,
            SDL_TEXTUREACCESS_STREAMING,
            WIDTH as i32,
            HEIGHT as i32,
        );
        if texture.is_null() {
            SDL_DestroyRenderer(renderer);
            SDL_DestroyWindow(window);
            return Err(sdl_error());
        }

        let fade_in = Duration::from_millis(500);
        let wait_duration = Duration::from_secs(7);
        let fade_out = Duration::from_millis(500);
        let started = Instant::now();
        let mut wait_started = None;
        let mut exit_started = None;
        let mut event = SdlEvent { bytes: [0; 56] };
        let mut pixels = vec![0u32; WIDTH * HEIGHT];
        loop {
            while SDL_PollEvent(&mut event) != 0 {
                let event_type = u32::from_ne_bytes(event.bytes[0..4].try_into().unwrap());
                if event_type == SDL_QUIT {
                    quit = true;
                    exit_started.get_or_insert_with(Instant::now);
                } else if wait_started.is_some() && ack_input.handles(&event) {
                    exit_started.get_or_insert_with(Instant::now);
                }
            }
            let now = Instant::now();
            if wait_started.is_none() && now.duration_since(started) >= fade_in {
                wait_started = Some(now);
            }
            if exit_started.is_none()
                && wait_started.is_some_and(|wait| now.duration_since(wait) >= wait_duration)
            {
                exit_started = Some(now);
            }
            let brightness = if let Some(exit) = exit_started {
                1.0 - (now.duration_since(exit).as_secs_f64() / fade_out.as_secs_f64())
                    .clamp(0.0, 1.0)
            } else if wait_started.is_some() {
                1.0
            } else {
                (now.duration_since(started).as_secs_f64() / fade_in.as_secs_f64()).clamp(0.0, 1.0)
            };
            world::render_pg13_screen(&mut pixels, picture, brightness);
            if SDL_UpdateTexture(
                texture,
                std::ptr::null(),
                pixels.as_ptr(),
                (WIDTH * 4) as i32,
            ) != 0
                || SDL_RenderClear(renderer) != 0
                || render_frame(renderer, texture) != 0
            {
                let error = sdl_error();
                SDL_DestroyTexture(texture);
                SDL_DestroyRenderer(renderer);
                SDL_DestroyWindow(window);
                return Err(error);
            }
            SDL_RenderPresent(renderer);
            if exit_started.is_some_and(|exit| now.duration_since(exit) >= fade_out) {
                break;
            }
            SDL_Delay(16);
        }
        SDL_DestroyTexture(texture);
        SDL_DestroyRenderer(renderer);
        SDL_DestroyWindow(window);
        let intro_music = take_intro_music(screen_music.device.0, &screen_music.mixer);
        return Ok((!quit, intro_music));
    }
}

pub fn run_signon_screen(font: &BitmapFont) -> Result<bool, Box<dyn Error>> {
    unsafe {
        if initialize_sdl(SDL_INIT_VIDEO) != 0 {
            return Err(sdl_error());
        }
        let mut ack_input = SdlAcknowledgementInput::new();
        let title = CString::new("Wolfenstein 3D")?;
        let window = SDL_CreateWindow(
            title.as_ptr(),
            SDL_WINDOWPOS_CENTERED,
            SDL_WINDOWPOS_CENTERED,
            window_width(),
            window_height(),
            window_creation_flags(),
        );
        if window.is_null() {
            return Err(sdl_error());
        }
        let mut renderer = SDL_CreateRenderer(
            window,
            -1,
            SDL_RENDERER_ACCELERATED | SDL_RENDERER_PRESENTVSYNC,
        );
        if renderer.is_null() {
            renderer = SDL_CreateRenderer(window, -1, 0);
        }
        if renderer.is_null() {
            SDL_DestroyWindow(window);
            return Err(sdl_error());
        }
        let texture = SDL_CreateTexture(
            renderer,
            SDL_PIXELFORMAT_ARGB8888,
            SDL_TEXTUREACCESS_STREAMING,
            WIDTH as i32,
            HEIGHT as i32,
        );
        if texture.is_null() {
            SDL_DestroyRenderer(renderer);
            SDL_DestroyWindow(window);
            return Err(sdl_error());
        }

        let mut pixels = vec![0u32; WIDTH * HEIGHT];
        let mut event = SdlEvent { bytes: [0; 56] };
        let mut working_started = None;
        let result = (|| -> Result<bool, Box<dyn Error>> {
            loop {
                while SDL_PollEvent(&mut event) != 0 {
                    let event_type = u32::from_ne_bytes(event.bytes[0..4].try_into().unwrap());
                    if event_type == SDL_QUIT {
                        return Ok(false);
                    }
                    if working_started.is_none() && ack_input.handles(&event) {
                        working_started = Some(Instant::now());
                    }
                }

                let now = Instant::now();
                let message = if working_started.is_some() {
                    "Working..."
                } else {
                    "Press a key"
                };
                world::render_signon_screen(&mut pixels, crate::signon::IMAGE, font, message);
                if SDL_UpdateTexture(
                    texture,
                    std::ptr::null(),
                    pixels.as_ptr(),
                    (WIDTH * 4) as i32,
                ) != 0
                    || SDL_RenderClear(renderer) != 0
                    || render_frame(renderer, texture) != 0
                {
                    return Err(sdl_error());
                }
                SDL_RenderPresent(renderer);

                if working_started.is_some_and(|started| {
                    now.duration_since(started) >= Duration::from_millis(900)
                }) {
                    return Ok(true);
                }
                SDL_Delay(16);
            }
        })();

        SDL_DestroyTexture(texture);
        SDL_DestroyRenderer(renderer);
        SDL_DestroyWindow(window);
        result
    }
}

pub fn run_choice_menu(
    heading: &str,
    options: &[String],
    layout: world::ChoiceMenuLayout,
    background: &Picture,
    illustrations: &[Picture],
    font: &BitmapFont,
    capture_scancode: bool,
    menu_music: Option<crate::opl2::ImfPlayer>,
    audio: &AudioArchive,
) -> Result<(ChoiceMenuResult, Option<crate::opl2::ImfPlayer>), Box<dyn Error>> {
    run_choice_menu_inner(
        heading,
        options,
        layout,
        background,
        illustrations,
        font,
        capture_scancode,
        None,
        menu_music,
        Some(audio),
        None,
    )
}

pub fn run_button_capture_menu(
    heading: &str,
    prompt: &str,
    mouse: bool,
    background: &Picture,
    font: &BitmapFont,
    menu_music: Option<crate::opl2::ImfPlayer>,
    audio: &AudioArchive,
) -> Result<(ButtonCaptureResult, Option<crate::opl2::ImfPlayer>), Box<dyn Error>> {
    let (result, menu_music) = run_choice_menu_inner(
        heading,
        &[prompt.to_owned()],
        world::ChoiceMenuLayout::Controls,
        background,
        &[],
        font,
        false,
        Some(mouse),
        menu_music,
        Some(audio),
        None,
    )?;
    let result = match result {
        ChoiceMenuResult::Selected(button) => ButtonCaptureResult::Captured(button),
        ChoiceMenuResult::Quit => ButtonCaptureResult::Quit,
        ChoiceMenuResult::Cancelled | ChoiceMenuResult::KeyPressed(_) => {
            ButtonCaptureResult::Cancelled
        }
    };
    Ok((result, menu_music))
}

pub fn run_jukebox_menu(
    options: &[String],
    music_tracks: &[usize],
    background: &Picture,
    font: &BitmapFont,
    audio: &AudioArchive,
) -> Result<ChoiceMenuResult, Box<dyn Error>> {
    let (result, _music) = run_choice_menu_inner(
        "Robert's Jukebox",
        options,
        world::ChoiceMenuLayout::Jukebox,
        background,
        &[],
        font,
        false,
        None,
        None,
        Some(audio),
        Some((audio, music_tracks)),
    )?;
    Ok(result)
}

fn run_choice_menu_inner(
    heading: &str,
    options: &[String],
    layout: world::ChoiceMenuLayout,
    background: &Picture,
    illustrations: &[Picture],
    font: &BitmapFont,
    capture_scancode: bool,
    capture_button: Option<bool>,
    menu_music: Option<crate::opl2::ImfPlayer>,
    menu_audio: Option<&AudioArchive>,
    jukebox_tracks: Option<(&AudioArchive, &[usize])>,
) -> Result<(ChoiceMenuResult, Option<crate::opl2::ImfPlayer>), Box<dyn Error>> {
    if options.is_empty() {
        return Err("selection menu needs at least one option".into());
    }
    let mut result = ChoiceMenuResult::Cancelled;
    unsafe {
        if initialize_sdl(SDL_INIT_VIDEO) != 0 {
            return Err(sdl_error());
        }
        let mut controller_input = SdlAcknowledgementInput::new();
        let screen_music = start_screen_music(menu_music);
        let title = CString::new(heading)?;
        let window = SDL_CreateWindow(
            title.as_ptr(),
            SDL_WINDOWPOS_CENTERED,
            SDL_WINDOWPOS_CENTERED,
            window_width(),
            window_height(),
            window_creation_flags(),
        );
        if window.is_null() {
            return Err(sdl_error());
        }
        let mut renderer = SDL_CreateRenderer(
            window,
            -1,
            SDL_RENDERER_ACCELERATED | SDL_RENDERER_PRESENTVSYNC,
        );
        if renderer.is_null() {
            renderer = SDL_CreateRenderer(window, -1, 0);
        }
        if renderer.is_null() {
            SDL_DestroyWindow(window);
            return Err(sdl_error());
        }
        let texture = SDL_CreateTexture(
            renderer,
            SDL_PIXELFORMAT_ARGB8888,
            SDL_TEXTUREACCESS_STREAMING,
            WIDTH as i32,
            HEIGHT as i32,
        );
        if texture.is_null() {
            SDL_DestroyRenderer(renderer);
            SDL_DestroyWindow(window);
            return Err(sdl_error());
        }
        let mut pixels = vec![0u32; WIDTH * HEIGHT];
        let mut selected = if layout == world::ChoiceMenuLayout::MouseSensitivity {
            usize::from(crate::settings::mouse_sensitivity())
        } else {
            0
        };
        let mut event = SdlEvent { bytes: [0; 56] };
        let mut done = false;
        let mut navigation_allowed = Instant::now();
        let mut navigation_repeat_scancode = None;
        let mut navigation_repeat_due = None;
        let navigation_repeat_interval = if layout == world::ChoiceMenuLayout::MouseSensitivity {
            Duration::from_secs_f64(10.0 / 70.0)
        } else {
            Duration::from_secs_f64(8.0 / 70.0)
        };
        let fade_duration = Duration::from_secs_f64(10.0 / 70.0);
        let screen_started = Instant::now();
        let mut exit_fade_started = None;
        let mut move_sound_due = None;
        while !done {
            let now = Instant::now();
            let cursor_elapsed = now.saturating_duration_since(screen_started + fade_duration);
            world::render_choice_menu(
                &mut pixels,
                font,
                heading,
                options,
                layout,
                background,
                illustrations,
                selected,
                dos_menu_cursor_frame(cursor_elapsed),
            );
            let brightness = if let Some(started) = exit_fade_started {
                1.0 - (now.duration_since(started).as_secs_f64() / fade_duration.as_secs_f64())
            } else {
                screen_started.elapsed().as_secs_f64() / fade_duration.as_secs_f64()
            };
            fade_frame(&mut pixels, brightness);
            if SDL_UpdateTexture(
                texture,
                std::ptr::null(),
                pixels.as_ptr(),
                (WIDTH * 4) as i32,
            ) != 0
                || SDL_RenderClear(renderer) != 0
                || render_frame(renderer, texture) != 0
            {
                let error = sdl_error();
                SDL_DestroyTexture(texture);
                SDL_DestroyRenderer(renderer);
                SDL_DestroyWindow(window);
                return Err(error);
            }
            SDL_RenderPresent(renderer);
            while SDL_PollEvent(&mut event) != 0 {
                let event_type = u32::from_ne_bytes(event.bytes[0..4].try_into().unwrap());
                controller_input.handles(&event);
                match event_type {
                    SDL_QUIT => {
                        result = ChoiceMenuResult::Quit;
                        done = true;
                    }
                    SDL_MOUSEMOTION
                        if crate::settings::mouse_input_enabled()
                            && !capture_scancode
                            && capture_button.is_none()
                            && exit_fade_started.is_none() =>
                    {
                        let x = i32::from_ne_bytes(event.bytes[20..24].try_into().unwrap());
                        let y = i32::from_ne_bytes(event.bytes[24..28].try_into().unwrap());
                        if let Some(index) = menu_pointer_selection(
                            window,
                            renderer,
                            layout,
                            options.len(),
                            selected,
                            x,
                            y,
                        ) {
                            if index != selected {
                                selected = index;
                                queue_mixer_pc_sound(
                                    menu_audio,
                                    screen_music.device.0,
                                    &screen_music.mixer,
                                    5,
                                );
                                if layout != world::ChoiceMenuLayout::MouseSensitivity {
                                    move_sound_due =
                                        Some(Instant::now() + Duration::from_secs_f64(8.0 / 70.0));
                                }
                            }
                        }
                    }
                    SDL_MOUSEBUTTONDOWN
                        if capture_button == Some(true) && exit_fade_started.is_none() =>
                    {
                        let button = match event.bytes[16] {
                            1 => Some(0),
                            2 => Some(1),
                            3 => Some(2),
                            _ => None,
                        };
                        if let Some(button) = button {
                            queue_mixer_pc_sound(
                                menu_audio,
                                screen_music.device.0,
                                &screen_music.mixer,
                                32,
                            );
                            result = ChoiceMenuResult::Selected(button);
                            exit_fade_started = Some(Instant::now());
                        }
                    }
                    SDL_MOUSEBUTTONDOWN
                        if !capture_scancode
                            && capture_button.is_none()
                            && crate::settings::mouse_input_enabled()
                            && exit_fade_started.is_none()
                            && event.bytes[16] == 3 =>
                    {
                        queue_mixer_pc_sound(
                            menu_audio,
                            screen_music.device.0,
                            &screen_music.mixer,
                            39,
                        );
                        result = ChoiceMenuResult::Cancelled;
                        exit_fade_started = Some(Instant::now());
                    }
                    SDL_MOUSEBUTTONDOWN
                        if !capture_scancode
                            && capture_button.is_none()
                            && crate::settings::mouse_input_enabled()
                            && exit_fade_started.is_none()
                            && event.bytes[16] == 1 =>
                    {
                        let x = i32::from_ne_bytes(event.bytes[20..24].try_into().unwrap());
                        let y = i32::from_ne_bytes(event.bytes[24..28].try_into().unwrap());
                        if let Some(index) = menu_pointer_selection(
                            window,
                            renderer,
                            layout,
                            options.len(),
                            selected,
                            x,
                            y,
                        ) {
                            if index != selected {
                                selected = index;
                                queue_mixer_pc_sound(
                                    menu_audio,
                                    screen_music.device.0,
                                    &screen_music.mixer,
                                    5,
                                );
                                if layout != world::ChoiceMenuLayout::MouseSensitivity {
                                    move_sound_due =
                                        Some(Instant::now() + Duration::from_secs_f64(8.0 / 70.0));
                                }
                            }
                            if let Some((audio, tracks)) = jukebox_tracks {
                                if let Some(track) = tracks.get(selected) {
                                    let music = audio.music_player_for_track(*track);
                                    replace_mixer_music(
                                        screen_music.device.0,
                                        &screen_music.mixer,
                                        music,
                                    );
                                }
                            } else {
                                queue_mixer_pc_sound(
                                    menu_audio,
                                    screen_music.device.0,
                                    &screen_music.mixer,
                                    32,
                                );
                                result = ChoiceMenuResult::Selected(selected);
                                exit_fade_started = Some(Instant::now());
                            }
                        }
                    }
                    SDL_KEYDOWN => {
                        if exit_fade_started.is_some() {
                            continue;
                        }
                        let scancode = i32::from_ne_bytes(event.bytes[16..20].try_into().unwrap());
                        if !capture_scancode
                            && capture_button.is_none()
                            && [
                                SDL_SCANCODE_RIGHT,
                                SDL_SCANCODE_LEFT,
                                SDL_SCANCODE_DOWN as i32,
                                SDL_SCANCODE_UP as i32,
                            ]
                            .contains(&scancode)
                        {
                            if event.bytes[13] == 1 {
                                continue;
                            }
                            navigation_repeat_scancode = Some(scancode);
                            navigation_repeat_due =
                                Some(Instant::now() + navigation_repeat_interval);
                        }
                        let previous_selection = selected;
                        if capture_button.is_some() {
                            if scancode == SDL_SCANCODE_ESCAPE as i32 {
                                result = ChoiceMenuResult::Cancelled;
                                exit_fade_started = Some(Instant::now());
                            } else {
                                continue;
                            }
                        } else if capture_scancode {
                            if event.bytes[13] != 0 {
                                continue;
                            } else if scancode == SDL_SCANCODE_ESCAPE as i32 {
                                result = ChoiceMenuResult::Cancelled;
                            } else if (58..=67).contains(&scancode) {
                                result = ChoiceMenuResult::Cancelled;
                            } else if (0..512).contains(&scancode) {
                                result = ChoiceMenuResult::KeyPressed(scancode as u16);
                            }
                            exit_fade_started = Some(Instant::now());
                        } else {
                            match scancode {
                                80 | 82
                                    if layout == world::ChoiceMenuLayout::MouseSensitivity
                                        && Instant::now() >= navigation_allowed =>
                                {
                                    selected = selected.saturating_sub(1);
                                    navigation_allowed =
                                        Instant::now() + Duration::from_secs_f64(10.0 / 70.0);
                                }
                                79 | 81
                                    if layout == world::ChoiceMenuLayout::MouseSensitivity
                                        && Instant::now() >= navigation_allowed =>
                                {
                                    selected = (selected + 1).min(options.len() - 1);
                                    navigation_allowed =
                                        Instant::now() + Duration::from_secs_f64(10.0 / 70.0);
                                }
                                81 if Instant::now() >= navigation_allowed => {
                                    selected = (selected + 1) % options.len();
                                    navigation_allowed =
                                        Instant::now() + Duration::from_secs_f64(8.0 / 70.0);
                                }
                                82 if Instant::now() >= navigation_allowed => {
                                    selected = (selected + options.len() - 1) % options.len();
                                    navigation_allowed =
                                        Instant::now() + Duration::from_secs_f64(8.0 / 70.0);
                                }
                                SDL_SCANCODE_RETURN => {
                                    if let Some((audio, tracks)) = jukebox_tracks {
                                        if let Some(track) = tracks.get(selected) {
                                            let music = audio.music_player_for_track(*track);
                                            if screen_music.device.0 != 0 {
                                                SDL_LockAudioDevice(screen_music.device.0);
                                            }
                                            (&mut *screen_music.mixer.get()).music = music;
                                            if screen_music.device.0 != 0 {
                                                SDL_UnlockAudioDevice(screen_music.device.0);
                                            }
                                        }
                                    } else {
                                        queue_mixer_pc_sound(
                                            menu_audio,
                                            screen_music.device.0,
                                            &screen_music.mixer,
                                            32,
                                        );
                                        result = ChoiceMenuResult::Selected(selected);
                                        exit_fade_started = Some(Instant::now());
                                    }
                                }
                                41 => {
                                    queue_mixer_pc_sound(
                                        menu_audio,
                                        screen_music.device.0,
                                        &screen_music.mixer,
                                        39,
                                    );
                                    exit_fade_started = Some(Instant::now());
                                }
                                _ => {
                                    // DOS HandleMenu searches LastASCII from the
                                    // item after the current selection, then wraps.
                                    let keycode =
                                        i32::from_ne_bytes(event.bytes[20..24].try_into().unwrap());
                                    let modifiers =
                                        u16::from_ne_bytes(event.bytes[24..26].try_into().unwrap());
                                    if modifiers & 0x03c0 == 0
                                        && ((b'a' as i32..=b'z' as i32).contains(&keycode)
                                            || (b'A' as i32..=b'Z' as i32).contains(&keycode))
                                    {
                                        let key = (keycode as u8).to_ascii_uppercase();
                                        selected = (1..=options.len())
                                            .map(|step| (selected + step) % options.len())
                                            .find(|&index| {
                                                options[index].as_bytes().first().is_some_and(
                                                    |first| first.to_ascii_uppercase() == key,
                                                )
                                            })
                                            .unwrap_or(selected);
                                    }
                                }
                            }
                        }
                        if !capture_scancode && selected != previous_selection {
                            if layout == world::ChoiceMenuLayout::MouseSensitivity {
                                queue_mixer_pc_sound(
                                    menu_audio,
                                    screen_music.device.0,
                                    &screen_music.mixer,
                                    5,
                                );
                                move_sound_due = None;
                            } else {
                                let has_half_step = (scancode == 81 && previous_selection > 0)
                                    || (scancode == 82 && previous_selection + 1 < options.len());
                                if has_half_step {
                                    queue_mixer_pc_sound(
                                        menu_audio,
                                        screen_music.device.0,
                                        &screen_music.mixer,
                                        5,
                                    );
                                    move_sound_due =
                                        Some(Instant::now() + Duration::from_secs_f64(8.0 / 70.0));
                                } else {
                                    queue_mixer_pc_sound(
                                        menu_audio,
                                        screen_music.device.0,
                                        &screen_music.mixer,
                                        4,
                                    );
                                }
                            }
                        }
                    }
                    SDL_KEYUP => {
                        let scancode = i32::from_ne_bytes(event.bytes[16..20].try_into().unwrap());
                        if scancode == SDL_SCANCODE_UP as i32
                            || scancode == SDL_SCANCODE_DOWN as i32
                            || scancode == SDL_SCANCODE_LEFT
                            || scancode == SDL_SCANCODE_RIGHT
                        {
                            navigation_allowed = Instant::now();
                            if navigation_repeat_scancode == Some(scancode) {
                                navigation_repeat_scancode = None;
                                navigation_repeat_due = None;
                            }
                        }
                    }
                    SDL_CONTROLLERBUTTONDOWN
                        if capture_button == Some(false) && exit_fade_started.is_none() =>
                    {
                        if let Some(button) = controller_gameplay_button_slot(event.bytes[12]) {
                            queue_mixer_pc_sound(
                                menu_audio,
                                screen_music.device.0,
                                &screen_music.mixer,
                                32,
                            );
                            result = ChoiceMenuResult::Selected(button);
                            exit_fade_started = Some(Instant::now());
                        }
                    }
                    SDL_CONTROLLERBUTTONDOWN
                        if crate::settings::controller_input_enabled()
                            && !capture_scancode
                            && capture_button.is_none()
                            && exit_fade_started.is_none() =>
                    {
                        if let Some(scancode) = controller_menu_scancode(event.bytes[12]) {
                            push_keydown_scancode(scancode);
                        }
                    }
                    _ => {}
                }
                if done {
                    break;
                }
            }
            if let (Some(scancode), Some(due)) = (navigation_repeat_scancode, navigation_repeat_due)
            {
                if Instant::now() >= due {
                    let mut key_count = 0;
                    let keys = unsafe { SDL_GetKeyboardState(&mut key_count) };
                    let is_down = !keys.is_null()
                        && scancode >= 0
                        && (scancode as usize) < key_count.max(0) as usize
                        && unsafe { *keys.add(scancode as usize) != 0 };
                    if is_down {
                        unsafe { push_repeated_keydown_scancode(scancode) };
                        navigation_repeat_due = Some(Instant::now() + navigation_repeat_interval);
                    } else {
                        navigation_repeat_scancode = None;
                        navigation_repeat_due = None;
                    }
                }
            }
            if move_sound_due.is_some_and(|due| Instant::now() >= due) {
                queue_mixer_pc_sound(menu_audio, screen_music.device.0, &screen_music.mixer, 4);
                move_sound_due = None;
            }
            if exit_fade_started.is_some_and(|started| started.elapsed() >= fade_duration) {
                done = true;
            }
            if !done {
                SDL_Delay(16);
            }
        }
        SDL_DestroyTexture(texture);
        SDL_DestroyRenderer(renderer);
        SDL_DestroyWindow(window);
        let menu_music = take_screen_music(screen_music.device.0, &screen_music.mixer);
        return Ok((result, menu_music));
    }
}

pub fn run_demo() -> Result<(), Box<dyn Error>> {
    let mut map = Map::demo();
    let doors = DoorSystem::default();
    let mut campaign = CampaignState::default();
    run(
        &mut map,
        None,
        doors,
        Vec::new(),
        None,
        &[],
        &SpriteAtlas::default(),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        &[],
        Player::default(),
        "Wolfenstein 3D — Rust development map",
        Difficulty::Medium,
        "DEMO",
        &mut campaign,
        None,
        0,
        None,
        None,
    )?;
    Ok(())
}

pub fn run_level(
    source_map: &Map,
    textures: &Vswap,
    doors: DoorSystem,
    item_pickups: Vec<ItemPickup>,
    audio: &AudioArchive,
    statics: &[crate::actors::ActorSpawn],
    sprites: &SpriteAtlas,
    font: &BitmapFont,
    status_bar: &crate::vga::Picture,
    preload_picture: &crate::vga::Picture,
    hud_pictures: &world::HudPictures,
    pause_sound_pictures: &world::PauseSoundMenuPictures,
    graphics: &crate::vga::VgaArchive,
    last_level_frame: &mut [u32],
    weapon_pictures: &[crate::vga::Picture],
    actor_count: usize,
    difficulty: Difficulty,
    data_variant: &str,
    campaign: &mut CampaignState,
    initial_save: Option<LevelSaveState>,
    initial_save_slot: usize,
    help_articles: Option<&HelpArticles>,
    demo_playback: Option<crate::demo::DemoPlayback>,
) -> Result<LevelResult, Box<dyn Error>> {
    let mut map = source_map.clone();
    let player = Player::at_map_start(&map)
        .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidData, message))?;
    let game_title = format!("Wolfenstein 3D — {} ({actor_count} actors)", map.name);
    run(
        &mut map,
        Some(textures),
        doors,
        item_pickups,
        Some(audio),
        statics,
        sprites,
        Some(font),
        Some(status_bar),
        Some(preload_picture),
        Some(hud_pictures),
        Some(pause_sound_pictures),
        Some(graphics),
        Some(last_level_frame),
        weapon_pictures,
        player,
        &game_title,
        difficulty,
        data_variant,
        campaign,
        initial_save,
        initial_save_slot,
        help_articles,
        demo_playback,
    )
}

#[derive(Clone, Copy)]
struct IntermissionAnimation {
    time_bonus: u32,
    ratios: [u8; 3],
    found: [u16; 3],
    totals: [u16; 3],
}

pub fn run_intermission(
    map_number: usize,
    campaign: &mut CampaignState,
    hud: &world::HudPictures,
    portraits: [&crate::vga::Picture; 2],
    audio: &AudioArchive,
    last_level_frame: &[u32],
) -> Result<bool, Box<dyn Error>> {
    const PAR_TIMES_SECONDS: [[u32; 8]; 6] = [
        [90, 120, 120, 210, 180, 180, 150, 150],
        [90, 210, 180, 120, 240, 360, 60, 180],
        [90, 90, 150, 150, 210, 150, 120, 360],
        [120, 120, 90, 60, 270, 210, 120, 270],
        [150, 90, 150, 150, 240, 180, 270, 210],
        [390, 240, 270, 360, 300, 330, 330, 510],
    ];
    let stats = campaign.last_level_stats.unwrap_or_default();
    let level = map_number % 10;
    let episode = map_number / 10;
    if level < 8 {
        campaign.level_ratios[level] = campaign.last_level_stats;
    }
    let (bonus, rows, animation) = if level < 8 {
        let par = PAR_TIMES_SECONDS
            .get(episode)
            .and_then(|times| times.get(level))
            .copied()
            .unwrap_or(0);
        let time_bonus = if par == 0 {
            0
        } else {
            par.saturating_sub(stats.elapsed_seconds)
                .saturating_mul(500)
        };
        let kill_ratio = completion_ratio(stats.kills, stats.total_kills);
        let secret_ratio = completion_ratio(stats.secrets, stats.total_secrets);
        let treasure_ratio = completion_ratio(stats.treasures, stats.total_treasures);
        let perfect_bonus = (kill_ratio == 100) as u32 * 10_000
            + (secret_ratio == 100) as u32 * 10_000
            + (treasure_ratio == 100) as u32 * 10_000;
        let bonus = time_bonus.saturating_add(perfect_bonus);
        let par_display = if par == 0 {
            "??:??".to_owned()
        } else {
            format!("{:02}:{:02}", par / 60, par % 60)
        };
        let rows = vec![
            format!(
                "TIME {:02}:{:02}  PAR {par_display}",
                stats.elapsed_seconds / 60,
                stats.elapsed_seconds % 60
            ),
            format!(
                "KILLS {kill_ratio:3}% ({}/{})",
                stats.kills, stats.total_kills
            ),
            format!(
                "SECRETS {secret_ratio:3}% ({}/{})",
                stats.secrets, stats.total_secrets
            ),
            format!(
                "TREASURE {treasure_ratio:3}% ({}/{})",
                stats.treasures, stats.total_treasures
            ),
            format!("LEVEL BONUS {bonus:06}"),
            format!("TOTAL SCORE {:06}", campaign.score.saturating_add(bonus)),
        ];
        (
            bonus,
            rows,
            Some(IntermissionAnimation {
                time_bonus,
                ratios: [kill_ratio, secret_ratio, treasure_ratio],
                found: [stats.kills, stats.secrets, stats.treasures],
                totals: [
                    stats.total_kills,
                    stats.total_secrets,
                    stats.total_treasures,
                ],
            }),
        )
    } else {
        let bonus = 15_000;
        (
            bonus,
            vec![
                "SECRET FLOOR COMPLETED!".to_owned(),
                format!("LEVEL BONUS {bonus:06}"),
                format!("TOTAL SCORE {:06}", campaign.score.saturating_add(bonus)),
            ],
            None,
        )
    };
    let base_score = campaign.score;
    let mut points_awarded = false;
    let mut extra_life_sounds = 0usize;
    let mut quit = false;
    unsafe {
        if initialize_sdl(SDL_INIT_VIDEO) != 0 {
            return Err(sdl_error());
        }
        let _controller_subsystem_available = initialize_subsystem(SDL_INIT_GAMECONTROLLER) == 0;
        let mut controller = SdlController::new();
        controller.open_first();
        let intermission_music = start_screen_music(audio.music_player_for_track(16));
        let title = CString::new("Wolfenstein 3D — Level complete")?;
        let window = SDL_CreateWindow(
            title.as_ptr(),
            SDL_WINDOWPOS_CENTERED,
            SDL_WINDOWPOS_CENTERED,
            window_width(),
            window_height(),
            window_creation_flags(),
        );
        if window.is_null() {
            return Err(sdl_error());
        }
        let mut renderer = SDL_CreateRenderer(
            window,
            -1,
            SDL_RENDERER_ACCELERATED | SDL_RENDERER_PRESENTVSYNC,
        );
        if renderer.is_null() {
            renderer = SDL_CreateRenderer(window, -1, 0);
        }
        if renderer.is_null() {
            SDL_DestroyWindow(window);
            return Err(sdl_error());
        }
        let texture = SDL_CreateTexture(
            renderer,
            SDL_PIXELFORMAT_ARGB8888,
            SDL_TEXTUREACCESS_STREAMING,
            WIDTH as i32,
            HEIGHT as i32,
        );
        if texture.is_null() {
            SDL_DestroyRenderer(renderer);
            SDL_DestroyWindow(window);
            return Err(sdl_error());
        }

        let audio_device = intermission_music.device.0;

        let mut pixels = vec![0u32; WIDTH * HEIGHT];
        if last_level_frame.len() >= WIDTH * HEIGHT {
            pixels[WIDTH * 160..WIDTH * HEIGHT]
                .copy_from_slice(&last_level_frame[WIDTH * 160..WIDTH * HEIGHT]);
        }
        let mut dynamic_rows = rows.clone();
        let mut shown_bonus = 0u32;
        let mut shown_ratios = [0u8; 3];
        let mut animation_phase = if animation.is_some() { 0 } else { 4 };
        if let Some(animation) = animation {
            update_intermission_rows(
                &mut dynamic_rows,
                animation,
                shown_ratios,
                shown_bonus,
                base_score,
            );
        }
        if let Err(error) = present_intermission_frame(
            renderer,
            texture,
            &mut pixels,
            hud,
            portraits[0],
            map_number,
            &dynamic_rows,
            base_score,
            campaign.lives,
            0.0,
        ) {
            SDL_DestroyTexture(texture);
            SDL_DestroyRenderer(renderer);
            SDL_DestroyWindow(window);
            return Err(error);
        }

        let started = Instant::now();
        let fade_duration = Duration::from_secs_f64(30.0 / 70.0);
        let tally_started = started + fade_duration;
        let mut previous = tally_started;
        let mut fade_out_started = None;
        let mut fade_in_complete = false;
        let mut portrait_frame = 0;
        let mut next_breath = tally_started + Duration::from_secs_f64(11.0 / 70.0);
        let mut phase_elapsed = 0.0;
        let mut time_bonus_steps = 0usize;
        let mut ratio_bonus_delay: Option<(f64, usize)> = None;
        let mut ratio_bonus_sound_playing = false;
        let mut pending_sounds = VecDeque::new();
        let mut event = SdlEvent { bytes: [0; 56] };
        loop {
            let now = Instant::now();
            let mut changed = false;
            let mut finish_requested = false;
            while SDL_PollEvent(&mut event) != 0 {
                let event_type = u32::from_ne_bytes(event.bytes[0..4].try_into().unwrap());
                if event_type == SDL_QUIT {
                    quit = true;
                    break;
                } else if event_type == SDL_CONTROLLERDEVICEADDED {
                    let device_index = i32::from_ne_bytes(event.bytes[8..12].try_into().unwrap());
                    controller.device_added(device_index);
                } else if event_type == SDL_CONTROLLERDEVICEREMOVED {
                    let instance_id = i32::from_ne_bytes(event.bytes[8..12].try_into().unwrap());
                    controller.device_removed(instance_id);
                } else if enabled_user_input_event(event_type)
                    && (event_type != SDL_KEYDOWN || event.bytes[13] == 0)
                    && now >= tally_started
                    && fade_out_started.is_none()
                {
                    if animation_phase < 4 {
                        animation_phase = 4;
                        shown_bonus = bonus;
                        if let Some(animation) = animation {
                            shown_ratios = animation.ratios;
                            update_intermission_rows(
                                &mut dynamic_rows,
                                animation,
                                shown_ratios,
                                shown_bonus,
                                base_score,
                            );
                            changed = true;
                        }
                    } else {
                        finish_requested = true;
                    }
                }
            }
            if quit {
                break;
            }
            if finish_requested {
                fade_out_started = Some(now);
            }
            let brightness = if let Some(exit) = fade_out_started {
                let elapsed = now.duration_since(exit);
                if elapsed >= fade_duration {
                    finish_requested = true;
                }
                changed = true;
                (1.0 - elapsed.as_secs_f64() / fade_duration.as_secs_f64()).max(0.0)
            } else {
                if !fade_in_complete {
                    changed = true;
                    fade_in_complete = now >= tally_started;
                }
                (now.duration_since(started).as_secs_f64() / fade_duration.as_secs_f64()).min(1.0)
            };

            if now >= next_breath {
                portrait_frame ^= 1;
                next_breath = now + Duration::from_secs_f64(36.0 / 70.0);
                changed = true;
            }

            if now >= tally_started
                && audio_device != 0
                && !mixer_has_active_effect(audio_device, &intermission_music.mixer)
            {
                if let Some(sound) = pending_sounds.pop_front() {
                    queue_mixer_pc_sound(
                        Some(audio),
                        audio_device,
                        &intermission_music.mixer,
                        sound,
                    );
                }
            }
            if now < tally_started {
                previous = tally_started;
            } else if audio_device != 0
                && (mixer_has_active_effect(audio_device, &intermission_music.mixer)
                    || !pending_sounds.is_empty())
            {
                previous = Instant::now();
            } else {
                let frame_dt = now
                    .saturating_duration_since(previous)
                    .as_secs_f64()
                    .min(0.25);
                previous = now;
                if let Some((remaining, sound)) = ratio_bonus_delay {
                    let remaining = remaining - frame_dt;
                    if remaining <= 0.0 {
                        ratio_bonus_delay = None;
                        if sound == 48 && animation_phase < 4 {
                            shown_bonus = shown_bonus.saturating_add(10_000);
                            changed = true;
                            if let Some(animation) = animation {
                                update_intermission_rows(
                                    &mut dynamic_rows,
                                    animation,
                                    shown_ratios,
                                    shown_bonus,
                                    base_score,
                                );
                            }
                        }
                        if audio_device != 0 {
                            pending_sounds.push_back(sound);
                            ratio_bonus_sound_playing = true;
                        } else {
                            animation_phase += 1;
                        }
                    } else {
                        ratio_bonus_delay = Some((remaining, sound));
                    }
                } else if ratio_bonus_sound_playing {
                    ratio_bonus_sound_playing = false;
                    animation_phase += 1;
                    if audio_device != 0 && animation_phase < 4 {
                        pending_sounds.push_back(42);
                    }
                } else if animation_phase < 4 {
                    phase_elapsed += frame_dt;
                    let mut tick_sounds = Vec::new();
                    loop {
                        let interval = if animation_phase == 0 { 0.001 } else { 0.012 };
                        if phase_elapsed < interval {
                            break;
                        }
                        phase_elapsed -= interval;
                        let Some(animation) = animation else {
                            animation_phase = 4;
                            break;
                        };
                        match animation_phase {
                            0 if shown_bonus < animation.time_bonus => {
                                shown_bonus = time_bonus_steps as u32 * 500;
                                if time_bonus_steps % 50 == 0 {
                                    tick_sounds.push(42);
                                }
                                time_bonus_steps += 1;
                                changed = true;
                            }
                            0 => {
                                if animation.time_bonus > 0 {
                                    tick_sounds.push(43);
                                }
                                animation_phase = 1;
                                tick_sounds.push(42);
                            }
                            phase @ 1..=3 => {
                                let index = phase - 1;
                                let ratio = animation.ratios[index];
                                if shown_ratios[index] < ratio {
                                    shown_ratios[index] += 1;
                                    changed = true;
                                    if shown_ratios[index] % 10 == 0 {
                                        tick_sounds.push(42);
                                    }
                                } else if ratio == 100 || ratio == 0 {
                                    ratio_bonus_delay =
                                        Some((30.0 / 70.0, if ratio == 100 { 48 } else { 47 }));
                                    break;
                                } else {
                                    tick_sounds.push(43);
                                    animation_phase += 1;
                                    if animation_phase < 4 {
                                        tick_sounds.push(42);
                                    }
                                }
                            }
                            _ => animation_phase = 4,
                        }
                        if audio_device != 0 && !tick_sounds.is_empty() {
                            break;
                        }
                    }
                    if audio_device != 0 {
                        pending_sounds.extend(tick_sounds);
                    }
                    if let Some(animation) = animation {
                        update_intermission_rows(
                            &mut dynamic_rows,
                            animation,
                            shown_ratios,
                            shown_bonus,
                            base_score,
                        );
                    }
                }
            }
            let intermission_audio_idle = audio_device == 0
                || (pending_sounds.is_empty()
                    && !mixer_has_active_effect(audio_device, &intermission_music.mixer));
            if !points_awarded
                && animation_phase >= 4
                && now >= tally_started
                && intermission_audio_idle
            {
                extra_life_sounds = award_points(
                    bonus,
                    &mut campaign.score,
                    &mut campaign.next_extra_life_score,
                    &mut campaign.lives,
                );
                points_awarded = true;
                pending_sounds.extend(std::iter::repeat_n(44, extra_life_sounds));
                changed = true;
            }
            if changed {
                if let Err(error) = present_intermission_frame(
                    renderer,
                    texture,
                    &mut pixels,
                    hud,
                    portraits[portrait_frame],
                    map_number,
                    &dynamic_rows,
                    if points_awarded {
                        campaign.score
                    } else {
                        base_score.saturating_add(shown_bonus)
                    },
                    campaign.lives,
                    brightness,
                ) {
                    SDL_DestroyTexture(texture);
                    SDL_DestroyRenderer(renderer);
                    SDL_DestroyWindow(window);
                    return Err(error);
                }
            }
            if finish_requested && brightness == 0.0 {
                break;
            }
            SDL_Delay(1);
        }

        SDL_DestroyTexture(texture);
        SDL_DestroyRenderer(renderer);
        SDL_DestroyWindow(window);
    }
    Ok(!quit)
}

pub fn run_victory_screen(
    campaign: &CampaignState,
    hud: &world::HudPictures,
    hero: &crate::vga::Picture,
    audio: &AudioArchive,
    last_level_frame: &[u32],
) -> Result<(bool, Option<crate::opl2::ImfPlayer>), Box<dyn Error>> {
    let mut totals = [0u32; 4];
    for stats in campaign.level_ratios.iter().take(8).flatten() {
        totals[0] = totals[0].saturating_add(stats.elapsed_seconds);
        totals[1] =
            totals[1].saturating_add(completion_ratio(stats.kills, stats.total_kills) as u32);
        totals[2] =
            totals[2].saturating_add(completion_ratio(stats.secrets, stats.total_secrets) as u32);
        totals[3] = totals[3]
            .saturating_add(completion_ratio(stats.treasures, stats.total_treasures) as u32);
    }
    let elapsed_seconds = totals[0];
    let averages = [
        (totals[1] / 8) as u8,
        (totals[2] / 8) as u8,
        (totals[3] / 8) as u8,
    ];
    let mut quit = false;
    let mut carried_music = None;
    unsafe {
        if initialize_sdl(SDL_INIT_VIDEO) != 0 {
            return Err(sdl_error());
        }
        let _controller_subsystem_available = initialize_subsystem(SDL_INIT_GAMECONTROLLER) == 0;
        let mut controller = SdlController::new();
        controller.open_first();
        let mut victory_music = start_screen_music(audio.music_player_for_track(24));
        let title = CString::new("Wolfenstein 3D — Victory")?;
        let window = SDL_CreateWindow(
            title.as_ptr(),
            SDL_WINDOWPOS_CENTERED,
            SDL_WINDOWPOS_CENTERED,
            window_width(),
            window_height(),
            window_creation_flags(),
        );
        if window.is_null() {
            return Err(sdl_error());
        }
        let mut renderer = SDL_CreateRenderer(
            window,
            -1,
            SDL_RENDERER_ACCELERATED | SDL_RENDERER_PRESENTVSYNC,
        );
        if renderer.is_null() {
            renderer = SDL_CreateRenderer(window, -1, 0);
        }
        if renderer.is_null() {
            SDL_DestroyWindow(window);
            return Err(sdl_error());
        }
        let texture = SDL_CreateTexture(
            renderer,
            SDL_PIXELFORMAT_ARGB8888,
            SDL_TEXTUREACCESS_STREAMING,
            WIDTH as i32,
            HEIGHT as i32,
        );
        if texture.is_null() {
            SDL_DestroyRenderer(renderer);
            SDL_DestroyWindow(window);
            return Err(sdl_error());
        }
        let mut pixels = vec![0u32; WIDTH * HEIGHT];
        if last_level_frame.len() >= WIDTH * HEIGHT {
            pixels[WIDTH * 160..WIDTH * HEIGHT]
                .copy_from_slice(&last_level_frame[WIDTH * 160..WIDTH * HEIGHT]);
        }
        world::render_victory_screen(&mut pixels, hud, hero, elapsed_seconds, averages);
        let fade_duration = Duration::from_secs_f64(30.0 / 70.0);
        let fade_started = Instant::now();
        let mut fade_out_started = None;
        let mut event = SdlEvent { bytes: [0; 56] };
        loop {
            let now = Instant::now();
            while SDL_PollEvent(&mut event) != 0 {
                let event_type = u32::from_ne_bytes(event.bytes[0..4].try_into().unwrap());
                if event_type == SDL_QUIT {
                    quit = true;
                    fade_out_started.get_or_insert(now);
                } else if event_type == SDL_CONTROLLERDEVICEADDED {
                    let device_index = i32::from_ne_bytes(event.bytes[8..12].try_into().unwrap());
                    controller.device_added(device_index);
                } else if event_type == SDL_CONTROLLERDEVICEREMOVED {
                    let instance_id = i32::from_ne_bytes(event.bytes[8..12].try_into().unwrap());
                    controller.device_removed(instance_id);
                } else if enabled_user_input_event(event_type)
                    && (event_type != SDL_KEYDOWN || event.bytes[13] == 0)
                    && now.duration_since(fade_started) >= fade_duration
                    && fade_out_started.is_none()
                {
                    fade_out_started = Some(now);
                }
            }
            let brightness = if let Some(started) = fade_out_started {
                1.0 - now.duration_since(started).as_secs_f64() / fade_duration.as_secs_f64()
            } else {
                now.duration_since(fade_started).as_secs_f64() / fade_duration.as_secs_f64()
            }
            .clamp(0.0, 1.0);
            world::render_victory_screen(&mut pixels, hud, hero, elapsed_seconds, averages);
            fade_frame(&mut pixels, brightness);
            if SDL_UpdateTexture(
                texture,
                std::ptr::null(),
                pixels.as_ptr(),
                (WIDTH * 4) as i32,
            ) != 0
                || SDL_RenderClear(renderer) != 0
                || render_frame(renderer, texture) != 0
            {
                let error = sdl_error();
                SDL_DestroyTexture(texture);
                SDL_DestroyRenderer(renderer);
                SDL_DestroyWindow(window);
                return Err(error);
            }
            SDL_RenderPresent(renderer);
            if fade_out_started.is_some_and(|started| now.duration_since(started) >= fade_duration)
            {
                break;
            }
            SDL_Delay(16);
        }
        carried_music = victory_music.take_music();
        SDL_DestroyTexture(texture);
        SDL_DestroyRenderer(renderer);
        SDL_DestroyWindow(window);
    }
    Ok((!quit, carried_music))
}

pub fn record_campaign_score(
    campaign: &CampaignState,
    map_number: usize,
    font: &BitmapFont,
    audio: &AudioArchive,
    graphics: &crate::vga::VgaArchive,
    extension: &str,
) -> Result<bool, Box<dyn Error>> {
    let path = crate::resources::high_score_path();
    let mut table = crate::highscores::Table::load(&path);
    let rank = table.insert(
        campaign.score,
        (map_number % 10 + 1).min(u16::MAX as usize) as u16,
        (map_number / 10).min(u16::MAX as usize) as u16,
    );
    let result = run_high_score_screen(
        &mut table, rank, font, audio, graphics, extension, false, None,
    )?;
    table.save(&path)?;
    Ok(result != AttractResult::Quit)
}

fn run_high_score_screen(
    table: &mut crate::highscores::Table,
    rank: Option<usize>,
    font: &BitmapFont,
    audio: &AudioArchive,
    graphics: &crate::vga::VgaArchive,
    extension: &str,
    attract_timeout: bool,
    intro_music: Option<IntroMusic>,
) -> Result<AttractResult, Box<dyn Error>> {
    let mut quit = false;
    let mut running = true;
    let mut timed_out = false;
    let mut name = String::new();
    let shareware = extension.eq_ignore_ascii_case("WL1");
    let title_chunk = if shareware { 94 } else { 90 };
    let title_picture = graphics.picture(title_chunk)?;
    let column_pictures = if shareware {
        [None, None, None]
    } else {
        [
            graphics.picture(39)?,
            graphics.picture(38)?,
            graphics.picture(40)?,
        ]
    };
    unsafe {
        if initialize_sdl(SDL_INIT_VIDEO) != 0 {
            return Err(sdl_error());
        }
        let mut ack_input = SdlAcknowledgementInput::new();
        let screen_music = if let Some(intro_music) = intro_music {
            start_screen_music_with_state(intro_music)
        } else {
            start_screen_music(audio.music_player_for_track(23))
        };
        let _music = screen_music;
        if rank.is_some() {
            SDL_StartTextInput();
        }
        let title = CString::new("Wolfenstein 3D — High Scores")?;
        let window = SDL_CreateWindow(
            title.as_ptr(),
            SDL_WINDOWPOS_CENTERED,
            SDL_WINDOWPOS_CENTERED,
            window_width(),
            window_height(),
            window_creation_flags(),
        );
        if window.is_null() {
            if rank.is_some() {
                SDL_StopTextInput();
            }
            return Err(sdl_error());
        }
        let mut renderer = SDL_CreateRenderer(
            window,
            -1,
            SDL_RENDERER_ACCELERATED | SDL_RENDERER_PRESENTVSYNC,
        );
        if renderer.is_null() {
            renderer = SDL_CreateRenderer(window, -1, 0);
        }
        if renderer.is_null() {
            SDL_DestroyWindow(window);
            if rank.is_some() {
                SDL_StopTextInput();
            }
            return Err(sdl_error());
        }
        let texture = SDL_CreateTexture(
            renderer,
            SDL_PIXELFORMAT_ARGB8888,
            SDL_TEXTUREACCESS_STREAMING,
            WIDTH as i32,
            HEIGHT as i32,
        );
        if texture.is_null() {
            SDL_DestroyRenderer(renderer);
            SDL_DestroyWindow(window);
            if rank.is_some() {
                SDL_StopTextInput();
            }
            return Err(sdl_error());
        }
        let mut pixels = vec![0u32; WIDTH * HEIGHT];
        let mut event = SdlEvent { bytes: [0; 56] };
        let screen_started = Instant::now();
        let fade_duration = Duration::from_secs_f64(30.0 / 70.0);
        let input_enabled_at = screen_started + fade_duration;
        let deadline =
            attract_timeout.then(|| input_enabled_at + std::time::Duration::from_secs(10));
        let mut exit_fade_started = None;
        while running {
            while SDL_PollEvent(&mut event) != 0 {
                let event_type = u32::from_ne_bytes(event.bytes[0..4].try_into().unwrap());
                if event_type == SDL_QUIT {
                    quit = true;
                    running = false;
                } else if Instant::now() >= input_enabled_at
                    && event_type == SDL_TEXTINPUT
                    && rank.is_some()
                {
                    if exit_fade_started.is_some() {
                        continue;
                    }
                    for byte in &event.bytes[12..44] {
                        if *byte == 0 {
                            break;
                        }
                        if (byte.is_ascii_graphic() || *byte == b' ')
                            && name.len() < crate::highscores::MAX_NAME_BYTES
                        {
                            name.push(*byte as char);
                        }
                    }
                } else if Instant::now() >= input_enabled_at
                    && event_type == SDL_KEYDOWN
                    && event.bytes[13] == 0
                {
                    if exit_fade_started.is_some() {
                        continue;
                    }
                    if let Some(score_rank) = rank {
                        let scancode = i32::from_ne_bytes(event.bytes[16..20].try_into().unwrap());
                        match scancode {
                            SDL_SCANCODE_RETURN => {
                                table.set_name(
                                    score_rank,
                                    if name.is_empty() { "PLAYER" } else { &name },
                                );
                                exit_fade_started = Some(Instant::now());
                            }
                            SDL_SCANCODE_BACKSPACE => {
                                name.pop();
                            }
                            value if value == SDL_SCANCODE_ESCAPE as i32 => {
                                table.set_name(
                                    score_rank,
                                    if name.is_empty() { "PLAYER" } else { &name },
                                );
                                exit_fade_started = Some(Instant::now());
                            }
                            _ => {}
                        }
                    } else {
                        exit_fade_started = Some(Instant::now());
                    }
                } else if Instant::now() >= input_enabled_at && ack_input.handles(&event) {
                    if rank.is_none() {
                        exit_fade_started.get_or_insert_with(Instant::now);
                    }
                }
            }
            if !running {
                break;
            }
            if deadline.is_some_and(|deadline| std::time::Instant::now() >= deadline) {
                timed_out = true;
                exit_fade_started.get_or_insert_with(Instant::now);
            }
            world::render_high_score_screen(
                &mut pixels,
                font,
                table,
                rank.map(|_| name.as_str()),
                rank,
                title_picture.as_ref(),
                [
                    column_pictures[0].as_ref(),
                    column_pictures[1].as_ref(),
                    column_pictures[2].as_ref(),
                ],
                shareware,
            );
            let brightness = if let Some(started) = exit_fade_started {
                1.0 - (started.elapsed().as_secs_f64() / fade_duration.as_secs_f64())
            } else {
                screen_started.elapsed().as_secs_f64() / fade_duration.as_secs_f64()
            };
            fade_frame(&mut pixels, brightness);
            if SDL_UpdateTexture(
                texture,
                std::ptr::null(),
                pixels.as_ptr(),
                (WIDTH * 4) as i32,
            ) != 0
                || SDL_RenderClear(renderer) != 0
                || render_frame(renderer, texture) != 0
            {
                let error = sdl_error();
                SDL_DestroyTexture(texture);
                SDL_DestroyRenderer(renderer);
                SDL_DestroyWindow(window);
                if rank.is_some() {
                    SDL_StopTextInput();
                }
                return Err(error);
            }
            SDL_RenderPresent(renderer);
            if exit_fade_started.is_some_and(|started| started.elapsed() >= fade_duration) {
                running = false;
                break;
            }
            SDL_Delay(16);
        }
        if let Some(score_rank) = rank {
            if table.entries[score_rank].name.is_empty() {
                table.set_name(score_rank, if name.is_empty() { "PLAYER" } else { &name });
            }
        }
        SDL_DestroyTexture(texture);
        SDL_DestroyRenderer(renderer);
        SDL_DestroyWindow(window);
        if rank.is_some() {
            SDL_StopTextInput();
        }
    }
    Ok(if quit {
        AttractResult::Quit
    } else if timed_out {
        AttractResult::TimedOut
    } else {
        AttractResult::SkipRemaining
    })
}

pub fn run_ending_articles(
    archive: &crate::vga::VgaArchive,
    extension: &str,
    episode: usize,
    font: &BitmapFont,
    music: Option<crate::opl2::ImfPlayer>,
) -> Result<bool, Box<dyn Error>> {
    if extension.eq_ignore_ascii_case("WL1") || episode >= 6 {
        return Ok(false);
    }
    run_article_chunk(
        archive,
        143 + episode,
        "Wolfenstein 3D — Ending",
        music,
        font,
    )
}

pub fn run_help_articles(
    archive: &crate::vga::VgaArchive,
    extension: &str,
    font: &BitmapFont,
    audio: &AudioArchive,
) -> Result<bool, Box<dyn Error>> {
    if extension.eq_ignore_ascii_case("WL1") {
        return Ok(false);
    }
    run_article_chunk(
        archive,
        138,
        "Wolfenstein 3D — Read This!",
        audio.music_player_for_track(0),
        font,
    )
}

pub fn prepare_help_articles(
    archive: &crate::vga::VgaArchive,
    extension: &str,
    font: &BitmapFont,
) -> Result<Option<HelpArticles>, Box<dyn Error>> {
    if extension.eq_ignore_ascii_case("WL1") || archive.chunk_count() <= 138 {
        return Ok(None);
    }
    let Some(article) = archive.chunk(138)? else {
        return Ok(None);
    };
    let pages = ending::parse(&article, archive, font)?;
    if pages.is_empty() {
        return Ok(None);
    }
    let (Some(top), Some(left), Some(right), Some(bottom)) = (
        archive.picture(6)?,
        archive.picture(7)?,
        archive.picture(8)?,
        archive.picture(9)?,
    ) else {
        return Ok(None);
    };
    Ok(Some(HelpArticles {
        pages,
        frame: [top, left, right, bottom],
    }))
}

fn run_article_chunk(
    archive: &crate::vga::VgaArchive,
    article_chunk: usize,
    title_text: &str,
    music: Option<crate::opl2::ImfPlayer>,
    font: &BitmapFont,
) -> Result<bool, Box<dyn Error>> {
    if article_chunk >= archive.chunk_count() {
        return Ok(false);
    }
    let Some(article) = archive.chunk(article_chunk)? else {
        return Ok(false);
    };
    let pages = ending::parse(&article, archive, font)?;
    if pages.is_empty() {
        return Ok(false);
    }
    let Some(top_frame) = archive.picture(6)? else {
        return Ok(false);
    };
    let Some(left_frame) = archive.picture(7)? else {
        return Ok(false);
    };
    let Some(right_frame) = archive.picture(8)? else {
        return Ok(false);
    };
    let Some(bottom_frame) = archive.picture(9)? else {
        return Ok(false);
    };
    let frame = [top_frame, left_frame, right_frame, bottom_frame];

    let mut current_page = 0usize;
    let mut running = true;
    let fade_in_duration = Duration::from_secs_f64(10.0 / 70.0);
    let fade_out_duration = Duration::from_secs_f64(30.0 / 70.0);
    let mut fade_out_started: Option<(Instant, f64)> = None;
    unsafe {
        if initialize_sdl(SDL_INIT_VIDEO) != 0 {
            return Err(sdl_error());
        }
        let _article_music = start_screen_music(music);
        let title = CString::new(title_text)?;
        let window = SDL_CreateWindow(
            title.as_ptr(),
            SDL_WINDOWPOS_CENTERED,
            SDL_WINDOWPOS_CENTERED,
            window_width(),
            window_height(),
            window_creation_flags(),
        );
        if window.is_null() {
            return Err(sdl_error());
        }
        let mut renderer = SDL_CreateRenderer(
            window,
            -1,
            SDL_RENDERER_ACCELERATED | SDL_RENDERER_PRESENTVSYNC,
        );
        if renderer.is_null() {
            renderer = SDL_CreateRenderer(window, -1, 0);
        }
        if renderer.is_null() {
            SDL_DestroyWindow(window);
            return Err(sdl_error());
        }
        let texture = SDL_CreateTexture(
            renderer,
            SDL_PIXELFORMAT_ARGB8888,
            SDL_TEXTUREACCESS_STREAMING,
            WIDTH as i32,
            HEIGHT as i32,
        );
        if texture.is_null() {
            SDL_DestroyRenderer(renderer);
            SDL_DestroyWindow(window);
            return Err(sdl_error());
        }
        let mut pixels = vec![0u32; WIDTH * HEIGHT];
        let mut event = SdlEvent { bytes: [0; 56] };
        let first_page_started = Instant::now();
        let first_page_layout_delay =
            Duration::from_secs_f64(pages[0].layout_delay_tics() as f64 / 70.0);
        let mut page_started = Instant::now();
        while running {
            while SDL_PollEvent(&mut event) != 0 {
                let event_type = u32::from_ne_bytes(event.bytes[0..4].try_into().unwrap());
                if event_type == SDL_QUIT {
                    running = false;
                } else if event_type == SDL_KEYDOWN && event.bytes[13] == 0 {
                    if fade_out_started.is_some() {
                        continue;
                    }
                    let scancode = i32::from_ne_bytes(event.bytes[16..20].try_into().unwrap());
                    let layout_delay = Duration::from_secs_f64(
                        pages[current_page].layout_delay_tics() as f64 / 70.0,
                    );
                    let first_page_fade_complete = current_page != 0
                        || first_page_started.elapsed()
                            >= first_page_layout_delay.saturating_add(fade_in_duration);
                    if page_started.elapsed() < layout_delay || !first_page_fade_complete {
                        continue;
                    }
                    match scancode {
                        value if value == SDL_SCANCODE_ESCAPE as i32 => {
                            let now = Instant::now();
                            fade_out_started = Some((now, 1.0));
                        }
                        80 | 82 | SDL_SCANCODE_PAGEUP if current_page > 0 => {
                            current_page -= 1;
                            page_started = Instant::now();
                        }
                        SDL_SCANCODE_RETURN | 79 | 81 | SDL_SCANCODE_PAGEDOWN
                            if current_page + 1 < pages.len() =>
                        {
                            current_page += 1;
                            page_started = Instant::now();
                        }
                        _ => {}
                    }
                }
            }
            if !running {
                break;
            }
            pages[current_page].render(
                &mut pixels,
                font,
                &frame,
                current_page + 1,
                pages.len(),
                page_started.elapsed().as_secs_f64() * 70.0,
            );
            let brightness = if let Some((fade_started, fade_from)) = fade_out_started {
                fade_from
                    * (1.0 - fade_started.elapsed().as_secs_f64() / fade_out_duration.as_secs_f64())
                        .clamp(0.0, 1.0)
            } else if current_page == 0 {
                (first_page_started.elapsed().as_secs_f64() / fade_in_duration.as_secs_f64())
                    .clamp(0.0, 1.0)
            } else {
                1.0
            };
            fade_frame(&mut pixels, brightness);
            if SDL_UpdateTexture(
                texture,
                std::ptr::null(),
                pixels.as_ptr(),
                (WIDTH * 4) as i32,
            ) != 0
                || SDL_RenderClear(renderer) != 0
                || render_frame(renderer, texture) != 0
            {
                let error = sdl_error();
                SDL_DestroyTexture(texture);
                SDL_DestroyRenderer(renderer);
                SDL_DestroyWindow(window);
                return Err(error);
            }
            SDL_RenderPresent(renderer);
            if fade_out_started.is_some_and(|(started, _)| started.elapsed() >= fade_out_duration) {
                break;
            }
            SDL_Delay(8);
        }
        SDL_DestroyTexture(texture);
        SDL_DestroyRenderer(renderer);
        SDL_DestroyWindow(window);
    }
    Ok(true)
}

fn update_intermission_rows(
    rows: &mut [String],
    animation: IntermissionAnimation,
    ratios: [u8; 3],
    bonus: u32,
    base_score: u32,
) {
    const LABELS: [&str; 3] = ["KILLS", "SECRETS", "TREASURE"];
    for index in 0..3 {
        rows[index + 1] = format!(
            "{} {:3}% ({}/{})",
            LABELS[index], ratios[index], animation.found[index], animation.totals[index]
        );
    }
    rows[4] = format!("LEVEL BONUS {bonus:06}");
    rows[5] = format!("TOTAL SCORE {:06}", base_score.saturating_add(bonus));
}

fn present_intermission_frame(
    renderer: *mut SdlRenderer,
    texture: *mut SdlTexture,
    pixels: &mut [u32],
    hud: &world::HudPictures,
    portrait: &crate::vga::Picture,
    map_number: usize,
    rows: &[String],
    score: u32,
    lives: u8,
    brightness: f64,
) -> Result<(), Box<dyn Error>> {
    world::render_intermission(pixels, hud, portrait, map_number, rows, score, lives);
    fade_frame(pixels, brightness);
    unsafe {
        if SDL_UpdateTexture(
            texture,
            std::ptr::null(),
            pixels.as_ptr(),
            (WIDTH * 4) as i32,
        ) != 0
            || SDL_RenderClear(renderer) != 0
            || render_frame(renderer, texture) != 0
        {
            return Err(sdl_error());
        }
        SDL_RenderPresent(renderer);
    }
    Ok(())
}

fn completion_ratio(found: u16, total: u16) -> u8 {
    if total == 0 {
        0
    } else {
        ((found as u32 * 100) / total as u32).min(100) as u8
    }
}

fn sanitize_save_name(name: &str) -> String {
    name.chars()
        .filter(|character| character.is_ascii_graphic() || *character == ' ')
        .take(31)
        .collect()
}

/// Mirrors the DOS `thrustspeed` sum used by `T_Shoot` to decide whether the
/// player's movement is fast enough to reduce enemy accuracy.
fn dos_player_thrustspeed(forward: f64, strafe: f64, run_held: bool) -> f64 {
    let control_step = if run_held { 70.0 } else { 35.0 };
    let forward_scale = if forward < 0.0 { 100.0 } else { 150.0 };
    forward.abs() * control_step * forward_scale + strafe.abs() * control_step * 150.0
}

fn dos_player_is_running(
    forward: f64,
    strafe: f64,
    run_held: bool,
    mouse_forward_control: f64,
    mouse_strafe_control: f64,
) -> bool {
    let control_step = if run_held { 70.0 } else { 35.0 };
    let forward = forward - mouse_forward_control / control_step;
    let strafe = strafe - mouse_strafe_control / control_step;
    dos_player_thrustspeed(forward, strafe, run_held) >= 6_000.0
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct DemoPlayerInput {
    forward: f64,
    strafe: f64,
    turn: f64,
    run: bool,
    use_pressed: bool,
    fire_pressed: bool,
    weapon_slot: Option<usize>,
}

fn demo_player_input(command: crate::demo::DemoCommand) -> DemoPlayerInput {
    const DOS_RUN_BUTTON: u8 = 1 << 2;
    const DOS_STRAFE_BUTTON: u8 = 1 << 1;
    const DOS_USE_BUTTON: u8 = 1 << 3;
    const DOS_ATTACK_BUTTON: u8 = 1;
    let run = command.buttons & DOS_RUN_BUTTON != 0;
    let step = if run { 70.0 } else { 35.0 };
    // PollControls replays the signed bytes directly. The run button
    // determines the normalization here. Playback does not clamp the axes.
    let control_x = f64::from(command.control_x) / step;
    let control_y = f64::from(command.control_y) / step;
    let strafing = command.buttons & DOS_STRAFE_BUTTON != 0;
    let weapon_slot = (0usize..4).find(|slot| command.buttons & (1u8 << (*slot + 4)) != 0);
    DemoPlayerInput {
        forward: -control_y,
        strafe: if strafing { control_x } else { 0.0 },
        turn: if strafing { 0.0 } else { control_x },
        run,
        use_pressed: command.buttons & DOS_USE_BUTTON != 0,
        fire_pressed: command.buttons & DOS_ATTACK_BUTTON != 0,
        weapon_slot,
    }
}

fn static_actor_spawns(spawns: &[ActorSpawn]) -> Vec<ActorSpawn> {
    spawns
        .iter()
        .copied()
        .filter(|actor| {
            matches!(
                actor.kind,
                ActorKind::Decoration(_) | ActorKind::Enemy(crate::actors::EnemyKind::DeadGuard)
            )
        })
        .collect()
}

fn run(
    map: &mut Map,
    textures: Option<&Vswap>,
    mut doors: DoorSystem,
    mut item_pickups: Vec<ItemPickup>,
    audio: Option<&AudioArchive>,
    statics: &[crate::actors::ActorSpawn],
    sprites: &SpriteAtlas,
    font: Option<&BitmapFont>,
    status_bar: Option<&crate::vga::Picture>,
    preload_picture: Option<&crate::vga::Picture>,
    hud_pictures: Option<&world::HudPictures>,
    pause_sound_pictures: Option<&world::PauseSoundMenuPictures>,
    graphics: Option<&crate::vga::VgaArchive>,
    mut last_level_frame: Option<&mut [u32]>,
    weapon_pictures: &[crate::vga::Picture],
    mut player: Player,
    title: &str,
    difficulty: Difficulty,
    data_variant: &str,
    campaign: &mut CampaignState,
    mut pending_restore: Option<LevelSaveState>,
    initial_save_slot: usize,
    help_articles: Option<&HelpArticles>,
    mut demo_playback: Option<crate::demo::DemoPlayback>,
) -> Result<LevelResult, Box<dyn Error>> {
    let pause_high_score_art = if let Some(graphics) = graphics {
        let shareware = data_variant.eq_ignore_ascii_case("WL1");
        Some(PauseHighScoreArt {
            title: graphics.picture(if shareware { 94 } else { 90 })?,
            columns: if shareware {
                [None, None, None]
            } else {
                [
                    graphics.picture(39)?,
                    graphics.picture(38)?,
                    graphics.picture(40)?,
                ]
            },
            shareware,
        })
    } else {
        None
    };
    let pause_new_game_art = graphics
        .map(|archive| PauseNewGameArt::load(archive, data_variant))
        .transpose()?;
    let mut pause_high_score_table = crate::highscores::Table::load(&resources::high_score_path());
    let mut level_tics = 0u64;
    let map9_death_scream_available = audio.is_some_and(|archive| archive.has_sound(75));
    let total_kills = statics
        .iter()
        .filter(|actor| {
            matches!(actor.kind, ActorKind::Enemy(kind) if kind != crate::actors::EnemyKind::DeadGuard)
        })
        .count()
        .min(u16::MAX as usize) as u16;
    let total_secrets = map.planes[1]
        .iter()
        .filter(|&&tile| tile == 98)
        .count()
        .min(u16::MAX as usize) as u16;
    let total_treasures = map.planes[1]
        .iter()
        .filter(|&&tile| (52..=56).contains(&tile))
        .count()
        .min(u16::MAX as usize) as u16;
    let mut treasures_found = 0u16;
    let mut level_kills = 0u16;
    let initial_map = map.clone();
    let initial_doors = doors.clone();
    let initial_pickups = item_pickups.clone();
    let initial_player = player;
    let mut active_actors = static_actor_spawns(statics);
    let mut enemies = EnemyRuntime::from_spawns(
        statics,
        difficulty,
        demo_playback.is_some(),
        crate::settings::digitized_effects_enabled(),
    );
    let mut bj_victory: Option<crate::actors::BjVictory> = None;
    let mut victory_spin_target_y: Option<f64> = None;
    let mut victory_player_running = None;
    let mut pushwalls = PushWallSystem::default();
    let outcome = unsafe {
        if initialize_sdl(SDL_INIT_VIDEO) != 0 {
            return Err(sdl_error());
        }
        // Keep the game usable on machines without an audio output device.
        let _audio_subsystem_available = initialize_subsystem(SDL_INIT_AUDIO) == 0;
        let _controller_subsystem_available = initialize_subsystem(SDL_INIT_GAMECONTROLLER) == 0;
        let mut controller = SdlController::new();
        controller.open_first();
        let title = CString::new(title)?;
        let window = SDL_CreateWindow(
            title.as_ptr(),
            SDL_WINDOWPOS_CENTERED,
            SDL_WINDOWPOS_CENTERED,
            window_width(),
            window_height(),
            window_creation_flags(),
        );
        if window.is_null() {
            return Err(sdl_error());
        }
        let mut renderer = SDL_CreateRenderer(
            window,
            -1,
            SDL_RENDERER_ACCELERATED | SDL_RENDERER_PRESENTVSYNC,
        );
        if renderer.is_null() {
            renderer = SDL_CreateRenderer(window, -1, 0);
        }
        if renderer.is_null() {
            SDL_DestroyWindow(window);
            return Err(sdl_error());
        }
        let texture = SDL_CreateTexture(
            renderer,
            SDL_PIXELFORMAT_ARGB8888,
            SDL_TEXTUREACCESS_STREAMING,
            WIDTH as i32,
            HEIGHT as i32,
        );
        if texture.is_null() {
            SDL_DestroyRenderer(renderer);
            SDL_DestroyWindow(window);
            return Err(sdl_error());
        }
        if demo_playback.is_none() {
            set_gameplay_mouse_capture();
        }

        let audio_mixer = Box::new(UnsafeCell::new(AudioMixer {
            music: audio.and_then(|archive| archive.music_player_for_map(map.number)),
            opl2: {
                let mut synth = crate::opl2::Opl2Synth::new(OUTPUT_RATE);
                synth.write_register(0x01, 0x20);
                synth
            },
            synthesized_effect: None,
            digitized_effect: None,
            listener: (player.x, player.y, player.angle),
        }));
        let audio_spec = SdlAudioSpec {
            frequency: OUTPUT_RATE as i32,
            format: SDL_AUDIO_S16LSB,
            channels: 2,
            silence: 0,
            samples: 1024,
            padding: 0,
            size: 0,
            callback: Some(level_audio_callback),
            userdata: (&*audio_mixer as *const UnsafeCell<AudioMixer>)
                .cast_mut()
                .cast(),
        };
        let audio_device = open_audio_device(crate::settings::audio_device(), &audio_spec);
        let _audio_device_guard = SdlAudioDeviceGuard(audio_device);
        if audio_device != 0 {
            SDL_PauseAudioDevice(audio_device, 0);
        }
        let mut event = SdlEvent { bytes: [0; 56] };
        let mut pixels = vec![0u32; WIDTH * HEIGHT];
        let mut mouse_motion_x = 0i32;
        let mut mouse_motion_y = 0i32;
        let mut mouse_buttons = 0u8;
        let mut previous = Instant::now();
        let mut simulation_accumulator = 0.0;
        let simulation_tic = 1.0 / 70.0;
        let mut running = true;
        let mut use_consumed = false;
        let mut fire_consumed = false;
        let mut load_was_pressed = false;
        let mut selected_save_slot = initial_save_slot;
        let mut save_path = crate::resources::save_slot_path(selected_save_slot);
        let mut save_name = crate::savegame::display_name(&save_path)
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "Save Slot 1".into());
        let mut editing_save_name = false;
        let mut save_name_draft = String::new();
        let mut overwrite_pending = None;
        let mut runtime_notice: Option<String> = None;
        let mut keys_owned = campaign.keys_owned;
        let mut health = campaign.health;
        let mut arsenal = campaign.arsenal.clone();
        let mut score = campaign.score;
        let mut next_extra_life_score = campaign.next_extra_life_score;
        let mut kills = campaign.kills;
        let mut weapon_attack = WeaponAttack::default();
        let mut lives = campaign.lives;
        let mut game_over = false;
        let mut level_complete = false;
        let mut victory = false;
        let mut death_elapsed = None;
        let mut death_turn_target: Option<f64> = None;
        let mut death_fade_pending = false;
        let mut damage_flash_tics = 0u32;
        let mut bonus_flash_tics = 0u32;
        let mut displayed_flash = (0, 0);
        let mut gatling_pickup_face = false;
        let mut game_paused = false;
        let mut help_article_page: Option<usize> = None;
        let mut help_page_started = Instant::now();
        let mut pause_key_screen_open = false;
        let mut pause_key_screen_ready = false;
        let mut pause_key_clear_controls = false;
        let mut pause_selection = 0usize;
        let mut pause_scores_open = false;
        let mut pause_slot_mode: Option<bool> = None;
        let mut pause_slot_selection = 0usize;
        let mut pause_slot_labels = runtime_save_slot_labels();
        let mut pause_sound_settings_open = false;
        let mut pause_sound_return_to_game = false;
        let mut pause_sound_selection = 0usize;
        let mut pause_controls_open = false;
        let mut pause_controls_return_to_game = false;
        let mut pause_controls_selection = 0usize;
        let mut pause_control_capture = None;
        let mut pause_button_mapping: Option<bool> = None;
        let mut pause_button_capture: Option<(bool, u8)> = None;
        let mut pause_movement_mapping_open = false;
        let mut pause_move_key_capture: Option<usize> = None;
        let mut pause_display_open = false;
        let mut pause_display_selection = 0usize;
        let mut pause_view_size_open = false;
        let mut pause_view_size_original = None;
        let mut pause_view_size_return_to_game = false;
        let mut pause_view_mouse_delta_x = 0i32;
        let mut pause_view_mouse_delta_y = 0i32;
        let mut pause_view_size_repeat_due = None;
        let mut pause_cursor_sound_due = None;
        let mut pause_cursor_input_due = None;
        let mut pause_navigation_repeat_scancode = None;
        let mut pause_navigation_repeat_due = None;
        let mut pause_confirming_quit = false;
        let mut pause_confirming_new_game = false;
        let mut pause_new_game: Option<PauseNewGameSelection> = None;
        let mut pause_quick_end_overlay = false;
        let mut pause_confirming_application_quit = false;
        let mut pause_quick_quit_overlay = false;
        let mut pause_quick_quit_prompt = QUIT_CONFIRMATION_MESSAGES[0];
        let mut pause_quit_confirm_selection = 1usize;
        let mut fast_end_requested = false;
        let mut pause_confirming_load = false;
        let mut pause_load_confirm_selection = 1usize;
        let mut quickload_fallback_open = false;
        let mut quickload_audio_resume = false;
        let mut level_result = None;
        let mut level_reveal_pending = true;
        let mut reveal_from_death = false;
        let mut interstitial_was_active = false;
        let mut camera_fade_in_pending = false;
        let mut pause_transition: Option<PauseTransition> = None;
        if demo_playback.is_none() {
            if let (Some(picture), Some(status_bar)) = (preload_picture, status_bar) {
                let _ = SDL_SetRelativeMouseMode(0);
                if !present_preload_screen(renderer, texture, &mut pixels, picture, status_bar)? {
                    level_result = Some(LevelResult::Quit);
                    running = false;
                }
                if running {
                    set_gameplay_mouse_capture();
                }
            }
        }
        while running {
            let mut state_changed = false;
            let mut pause_save_requested = false;
            let mut pause_quicksave_requested = false;
            let mut pause_load_requested = false;
            while SDL_PollEvent(&mut event) != 0 {
                let event_type = u32::from_ne_bytes(event.bytes[0..4].try_into().unwrap());
                if event_type == SDL_QUIT {
                    if demo_playback.is_some() {
                        level_result = Some(LevelResult::Quit);
                    }
                    running = false;
                } else if event_type == SDL_CONTROLLERDEVICEADDED {
                    let device_index = i32::from_ne_bytes(event.bytes[8..12].try_into().unwrap());
                    controller.device_added(device_index);
                } else if event_type == SDL_CONTROLLERDEVICEREMOVED {
                    let instance_id = i32::from_ne_bytes(event.bytes[8..12].try_into().unwrap());
                    controller.device_removed(instance_id);
                } else if event_type == SDL_CONTROLLERBUTTONDOWN
                    && pause_button_capture.is_some()
                    && crate::settings::controller_input_enabled()
                {
                    if let Some((false, action)) = pause_button_capture {
                        if let Some(button) = controller_gameplay_button_slot(event.bytes[12]) {
                            crate::settings::set_controller_button_action(action, button)?;
                            pause_button_capture = None;
                            runtime_notice = Some(format!(
                                "{} CONTROLLER BUTTON UPDATED",
                                button_action_name(action)
                            ));
                            state_changed = true;
                        }
                    }
                } else if event_type == SDL_CONTROLLERBUTTONDOWN
                    && crate::settings::controller_input_enabled()
                    && pause_control_capture.is_none()
                    && pause_button_capture.is_none()
                    && pause_move_key_capture.is_none()
                    && !editing_save_name
                    && !pause_key_screen_open
                {
                    let button = event.bytes[12];
                    let menu_active = game_paused || help_article_page.is_some();
                    let scancode = if i32::from(button) == SDL_CONTROLLER_BUTTON_START {
                        Some(SDL_SCANCODE_ESCAPE as i32)
                    } else if menu_active {
                        controller_menu_scancode(button)
                    } else {
                        None
                    };
                    if let Some(scancode) = scancode {
                        unsafe { push_keydown_scancode(scancode) };
                    }
                } else if event_type == SDL_WINDOWEVENT {
                    match event.bytes[12] {
                        SDL_WINDOWEVENT_FOCUS_LOST => {
                            mouse_motion_x = 0;
                            mouse_motion_y = 0;
                            mouse_buttons = 0;
                            let _ = SDL_SetRelativeMouseMode(0);
                        }
                        SDL_WINDOWEVENT_FOCUS_GAINED
                            if !game_paused
                                && help_article_page.is_none()
                                && pause_transition.is_none()
                                && demo_playback.is_none()
                                && death_elapsed.is_none()
                                && bj_victory.is_none()
                                && !level_reveal_pending
                                && !editing_save_name =>
                        {
                            set_gameplay_mouse_capture();
                        }
                        _ => {}
                    }
                } else if event_type == SDL_MOUSEMOTION {
                    if !game_paused
                        && crate::settings::mouse_input_enabled()
                        && help_article_page.is_none()
                        && pause_transition.is_none()
                        && death_elapsed.is_none()
                        && demo_playback.is_none()
                        && bj_victory.is_none()
                        && !pause_key_screen_open
                        && !level_reveal_pending
                        && !editing_save_name
                    {
                        let dx = i32::from_ne_bytes(event.bytes[28..32].try_into().unwrap());
                        let dy = i32::from_ne_bytes(event.bytes[32..36].try_into().unwrap());
                        mouse_motion_x = mouse_motion_x.saturating_add(dx);
                        mouse_motion_y = mouse_motion_y.saturating_add(dy);
                    } else if game_paused
                        && pause_view_size_open
                        && crate::settings::mouse_input_enabled()
                        && pause_transition.is_none()
                        && !editing_save_name
                    {
                        pause_view_mouse_delta_x = pause_view_mouse_delta_x.saturating_add(
                            i32::from_ne_bytes(event.bytes[28..32].try_into().unwrap()),
                        );
                        pause_view_mouse_delta_y = pause_view_mouse_delta_y.saturating_add(
                            i32::from_ne_bytes(event.bytes[32..36].try_into().unwrap()),
                        );
                        let mut adjustment =
                            if pause_view_mouse_delta_y < -PAUSE_VIEW_SIZE_MOUSE_THRESHOLD {
                                Some(1i8)
                            } else if pause_view_mouse_delta_y > PAUSE_VIEW_SIZE_MOUSE_THRESHOLD {
                                Some(-1i8)
                            } else {
                                None
                            };
                        if pause_view_mouse_delta_x < -PAUSE_VIEW_SIZE_MOUSE_THRESHOLD {
                            adjustment = Some(-1);
                        } else if pause_view_mouse_delta_x > PAUSE_VIEW_SIZE_MOUSE_THRESHOLD {
                            adjustment = Some(1);
                        }
                        if let Some(direction) = adjustment {
                            pause_view_mouse_delta_x = 0;
                            pause_view_mouse_delta_y = 0;
                            let current = crate::settings::view_size();
                            let next = if direction > 0 {
                                current
                                    .saturating_add(1)
                                    .min(crate::settings::MAX_VIEW_SIZE)
                            } else {
                                current
                                    .saturating_sub(1)
                                    .max(crate::settings::MIN_VIEW_SIZE)
                            };
                            if next != current {
                                crate::settings::preview_view_size(next)?;
                                state_changed = true;
                            }
                            queue_mixer_pc_sound(audio, audio_device, &audio_mixer, 0);
                        }
                    } else if game_paused
                        && crate::settings::mouse_input_enabled()
                        && help_article_page.is_none()
                        && pause_transition.is_none()
                        && !pause_scores_open
                        && !pause_view_size_open
                        && !pause_key_screen_open
                        && !editing_save_name
                        && pause_cursor_input_due.is_none_or(|due| Instant::now() >= due)
                    {
                        let x = i32::from_ne_bytes(event.bytes[20..24].try_into().unwrap());
                        let y = i32::from_ne_bytes(event.bytes[24..28].try_into().unwrap());
                        let target = active_pause_menu_mouse_target(
                            pause_confirming_load
                                || pause_confirming_quit
                                || pause_confirming_new_game
                                || pause_confirming_application_quit,
                            pause_new_game.as_ref().map(|menu| menu.step),
                            pause_new_game_art.as_ref().map_or((0, 0), |art| {
                                (art.episode_options.len(), art.difficulty_options.len())
                            }),
                            pause_slot_mode,
                            pause_sound_settings_open,
                            pause_controls_open,
                            pause_button_mapping,
                            pause_movement_mapping_open,
                            pause_control_capture.is_some()
                                || pause_button_capture.is_some()
                                || pause_move_key_capture.is_some(),
                            pause_display_open,
                            pause_slot_labels.len(),
                        );
                        if let (Some((target, options_len)), Some((x, y))) =
                            (target, menu_pointer_coordinates(window, renderer, x, y))
                        {
                            if let Some(index) = pause_menu_selection_at(target, options_len, x, y)
                            {
                                let selection = match target {
                                    PauseMenuMouseTarget::Root => &mut pause_selection,
                                    PauseMenuMouseTarget::Confirm if pause_confirming_load => {
                                        &mut pause_load_confirm_selection
                                    }
                                    PauseMenuMouseTarget::Confirm => {
                                        &mut pause_quit_confirm_selection
                                    }
                                    PauseMenuMouseTarget::Slot => &mut pause_slot_selection,
                                    PauseMenuMouseTarget::Sound => &mut pause_sound_selection,
                                    PauseMenuMouseTarget::Controls => &mut pause_controls_selection,
                                    PauseMenuMouseTarget::Display => &mut pause_display_selection,
                                    PauseMenuMouseTarget::NewGameEpisode
                                    | PauseMenuMouseTarget::NewGameDifficulty => {
                                        &mut pause_new_game.as_mut().unwrap().selected
                                    }
                                };
                                if *selection != index {
                                    *selection = index;
                                    queue_mixer_pc_sound(audio, audio_device, &audio_mixer, 5);
                                    let now = Instant::now();
                                    pause_cursor_sound_due =
                                        Some(now + Duration::from_secs_f64(8.0 / 70.0));
                                    pause_cursor_input_due =
                                        Some(now + Duration::from_secs_f64(8.0 / 70.0));
                                    state_changed = true;
                                }
                            }
                        }
                    }
                } else if event_type == SDL_MOUSEBUTTONDOWN || event_type == SDL_MOUSEBUTTONUP {
                    let button_mask = match event.bytes[16] {
                        1 => SDL_MOUSE_LEFT_MASK,
                        2 => SDL_MOUSE_MIDDLE_MASK,
                        3 => SDL_MOUSE_RIGHT_MASK,
                        _ => 0,
                    };
                    if event_type == SDL_MOUSEBUTTONUP {
                        mouse_buttons &= !button_mask;
                    } else if pause_button_capture.is_some_and(|(mouse, _)| mouse)
                        && crate::settings::mouse_input_enabled()
                    {
                        if let Some((true, action)) = pause_button_capture {
                            let physical_button = match event.bytes[16] {
                                1 => Some(0),
                                2 => Some(1),
                                3 => Some(2),
                                _ => None,
                            };
                            if let Some(button) = physical_button {
                                crate::settings::set_mouse_button_action(action, button)?;
                                pause_button_capture = None;
                                runtime_notice = Some(format!(
                                    "{} MOUSE BUTTON UPDATED",
                                    button_action_name(action)
                                ));
                                state_changed = true;
                            }
                        }
                    } else if pause_button_capture.is_some() {
                        // Ignore input from the other device while a binding is captured.
                    } else if !crate::settings::mouse_input_enabled() {
                        // Keep the device toggle authoritative for gameplay
                        // and all in-game mouse menu actions.
                    } else if pause_scores_open && pause_transition.is_none() {
                        pause_scores_open = false;
                        let menu_music =
                            audio.and_then(|archive| archive.music_player_for_track(14));
                        replace_mixer_music(audio_device, &audio_mixer, menu_music);
                        pause_transition = Some(PauseTransition::new(&pixels, false));
                    } else if pause_key_screen_open {
                        if pause_key_screen_ready {
                            pause_key_screen_open = false;
                            pause_key_screen_ready = false;
                            pause_key_clear_controls = true;
                            mouse_buttons |= button_mask;
                            simulation_accumulator = 0.0;
                            previous = Instant::now();
                            if audio_device != 0 {
                                SDL_PauseAudioDevice(audio_device, 0);
                            }
                        }
                    } else if game_paused
                        && help_article_page.is_none()
                        && pause_transition.is_none()
                        && !pause_key_screen_open
                        && !editing_save_name
                    {
                        if event.bytes[16] == 3 {
                            unsafe { push_keydown_scancode(SDL_SCANCODE_ESCAPE as i32) };
                        } else if event.bytes[16] == 1 {
                            if pause_view_size_open {
                                unsafe { push_keydown_scancode(SDL_SCANCODE_RETURN) };
                            } else {
                                let x = i32::from_ne_bytes(event.bytes[20..24].try_into().unwrap());
                                let y = i32::from_ne_bytes(event.bytes[24..28].try_into().unwrap());
                                let target = active_pause_menu_mouse_target(
                                    pause_confirming_load
                                        || pause_confirming_quit
                                        || pause_confirming_new_game
                                        || pause_confirming_application_quit,
                                    pause_new_game.as_ref().map(|menu| menu.step),
                                    pause_new_game_art.as_ref().map_or((0, 0), |art| {
                                        (art.episode_options.len(), art.difficulty_options.len())
                                    }),
                                    pause_slot_mode,
                                    pause_sound_settings_open,
                                    pause_controls_open,
                                    pause_button_mapping,
                                    pause_movement_mapping_open,
                                    pause_control_capture.is_some()
                                        || pause_button_capture.is_some()
                                        || pause_move_key_capture.is_some(),
                                    pause_display_open,
                                    pause_slot_labels.len(),
                                );
                                if let (Some((target, options_len)), Some((x, y))) =
                                    (target, menu_pointer_coordinates(window, renderer, x, y))
                                {
                                    if let Some(index) =
                                        pause_menu_selection_at(target, options_len, x, y)
                                    {
                                        let selection = match target {
                                            PauseMenuMouseTarget::Root => &mut pause_selection,
                                            PauseMenuMouseTarget::Confirm
                                                if pause_confirming_load =>
                                            {
                                                &mut pause_load_confirm_selection
                                            }
                                            PauseMenuMouseTarget::Confirm => {
                                                &mut pause_quit_confirm_selection
                                            }
                                            PauseMenuMouseTarget::Slot => &mut pause_slot_selection,
                                            PauseMenuMouseTarget::Sound => {
                                                &mut pause_sound_selection
                                            }
                                            PauseMenuMouseTarget::Controls => {
                                                &mut pause_controls_selection
                                            }
                                            PauseMenuMouseTarget::Display => {
                                                &mut pause_display_selection
                                            }
                                            PauseMenuMouseTarget::NewGameEpisode
                                            | PauseMenuMouseTarget::NewGameDifficulty => {
                                                &mut pause_new_game.as_mut().unwrap().selected
                                            }
                                        };
                                        *selection = index;
                                        state_changed = true;
                                        unsafe { push_keydown_scancode(SDL_SCANCODE_RETURN) };
                                    }
                                }
                            }
                        }
                    } else if !game_paused
                        && help_article_page.is_none()
                        && pause_transition.is_none()
                        && death_elapsed.is_none()
                        && demo_playback.is_none()
                        && bj_victory.is_none()
                        && !pause_key_screen_open
                        && !level_reveal_pending
                        && !editing_save_name
                    {
                        mouse_buttons |= button_mask;
                    }
                } else if event_type == SDL_TEXTINPUT && editing_save_name {
                    for byte in &event.bytes[12..44] {
                        if *byte == 0 {
                            break;
                        }
                        if (byte.is_ascii_graphic() || *byte == b' ') && save_name_draft.len() < 31
                        {
                            save_name_draft.push(*byte as char);
                        }
                    }
                    runtime_notice = Some(format!("SLOT NAME: {save_name_draft}_"));
                    state_changed = true;
                } else if event_type == SDL_KEYUP {
                    let scancode = i32::from_ne_bytes(event.bytes[16..20].try_into().unwrap());
                    if game_paused
                        && (scancode == SDL_SCANCODE_UP as i32
                            || scancode == SDL_SCANCODE_DOWN as i32
                            || scancode == SDL_SCANCODE_LEFT
                            || scancode == SDL_SCANCODE_RIGHT)
                    {
                        pause_cursor_input_due = None;
                        if pause_navigation_repeat_scancode == Some(scancode) {
                            pause_navigation_repeat_scancode = None;
                            pause_navigation_repeat_due = None;
                        }
                    }
                } else if event_type == SDL_KEYDOWN
                    && (event.bytes[13] == 0
                        || (game_paused
                            && !pause_view_size_open
                            && !pause_scores_open
                            && pause_control_capture.is_none()
                            && pause_button_capture.is_none()
                            && pause_move_key_capture.is_none()
                            && !editing_save_name
                            && [
                                SDL_SCANCODE_UP as i32,
                                SDL_SCANCODE_DOWN as i32,
                                SDL_SCANCODE_LEFT,
                                SDL_SCANCODE_RIGHT,
                            ]
                            .contains(&i32::from_ne_bytes(
                                event.bytes[16..20].try_into().unwrap(),
                            ))))
                {
                    if pause_transition.is_some() {
                        continue;
                    }
                    if demo_playback.is_some() {
                        level_result = Some(LevelResult::DemoAborted);
                        running = false;
                        continue;
                    }
                    if bj_victory.is_some() {
                        continue;
                    }
                    if death_elapsed.is_some() {
                        if death_turn_target.is_none() && !death_fade_pending {
                            death_elapsed = Some(100.0 / 70.0);
                        }
                        continue;
                    }
                    let scancode = i32::from_ne_bytes(event.bytes[16..20].try_into().unwrap());
                    let is_pause_direction = [
                        SDL_SCANCODE_UP as i32,
                        SDL_SCANCODE_DOWN as i32,
                        SDL_SCANCODE_LEFT,
                        SDL_SCANCODE_RIGHT,
                    ]
                    .contains(&scancode);
                    if game_paused
                        && is_pause_direction
                        && !pause_view_size_open
                        && event.bytes[13] == 1
                    {
                        continue;
                    }
                    if game_paused
                        && is_pause_direction
                        && !pause_view_size_open
                        && event.bytes[13] != 1
                    {
                        pause_navigation_repeat_scancode = Some(scancode);
                        pause_navigation_repeat_due =
                            Some(Instant::now() + Duration::from_secs_f64(8.0 / 70.0));
                    }
                    if game_paused
                        && pause_cursor_input_due.is_some_and(|due| Instant::now() < due)
                        && [
                            SDL_SCANCODE_UP as i32,
                            SDL_SCANCODE_DOWN as i32,
                            SDL_SCANCODE_LEFT,
                            SDL_SCANCODE_RIGHT,
                        ]
                        .contains(&scancode)
                    {
                        continue;
                    }
                    if pause_scores_open {
                        pause_scores_open = false;
                        let menu_music =
                            audio.and_then(|archive| archive.music_player_for_track(14));
                        replace_mixer_music(audio_device, &audio_mixer, menu_music);
                        pause_transition = Some(PauseTransition::new(&pixels, false));
                        continue;
                    }
                    if pause_quick_end_overlay || pause_quick_quit_overlay {
                        if matches!(scancode, SDL_SCANCODE_Y | SDL_SCANCODE_N) {
                            pause_quit_confirm_selection =
                                if scancode == SDL_SCANCODE_N { 1 } else { 0 };
                            unsafe { push_keydown_scancode(SDL_SCANCODE_RETURN) };
                            continue;
                        }
                    }
                    if level_reveal_pending {
                        continue;
                    }
                    if let Some(page) = help_article_page {
                        let Some(help) = help_articles else {
                            help_article_page = None;
                            continue;
                        };
                        let layout_delay = Duration::from_secs_f64(
                            help.pages[page].layout_delay_tics() as f64 / 70.0,
                        );
                        if help_page_started.elapsed() < layout_delay {
                            continue;
                        }
                        if matches!(scancode, 41 | SDL_SCANCODE_F1) {
                            help_article_page = None;
                            simulation_accumulator = 0.0;
                            previous = Instant::now();
                            pause_transition = Some(PauseTransition::with_durations(
                                &pixels,
                                true,
                                Duration::from_secs_f64(30.0 / 70.0),
                                Duration::from_secs_f64(30.0 / 70.0),
                            ));
                        } else if matches!(scancode, 80 | 82 | 75) && page > 0 {
                            help_article_page = Some(page - 1);
                            help_page_started = Instant::now();
                        } else if matches!(scancode, 79 | 81 | 78 | 40)
                            && page + 1 < help.pages.len()
                        {
                            help_article_page = Some(page + 1);
                            help_page_started = Instant::now();
                        }
                        continue;
                    }
                    if pause_key_screen_open {
                        if pause_key_screen_ready {
                            pause_key_screen_open = false;
                            pause_key_screen_ready = false;
                            pause_key_clear_controls = true;
                            simulation_accumulator = 0.0;
                            previous = Instant::now();
                            if audio_device != 0 {
                                SDL_PauseAudioDevice(audio_device, 0);
                            }
                        }
                        continue;
                    }
                    if scancode == SDL_SCANCODE_F1
                        && !game_paused
                        && !game_over
                        && !level_complete
                        && !victory
                        && !editing_save_name
                        && !level_reveal_pending
                    {
                        if help_articles.is_some_and(|help| !help.pages.is_empty()) {
                            help_article_page = Some(0);
                            help_page_started = Instant::now();
                            simulation_accumulator = 0.0;
                            previous = Instant::now();
                            mouse_motion_x = 0;
                            mouse_motion_y = 0;
                            mouse_buttons = 0;
                            let _ = SDL_SetRelativeMouseMode(0);
                            pause_transition = Some(PauseTransition::enter_help_articles(&pixels));
                            if audio_device != 0 {
                                SDL_PauseAudioDevice(audio_device, 1);
                            }
                        }
                        continue;
                    }
                    if pause_key_clear_controls {
                        continue;
                    }
                    if scancode == SDL_SCANCODE_PAUSE
                        && !game_paused
                        && !game_over
                        && !level_complete
                        && !victory
                        && pause_transition.is_none()
                        && !editing_save_name
                        && !level_reveal_pending
                    {
                        pause_key_screen_open = true;
                        pause_key_screen_ready = false;
                        pause_key_clear_controls = true;
                        mouse_motion_x = 0;
                        mouse_motion_y = 0;
                        simulation_accumulator = 0.0;
                        previous = Instant::now();
                        if audio_device != 0 {
                            SDL_PauseAudioDevice(audio_device, 1);
                        }
                        continue;
                    }
                    if (SDL_SCANCODE_F2..=SDL_SCANCODE_F4).contains(&scancode)
                        && !game_paused
                        && !game_over
                        && !level_complete
                        && !victory
                        && !editing_save_name
                        && pause_control_capture.is_none()
                        && pause_button_capture.is_none()
                        && pause_move_key_capture.is_none()
                    {
                        game_paused = true;
                        simulation_accumulator = 0.0;
                        previous = Instant::now();
                        let _ = SDL_SetRelativeMouseMode(0);
                        mouse_motion_x = 0;
                        mouse_motion_y = 0;
                        mouse_buttons = 0;
                        if audio_device != 0 {
                            SDL_PauseAudioDevice(audio_device, 1);
                        }
                        match scancode {
                            SDL_SCANCODE_F2 => {
                                pause_slot_mode = Some(false);
                                pause_slot_selection = selected_save_slot;
                                pause_slot_labels = runtime_save_slot_labels();
                                pause_selection = 1;
                            }
                            SDL_SCANCODE_F3 => {
                                pause_slot_mode = Some(true);
                                pause_slot_selection = selected_save_slot;
                                pause_slot_labels = runtime_save_slot_labels();
                                pause_selection = 2;
                            }
                            _ => {
                                pause_sound_settings_open = true;
                                pause_sound_return_to_game = true;
                                pause_sound_selection = 0;
                                pause_selection = 3;
                            }
                        }
                        pause_transition = Some(PauseTransition::enter_control_panel(&pixels));
                        continue;
                    }
                    if (SDL_SCANCODE_F5..=SDL_SCANCODE_F7).contains(&scancode)
                        && !game_paused
                        && !game_over
                        && !level_complete
                        && !victory
                        && !editing_save_name
                        && pause_control_capture.is_none()
                        && pause_button_capture.is_none()
                        && pause_move_key_capture.is_none()
                    {
                        game_paused = true;
                        simulation_accumulator = 0.0;
                        previous = Instant::now();
                        let _ = SDL_SetRelativeMouseMode(0);
                        mouse_motion_x = 0;
                        mouse_motion_y = 0;
                        mouse_buttons = 0;
                        if scancode != SDL_SCANCODE_F7 && audio_device != 0 {
                            SDL_PauseAudioDevice(audio_device, 1);
                        }
                        match scancode {
                            SDL_SCANCODE_F5 => {
                                pause_view_size_original = Some(crate::settings::view_size());
                                pause_view_size_open = true;
                                pause_view_size_repeat_due = None;
                                pause_view_size_return_to_game = true;
                                pause_view_mouse_delta_x = 0;
                                pause_view_mouse_delta_y = 0;
                            }
                            SDL_SCANCODE_F6 => {
                                pause_controls_open = true;
                                pause_controls_return_to_game = true;
                                pause_controls_selection = 0;
                            }
                            _ => {
                                pause_confirming_quit = true;
                                pause_quick_end_overlay = true;
                                pause_quit_confirm_selection = 1;
                                fast_end_requested = true;
                            }
                        }
                        if scancode != SDL_SCANCODE_F7 {
                            pause_transition = Some(PauseTransition::enter_control_panel(&pixels));
                        }
                        continue;
                    }
                    if scancode == SDL_SCANCODE_F10
                        && !game_paused
                        && !game_over
                        && !level_complete
                        && !victory
                        && !editing_save_name
                        && pause_control_capture.is_none()
                        && pause_button_capture.is_none()
                        && pause_move_key_capture.is_none()
                    {
                        game_paused = true;
                        simulation_accumulator = 0.0;
                        previous = Instant::now();
                        let _ = SDL_SetRelativeMouseMode(0);
                        mouse_motion_x = 0;
                        mouse_motion_y = 0;
                        mouse_buttons = 0;
                        let first_random = enemies.next_dos_random();
                        let second_random = enemies.next_dos_random();
                        let message_index = first_random & (0x07 + (second_random & 1));
                        pause_quick_quit_prompt =
                            QUIT_CONFIRMATION_MESSAGES[message_index as usize];
                        pause_confirming_application_quit = true;
                        pause_quick_quit_overlay = true;
                        pause_quit_confirm_selection = 1;
                        continue;
                    }
                    if game_paused
                        && pause_slot_mode.is_some()
                        && scancode == SDL_SCANCODE_F8
                        && !editing_save_name
                    {
                        selected_save_slot = pause_slot_selection;
                        save_path = crate::resources::save_slot_path(selected_save_slot);
                        save_name = crate::savegame::display_name(&save_path)
                            .filter(|name| !name.is_empty())
                            .unwrap_or_else(|| format!("Save Slot {}", selected_save_slot + 1));
                        save_name_draft = if save_name.starts_with("Save Slot ") {
                            String::new()
                        } else {
                            save_name.clone()
                        };
                        editing_save_name = true;
                        SDL_StartTextInput();
                        runtime_notice = Some(format!("SLOT NAME: {save_name_draft}_"));
                        state_changed = true;
                        continue;
                    }
                    if scancode == SDL_SCANCODE_F8
                        && !game_paused
                        && !game_over
                        && !level_complete
                        && !victory
                        && !editing_save_name
                        && pause_control_capture.is_none()
                        && pause_button_capture.is_none()
                        && pause_move_key_capture.is_none()
                    {
                        if save_path.exists() {
                            pause_quicksave_requested = true;
                        } else {
                            game_paused = true;
                            simulation_accumulator = 0.0;
                            previous = Instant::now();
                            let _ = SDL_SetRelativeMouseMode(0);
                            mouse_motion_x = 0;
                            mouse_motion_y = 0;
                            mouse_buttons = 0;
                            if audio_device != 0 {
                                SDL_PauseAudioDevice(audio_device, 1);
                            }
                            pause_slot_mode = Some(false);
                            pause_slot_selection = selected_save_slot;
                            pause_slot_labels = runtime_save_slot_labels();
                            pause_selection = 1;
                            pause_transition = Some(PauseTransition::enter_control_panel(&pixels));
                        }
                        continue;
                    }
                    if scancode == SDL_SCANCODE_F9 as i32
                        && !game_paused
                        && !game_over
                        && !level_complete
                        && !victory
                        && !editing_save_name
                        && pause_control_capture.is_none()
                        && pause_button_capture.is_none()
                        && pause_move_key_capture.is_none()
                    {
                        game_paused = true;
                        simulation_accumulator = 0.0;
                        previous = Instant::now();
                        let _ = SDL_SetRelativeMouseMode(0);
                        mouse_motion_x = 0;
                        mouse_motion_y = 0;
                        mouse_buttons = 0;
                        if audio_device != 0 {
                            SDL_PauseAudioDevice(audio_device, 1);
                        }
                        if save_path.exists() {
                            pause_confirming_load = true;
                            pause_load_confirm_selection = 1;
                        } else {
                            quickload_fallback_open = true;
                            pause_slot_mode = Some(true);
                            pause_slot_selection = selected_save_slot;
                            pause_slot_labels = runtime_save_slot_labels();
                            pause_selection = 2;
                            pause_transition = Some(PauseTransition::enter_control_panel(&pixels));
                        }
                        continue;
                    }
                    if editing_save_name {
                        match scancode {
                            SDL_SCANCODE_RETURN => {
                                save_name = sanitize_save_name(&save_name_draft);
                                if save_name.is_empty() {
                                    save_name = format!("Save Slot {}", selected_save_slot + 1);
                                }
                                editing_save_name = false;
                                SDL_StopTextInput();
                                if save_path.exists() {
                                    match crate::savegame::load::<LevelSaveState>(&save_path) {
                                        Ok(mut saved) => {
                                            saved.save_name = save_name.clone();
                                            match crate::savegame::save(&save_path, &saved) {
                                                Ok(()) => {
                                                    pause_slot_labels[selected_save_slot] = format!(
                                                        "SLOT {}: {}",
                                                        selected_save_slot + 1,
                                                        save_name
                                                            .chars()
                                                            .take(22)
                                                            .collect::<String>()
                                                    );
                                                    runtime_notice = Some(format!(
                                                        "SLOT {} RENAMED: {save_name}",
                                                        selected_save_slot + 1
                                                    ));
                                                }
                                                Err(error) => {
                                                    runtime_notice =
                                                        Some(format!("RENAME FAILED: {error}"));
                                                }
                                            }
                                        }
                                        Err(error) => {
                                            runtime_notice =
                                                Some(format!("RENAME FAILED: {error}"));
                                        }
                                    }
                                } else {
                                    runtime_notice = Some(format!(
                                        "SLOT {} NAME SET FOR NEXT SAVE: {save_name}",
                                        selected_save_slot + 1
                                    ));
                                }
                                state_changed = true;
                            }
                            SDL_SCANCODE_BACKSPACE => {
                                save_name_draft.pop();
                                runtime_notice = Some(format!("SLOT NAME: {save_name_draft}_"));
                                state_changed = true;
                            }
                            value if value == SDL_SCANCODE_ESCAPE as i32 => {
                                editing_save_name = false;
                                SDL_StopTextInput();
                                runtime_notice = Some("SLOT NAME EDIT CANCELLED".into());
                                state_changed = true;
                            }
                            _ => {}
                        }
                    } else if let Some(control_index) = pause_control_capture {
                        if scancode == SDL_SCANCODE_ESCAPE as i32 {
                            pause_control_capture = None;
                        } else if (58..=67).contains(&scancode) {
                            runtime_notice = Some("F1-F10 ARE RESERVED".into());
                            state_changed = true;
                        } else if (1..512).contains(&scancode) {
                            crate::settings::set_control(control_index, scancode as u16)?;
                            pause_control_capture = None;
                            runtime_notice = Some(format!(
                                "{} KEY UPDATED",
                                PAUSE_CONTROL_LABELS[control_index]
                            ));
                            state_changed = true;
                        }
                    } else if let Some(direction) = pause_move_key_capture {
                        if scancode == SDL_SCANCODE_ESCAPE as i32 {
                            pause_move_key_capture = None;
                        } else if (58..=67).contains(&scancode) {
                            runtime_notice = Some("F1-F10 ARE RESERVED".into());
                            state_changed = true;
                        } else if (1..512).contains(&scancode) {
                            crate::settings::set_keyboard_move_key(direction, scancode as u16)?;
                            pause_move_key_capture = None;
                            runtime_notice =
                                Some(format!("{} KEY UPDATED", PAUSE_MOVE_LABELS[direction]));
                            state_changed = true;
                        }
                    } else if pause_button_capture.is_some() {
                        if scancode == SDL_SCANCODE_ESCAPE as i32 {
                            pause_button_capture = None;
                            state_changed = true;
                        }
                    } else {
                        match scancode {
                            value
                                if value == SDL_SCANCODE_ESCAPE as i32
                                    && !game_over
                                    && !level_complete
                                    && !victory
                                    && bj_victory.is_none()
                                    && death_elapsed.is_none() =>
                            {
                                if game_paused {
                                    queue_mixer_pc_sound(audio, audio_device, &audio_mixer, 39);
                                }
                                if pause_confirming_load {
                                    pause_confirming_load = false;
                                    game_paused = false;
                                    simulation_accumulator = 0.0;
                                    previous = Instant::now();
                                    if audio_device != 0 {
                                        SDL_PauseAudioDevice(audio_device, 0);
                                    }
                                    if demo_playback.is_none() {
                                        set_gameplay_mouse_capture();
                                    }
                                } else if pause_view_size_open {
                                    if let Some(original) = pause_view_size_original.take() {
                                        crate::settings::preview_view_size(original)?;
                                    }
                                    pause_view_size_open = false;
                                    pause_view_size_repeat_due = None;
                                    pause_view_mouse_delta_x = 0;
                                    pause_view_mouse_delta_y = 0;
                                    if pause_view_size_return_to_game {
                                        pause_view_size_return_to_game = false;
                                        pause_display_open = false;
                                        game_paused = false;
                                        simulation_accumulator = 0.0;
                                        previous = Instant::now();
                                        pause_transition =
                                            Some(PauseTransition::new(&pixels, true));
                                    } else {
                                        pause_display_open = true;
                                        pause_transition =
                                            Some(PauseTransition::new(&pixels, false));
                                    }
                                    state_changed = true;
                                } else if let Some(step) =
                                    pause_new_game.as_ref().map(|menu| menu.step)
                                {
                                    match step {
                                        PauseNewGameStep::Episode => {
                                            pause_new_game = None;
                                            pause_selection = 7;
                                            pause_transition =
                                                Some(PauseTransition::new(&pixels, false));
                                        }
                                        PauseNewGameStep::Confirm => {
                                            pause_new_game = None;
                                            pause_confirming_new_game = false;
                                            pause_selection = 7;
                                        }
                                        PauseNewGameStep::Difficulty => {
                                            if let Some(menu) = pause_new_game.as_mut() {
                                                menu.step = PauseNewGameStep::Episode;
                                                menu.selected = 0;
                                            }
                                            pause_transition =
                                                Some(PauseTransition::new(&pixels, false));
                                        }
                                    }
                                    state_changed = true;
                                } else if pause_confirming_application_quit {
                                    pause_confirming_application_quit = false;
                                    pause_quick_quit_overlay = false;
                                    game_paused = false;
                                    simulation_accumulator = 0.0;
                                    previous = Instant::now();
                                    if audio_device != 0 {
                                        SDL_PauseAudioDevice(audio_device, 0);
                                    }
                                    set_gameplay_mouse_capture();
                                } else if pause_quick_end_overlay {
                                    pause_quick_end_overlay = false;
                                    pause_confirming_quit = false;
                                    fast_end_requested = false;
                                    game_paused = false;
                                    simulation_accumulator = 0.0;
                                    previous = Instant::now();
                                    set_gameplay_mouse_capture();
                                } else if pause_confirming_new_game {
                                    pause_confirming_new_game = false;
                                    pause_selection = 7;
                                } else if pause_confirming_quit {
                                    pause_confirming_quit = false;
                                    fast_end_requested = false;
                                    pause_selection = 8;
                                } else if game_paused && pause_slot_mode.is_some() {
                                    let loading = pause_slot_mode.take().unwrap();
                                    if loading && quickload_fallback_open {
                                        quickload_fallback_open = false;
                                        game_paused = false;
                                        previous = Instant::now();
                                        pause_transition =
                                            Some(PauseTransition::new(&pixels, true));
                                    } else {
                                        pause_selection = if loading { 2 } else { 1 };
                                        pause_transition =
                                            Some(PauseTransition::new(&pixels, false));
                                    }
                                } else if game_paused && pause_sound_settings_open {
                                    pause_sound_settings_open = false;
                                    if pause_sound_return_to_game {
                                        pause_sound_return_to_game = false;
                                        game_paused = false;
                                        simulation_accumulator = 0.0;
                                        previous = Instant::now();
                                        pause_transition =
                                            Some(PauseTransition::new(&pixels, true));
                                    } else {
                                        pause_selection = 3;
                                        pause_transition =
                                            Some(PauseTransition::new(&pixels, false));
                                    }
                                } else if game_paused
                                    && pause_controls_open
                                    && pause_movement_mapping_open
                                {
                                    pause_movement_mapping_open = false;
                                    pause_controls_selection = crate::settings::CONTROL_COUNT + 5;
                                } else if game_paused
                                    && pause_controls_open
                                    && pause_button_mapping.is_some()
                                {
                                    let mouse = pause_button_mapping.take().unwrap();
                                    pause_controls_selection =
                                        crate::settings::CONTROL_COUNT + if mouse { 3 } else { 4 };
                                } else if game_paused && pause_controls_open {
                                    pause_controls_open = false;
                                    if pause_controls_return_to_game {
                                        pause_controls_return_to_game = false;
                                        game_paused = false;
                                        simulation_accumulator = 0.0;
                                        previous = Instant::now();
                                        pause_transition =
                                            Some(PauseTransition::new(&pixels, true));
                                    } else {
                                        pause_selection = 4;
                                        pause_transition =
                                            Some(PauseTransition::new(&pixels, false));
                                    }
                                } else if game_paused && pause_display_open {
                                    pause_display_open = false;
                                    pause_selection = 5;
                                    pause_transition = Some(PauseTransition::new(&pixels, false));
                                } else if game_paused {
                                    game_paused = false;
                                    pause_transition = Some(PauseTransition::new(&pixels, true));
                                    overwrite_pending = None;
                                    previous = Instant::now();
                                } else {
                                    game_paused = true;
                                    pause_transition =
                                        Some(PauseTransition::enter_control_panel(&pixels));
                                    simulation_accumulator = 0.0;
                                    previous = Instant::now();
                                    let _ = SDL_SetRelativeMouseMode(0);
                                    mouse_motion_x = 0;
                                    mouse_motion_y = 0;
                                    mouse_buttons = 0;
                                    if audio_device != 0 {
                                        SDL_PauseAudioDevice(audio_device, 1);
                                    }
                                }
                            }
                            value
                                if game_paused
                                    && (value == SDL_SCANCODE_UP as i32
                                        || value == SDL_SCANCODE_DOWN as i32
                                        || (pause_view_size_open
                                            && (value == SDL_SCANCODE_LEFT
                                                || value == SDL_SCANCODE_RIGHT))
                                        || (pause_controls_open
                                            && pause_button_mapping.is_none()
                                            && !pause_movement_mapping_open
                                            && pause_controls_selection
                                                == crate::settings::CONTROL_COUNT
                                            && (value == SDL_SCANCODE_LEFT
                                                || value == SDL_SCANCODE_RIGHT))) =>
                            {
                                let adjusting_mouse_sensitivity = pause_controls_open
                                    && pause_button_mapping.is_none()
                                    && !pause_movement_mapping_open
                                    && pause_controls_selection == crate::settings::CONTROL_COUNT
                                    && (value == SDL_SCANCODE_LEFT || value == SDL_SCANCODE_RIGHT);
                                if adjusting_mouse_sensitivity {
                                    let sensitivity = crate::settings::mouse_sensitivity();
                                    let next = if value == SDL_SCANCODE_RIGHT {
                                        sensitivity
                                            .saturating_add(1)
                                            .min(crate::settings::MAX_MOUSE_SENSITIVITY)
                                    } else {
                                        sensitivity
                                            .saturating_sub(1)
                                            .max(crate::settings::MIN_MOUSE_SENSITIVITY)
                                    };
                                    if next != sensitivity {
                                        crate::settings::set_mouse_sensitivity(next)?;
                                    }
                                } else if pause_view_size_open {
                                    let increasing = value == SDL_SCANCODE_UP as i32
                                        || value == SDL_SCANCODE_RIGHT;
                                    let current = crate::settings::view_size();
                                    let next = if increasing {
                                        current
                                            .saturating_add(1)
                                            .min(crate::settings::MAX_VIEW_SIZE)
                                    } else {
                                        current
                                            .saturating_sub(1)
                                            .max(crate::settings::MIN_VIEW_SIZE)
                                    };
                                    crate::settings::preview_view_size(next)?;
                                    pause_view_size_repeat_due =
                                        Some(Instant::now() + Duration::from_secs_f64(10.0 / 70.0));
                                } else if pause_confirming_load {
                                    pause_load_confirm_selection = 1 - pause_load_confirm_selection;
                                } else if pause_confirming_new_game
                                    || pause_confirming_quit
                                    || pause_confirming_application_quit
                                {
                                    pause_quit_confirm_selection = 1 - pause_quit_confirm_selection;
                                } else if let Some(menu) = pause_new_game.as_mut() {
                                    let option_count = match menu.step {
                                        PauseNewGameStep::Episode => pause_new_game_art
                                            .as_ref()
                                            .map_or(0, |art| art.episode_options.len()),
                                        PauseNewGameStep::Difficulty => pause_new_game_art
                                            .as_ref()
                                            .map_or(0, |art| art.difficulty_options.len()),
                                        PauseNewGameStep::Confirm => 0,
                                    };
                                    if option_count > 0 {
                                        let direction = if value == SDL_SCANCODE_DOWN as i32 {
                                            1
                                        } else {
                                            option_count - 1
                                        };
                                        menu.selected = (menu.selected + direction) % option_count;
                                    }
                                } else if pause_slot_mode.is_some() {
                                    let direction = if value == SDL_SCANCODE_DOWN as i32 {
                                        1
                                    } else {
                                        crate::resources::SAVE_SLOT_COUNT - 1
                                    };
                                    pause_slot_selection = (pause_slot_selection + direction)
                                        % crate::resources::SAVE_SLOT_COUNT;
                                } else if pause_sound_settings_open {
                                    let direction = if value == SDL_SCANCODE_DOWN as i32 {
                                        1
                                    } else {
                                        7
                                    };
                                    pause_sound_selection = (pause_sound_selection + direction) % 8;
                                } else if pause_controls_open {
                                    let count = if pause_movement_mapping_open {
                                        crate::settings::MOVE_DIRECTION_COUNT + 1
                                    } else {
                                        pause_button_mapping.map_or(
                                            crate::settings::CONTROL_COUNT
                                                + PAUSE_CONTROL_SETTINGS_COUNT,
                                            |mouse| {
                                                (if mouse {
                                                    MOUSE_BUTTON_ACTIONS.len()
                                                } else {
                                                    CONTROLLER_BUTTON_ACTIONS.len()
                                                }) + 1
                                            },
                                        )
                                    };
                                    let direction = if value == SDL_SCANCODE_DOWN as i32 {
                                        1
                                    } else {
                                        count - 1
                                    };
                                    pause_controls_selection =
                                        (pause_controls_selection + direction) % count;
                                } else if pause_display_open {
                                    let direction = if value == SDL_SCANCODE_DOWN as i32 {
                                        1
                                    } else {
                                        4
                                    };
                                    pause_display_selection =
                                        (pause_display_selection + direction) % 8;
                                } else {
                                    let direction = if value == SDL_SCANCODE_DOWN as i32 {
                                        1
                                    } else {
                                        8
                                    };
                                    pause_selection = (pause_selection + direction) % 9;
                                }
                                if pause_view_size_open {
                                    queue_mixer_pc_sound(audio, audio_device, &audio_mixer, 0);
                                    pause_view_size_repeat_due =
                                        Some(Instant::now() + Duration::from_secs_f64(10.0 / 70.0));
                                } else {
                                    queue_mixer_pc_sound(audio, audio_device, &audio_mixer, 5);
                                }
                                if !pause_view_size_open && !adjusting_mouse_sensitivity {
                                    let now = Instant::now();
                                    pause_cursor_sound_due =
                                        Some(now + Duration::from_secs_f64(8.0 / 70.0));
                                    pause_cursor_input_due =
                                        Some(now + Duration::from_secs_f64(8.0 / 70.0));
                                }
                                state_changed = true;
                            }
                            SDL_SCANCODE_RETURN if game_paused => {
                                let accept_sound = if (pause_quick_end_overlay
                                    || pause_quick_quit_overlay)
                                    && pause_quit_confirm_selection != 0
                                {
                                    39
                                } else {
                                    32
                                };
                                if !pause_sound_settings_open {
                                    queue_mixer_pc_sound(
                                        audio,
                                        audio_device,
                                        &audio_mixer,
                                        accept_sound,
                                    );
                                }
                                if pause_view_size_open {
                                    let view_size_changed =
                                        pause_view_size_original.is_some_and(|original| {
                                            original != crate::settings::view_size()
                                        });
                                    crate::settings::save_view_size()?;
                                    if view_size_changed {
                                        queue_mixer_pc_sound(audio, audio_device, &audio_mixer, 32);
                                    }
                                    pause_view_size_open = false;
                                    pause_view_size_repeat_due = None;
                                    pause_view_size_original = None;
                                    pause_view_mouse_delta_x = 0;
                                    pause_view_mouse_delta_y = 0;
                                    if pause_view_size_return_to_game {
                                        pause_view_size_return_to_game = false;
                                        pause_display_open = false;
                                        game_paused = false;
                                        simulation_accumulator = 0.0;
                                        previous = Instant::now();
                                        pause_transition =
                                            Some(PauseTransition::new(&pixels, true));
                                    } else {
                                        pause_transition =
                                            Some(PauseTransition::new(&pixels, false));
                                    }
                                    state_changed = true;
                                } else if pause_confirming_load {
                                    if pause_load_confirm_selection == 0 {
                                        pause_confirming_load = false;
                                        game_paused = false;
                                        pause_load_requested = true;
                                        quickload_audio_resume = true;
                                    } else {
                                        pause_confirming_load = false;
                                        game_paused = false;
                                        simulation_accumulator = 0.0;
                                        previous = Instant::now();
                                        if audio_device != 0 {
                                            SDL_PauseAudioDevice(audio_device, 0);
                                        }
                                        set_gameplay_mouse_capture();
                                    }
                                } else if pause_confirming_new_game
                                    || pause_confirming_quit
                                    || pause_confirming_application_quit
                                {
                                    if pause_quit_confirm_selection == 0 {
                                        if pause_confirming_new_game {
                                            pause_confirming_new_game = false;
                                            if let Some(menu) = pause_new_game.as_mut() {
                                                menu.step = PauseNewGameStep::Difficulty;
                                                menu.selected = 0;
                                            }
                                            pause_transition =
                                                Some(PauseTransition::new(&pixels, false));
                                            state_changed = true;
                                        } else if pause_confirming_application_quit {
                                            level_result = Some(LevelResult::Quit);
                                            pause_confirming_application_quit = false;
                                            if pause_quick_quit_overlay {
                                                pause_quick_quit_overlay = false;
                                                game_paused = true;
                                                pause_transition = Some(
                                                    PauseTransition::exit_application(&pixels),
                                                );
                                                if audio_device != 0 {
                                                    SDL_PauseAudioDevice(audio_device, 1);
                                                }
                                            } else {
                                                pause_quick_quit_overlay = false;
                                                game_paused = false;
                                                running = false;
                                            }
                                        } else if fast_end_requested {
                                            lives = 0;
                                            game_over = true;
                                            level_result = Some(LevelResult::GameOver);
                                            game_paused = false;
                                            running = false;
                                            fast_end_requested = false;
                                            pause_quick_end_overlay = false;
                                        } else {
                                            health = 0;
                                            lives = 0;
                                            game_paused = false;
                                            previous = Instant::now();
                                            if audio_device != 0 {
                                                SDL_PauseAudioDevice(audio_device, 0);
                                            }
                                            death_elapsed = Some(0.0);
                                            death_fade_pending = true;
                                        }
                                        overwrite_pending = None;
                                        state_changed = true;
                                    } else {
                                        if pause_confirming_application_quit {
                                            pause_confirming_application_quit = false;
                                            pause_quick_quit_overlay = false;
                                            game_paused = false;
                                            simulation_accumulator = 0.0;
                                            previous = Instant::now();
                                            if audio_device != 0 {
                                                SDL_PauseAudioDevice(audio_device, 0);
                                            }
                                            set_gameplay_mouse_capture();
                                        } else if pause_confirming_new_game {
                                            pause_confirming_new_game = false;
                                            pause_new_game = None;
                                            pause_selection = 7;
                                            pause_transition =
                                                Some(PauseTransition::new(&pixels, false));
                                        } else if pause_quick_end_overlay {
                                            pause_confirming_quit = false;
                                            pause_quick_end_overlay = false;
                                            fast_end_requested = false;
                                            game_paused = false;
                                            simulation_accumulator = 0.0;
                                            previous = Instant::now();
                                            set_gameplay_mouse_capture();
                                        } else {
                                            pause_confirming_quit = false;
                                            fast_end_requested = false;
                                            pause_selection = 8;
                                        }
                                    }
                                } else if let Some(menu) = pause_new_game.as_mut() {
                                    match menu.step {
                                        PauseNewGameStep::Episode => {
                                            menu.episode = menu.selected;
                                            menu.selected = 1;
                                            menu.step = PauseNewGameStep::Confirm;
                                            pause_confirming_new_game = true;
                                            pause_quit_confirm_selection = 1;
                                            pause_transition =
                                                Some(PauseTransition::new(&pixels, false));
                                        }
                                        PauseNewGameStep::Difficulty => {
                                            let episode = menu.episode;
                                            let selection = menu.selected;
                                            let difficulty = [
                                                Difficulty::Baby,
                                                Difficulty::Easy,
                                                Difficulty::Medium,
                                                Difficulty::Hard,
                                            ][selection];
                                            level_result = Some(LevelResult::NewGame {
                                                map_number: episode * 10,
                                                difficulty,
                                            });
                                            pause_new_game = None;
                                            game_paused = false;
                                            running = false;
                                        }
                                        PauseNewGameStep::Confirm => {}
                                    }
                                    state_changed = true;
                                } else if let Some(loading) = pause_slot_mode {
                                    let previous_slot = selected_save_slot;
                                    selected_save_slot = pause_slot_selection;
                                    save_path =
                                        crate::resources::save_slot_path(selected_save_slot);
                                    save_name = crate::savegame::display_name(&save_path)
                                        .filter(|name| !name.is_empty())
                                        .unwrap_or_else(|| {
                                            format!("Save Slot {}", selected_save_slot + 1)
                                        });
                                    if previous_slot != selected_save_slot {
                                        overwrite_pending = None;
                                    }
                                    if loading && !save_path.exists() {
                                        runtime_notice = Some(format!(
                                            "SLOT {} IS EMPTY",
                                            selected_save_slot + 1
                                        ));
                                    } else {
                                        pause_load_requested = loading;
                                        pause_save_requested = !loading;
                                        pause_slot_mode = None;
                                        if loading && quickload_fallback_open {
                                            quickload_fallback_open = false;
                                            game_paused = false;
                                            quickload_audio_resume = true;
                                        }
                                    }
                                    state_changed = true;
                                } else if pause_sound_settings_open {
                                    match pause_sound_selection {
                                        0 => {
                                            if crate::settings::sound_effects_mode()
                                                != crate::settings::SoundEffectsMode::Off
                                            {
                                                crate::settings::set_sound_effects_mode(
                                                    crate::settings::SoundEffectsMode::Off,
                                                )?;
                                                stop_mixer_synthesized_effect(
                                                    audio_device,
                                                    &audio_mixer,
                                                );
                                            }
                                        }
                                        1 => {
                                            if crate::settings::sound_effects_mode()
                                                != crate::settings::SoundEffectsMode::Pc
                                            {
                                                crate::settings::set_sound_effects_mode(
                                                    crate::settings::SoundEffectsMode::Pc,
                                                )?;
                                                stop_mixer_synthesized_effect(
                                                    audio_device,
                                                    &audio_mixer,
                                                );
                                                queue_mixer_pc_sound(
                                                    audio,
                                                    audio_device,
                                                    &audio_mixer,
                                                    32,
                                                );
                                            }
                                        }
                                        2 => {
                                            if crate::settings::sound_effects_mode()
                                                != crate::settings::SoundEffectsMode::Adlib
                                            {
                                                crate::settings::set_sound_effects_mode(
                                                    crate::settings::SoundEffectsMode::Adlib,
                                                )?;
                                                stop_mixer_synthesized_effect(
                                                    audio_device,
                                                    &audio_mixer,
                                                );
                                                queue_mixer_pc_sound(
                                                    audio,
                                                    audio_device,
                                                    &audio_mixer,
                                                    32,
                                                );
                                            }
                                        }
                                        3 => {
                                            if crate::settings::digitized_effects_enabled() {
                                                crate::settings::set_digitized_effects_enabled(
                                                    false,
                                                )?;
                                                stop_mixer_digitized_effect(
                                                    audio_device,
                                                    &audio_mixer,
                                                );
                                            }
                                        }
                                        4 => {
                                            if !crate::settings::digitized_effects_enabled() {
                                                crate::settings::set_digitized_effects_enabled(
                                                    true,
                                                )?;
                                                queue_mixer_pc_sound(
                                                    audio,
                                                    audio_device,
                                                    &audio_mixer,
                                                    32,
                                                );
                                            }
                                        }
                                        5 => {
                                            let music_was_enabled =
                                                crate::settings::music_enabled();
                                            crate::settings::set_music_enabled(false)?;
                                            if music_was_enabled {
                                                stop_mixer_music(audio_device, &audio_mixer);
                                                queue_mixer_pc_sound(
                                                    audio,
                                                    audio_device,
                                                    &audio_mixer,
                                                    32,
                                                );
                                            }
                                        }
                                        6 => {
                                            let music_was_enabled =
                                                crate::settings::music_enabled();
                                            crate::settings::set_music_enabled(true)?;
                                            if !music_was_enabled {
                                                let menu_music = audio.and_then(|archive| {
                                                    archive.music_player_for_track(14)
                                                });
                                                replace_mixer_music(
                                                    audio_device,
                                                    &audio_mixer,
                                                    menu_music,
                                                );
                                                queue_mixer_pc_sound(
                                                    audio,
                                                    audio_device,
                                                    &audio_mixer,
                                                    32,
                                                );
                                            }
                                        }
                                        _ => {
                                            queue_mixer_pc_sound(
                                                audio,
                                                audio_device,
                                                &audio_mixer,
                                                39,
                                            );
                                            pause_sound_settings_open = false;
                                            if pause_sound_return_to_game {
                                                pause_sound_return_to_game = false;
                                                game_paused = false;
                                                simulation_accumulator = 0.0;
                                                previous = Instant::now();
                                                pause_transition =
                                                    Some(PauseTransition::new(&pixels, true));
                                            } else {
                                                pause_transition =
                                                    Some(PauseTransition::new(&pixels, false));
                                            }
                                        }
                                    }
                                    state_changed = true;
                                } else if pause_controls_open {
                                    if pause_movement_mapping_open {
                                        if pause_controls_selection
                                            < crate::settings::MOVE_DIRECTION_COUNT
                                        {
                                            pause_move_key_capture = Some(pause_controls_selection);
                                        } else {
                                            pause_movement_mapping_open = false;
                                            pause_controls_selection =
                                                crate::settings::CONTROL_COUNT + 5;
                                        }
                                    } else if let Some(mouse) = pause_button_mapping {
                                        let actions = if mouse {
                                            &MOUSE_BUTTON_ACTIONS[..]
                                        } else {
                                            &CONTROLLER_BUTTON_ACTIONS[..]
                                        };
                                        if let Some(&action) = actions.get(pause_controls_selection)
                                        {
                                            pause_button_capture = Some((mouse, action));
                                        } else {
                                            pause_button_mapping = None;
                                            pause_controls_selection =
                                                crate::settings::CONTROL_COUNT
                                                    + if mouse { 3 } else { 4 };
                                        }
                                    } else if pause_controls_selection
                                        < crate::settings::CONTROL_COUNT
                                    {
                                        pause_control_capture = Some(pause_controls_selection);
                                    } else if pause_controls_selection
                                        == crate::settings::CONTROL_COUNT + 1
                                    {
                                        crate::settings::set_mouse_input_enabled(
                                            !crate::settings::mouse_input_enabled(),
                                        )?;
                                    } else if pause_controls_selection
                                        == crate::settings::CONTROL_COUNT + 2
                                    {
                                        crate::settings::set_controller_input_enabled(
                                            !crate::settings::controller_input_enabled(),
                                        )?;
                                    } else if pause_controls_selection
                                        == crate::settings::CONTROL_COUNT + 3
                                    {
                                        pause_button_mapping = Some(true);
                                        pause_controls_selection = 0;
                                    } else if pause_controls_selection
                                        == crate::settings::CONTROL_COUNT + 4
                                    {
                                        pause_button_mapping = Some(false);
                                        pause_controls_selection = 0;
                                    } else if pause_controls_selection
                                        == crate::settings::CONTROL_COUNT + 5
                                    {
                                        pause_movement_mapping_open = true;
                                        pause_controls_selection = 0;
                                    } else if pause_controls_selection
                                        == crate::settings::CONTROL_COUNT + 6
                                    {
                                        crate::settings::reset_controls()?;
                                        runtime_notice = Some("DEFAULT CONTROLS RESTORED".into());
                                    } else if pause_controls_selection
                                        == crate::settings::CONTROL_COUNT + 7
                                    {
                                        pause_controls_open = false;
                                        if pause_controls_return_to_game {
                                            pause_controls_return_to_game = false;
                                            game_paused = false;
                                            simulation_accumulator = 0.0;
                                            previous = Instant::now();
                                            pause_transition =
                                                Some(PauseTransition::new(&pixels, true));
                                        } else {
                                            pause_transition =
                                                Some(PauseTransition::new(&pixels, false));
                                        }
                                    }
                                    state_changed = true;
                                } else if pause_display_open {
                                    match pause_display_selection {
                                        0 => {
                                            let fullscreen = !crate::settings::fullscreen_enabled();
                                            let flags = if fullscreen {
                                                SDL_WINDOW_FULLSCREEN_DESKTOP
                                            } else {
                                                0
                                            };
                                            if SDL_SetWindowFullscreen(window, flags) == 0 {
                                                crate::settings::set_fullscreen(fullscreen)?;
                                                if !fullscreen {
                                                    let (width, height) = window_size();
                                                    SDL_SetWindowSize(window, width, height);
                                                }
                                                runtime_notice = Some(format!(
                                                    "FULLSCREEN: {}",
                                                    if fullscreen { "ON" } else { "OFF" }
                                                ));
                                            } else {
                                                runtime_notice =
                                                    Some("FULLSCREEN CHANGE FAILED".into());
                                            }
                                        }
                                        1 => {
                                            let current = crate::settings::window_scale();
                                            let next = if current >= 4 { 2 } else { current + 1 };
                                            crate::settings::set_window_scale(next)?;
                                            let (width, height) = window_size();
                                            if !crate::settings::fullscreen_enabled() {
                                                SDL_SetWindowSize(window, width, height);
                                            }
                                            runtime_notice = Some(format!(
                                                "WINDOW SIZE: {} X {}",
                                                width, height
                                            ));
                                        }
                                        2 => {
                                            pause_view_size_original =
                                                Some(crate::settings::view_size());
                                            pause_view_size_open = true;
                                            pause_view_size_repeat_due = None;
                                            pause_view_size_return_to_game = false;
                                            pause_view_mouse_delta_x = 0;
                                            pause_view_mouse_delta_y = 0;
                                            pause_transition =
                                                Some(PauseTransition::new(&pixels, false));
                                        }
                                        3 => {
                                            crate::settings::set_world_renderer(
                                                crate::settings::world_renderer().next(),
                                            )?;
                                        }
                                        4 => {
                                            crate::settings::set_render_resolution(
                                                crate::settings::render_resolution().next(),
                                            )?;
                                        }
                                        5 => {
                                            crate::settings::set_show_historical_symbols(
                                                !crate::settings::show_historical_symbols(),
                                            )?;
                                        }
                                        _ => {
                                            pause_display_open = false;
                                            pause_transition =
                                                Some(PauseTransition::new(&pixels, false));
                                        }
                                    }
                                    state_changed = true;
                                } else {
                                    match pause_selection {
                                        0 => {
                                            game_paused = false;
                                            pause_transition =
                                                Some(PauseTransition::new(&pixels, true));
                                            overwrite_pending = None;
                                            previous = Instant::now();
                                        }
                                        1 | 2 => {
                                            pause_slot_mode = Some(pause_selection == 2);
                                            pause_slot_selection = selected_save_slot;
                                            pause_slot_labels = runtime_save_slot_labels();
                                            pause_transition =
                                                Some(PauseTransition::new(&pixels, false));
                                            state_changed = true;
                                        }
                                        3 => {
                                            pause_sound_settings_open = true;
                                            pause_sound_return_to_game = false;
                                            pause_sound_selection = 0;
                                            pause_transition =
                                                Some(PauseTransition::new(&pixels, false));
                                            state_changed = true;
                                        }
                                        4 => {
                                            pause_controls_open = true;
                                            pause_controls_return_to_game = false;
                                            pause_controls_selection = 0;
                                            pause_transition =
                                                Some(PauseTransition::new(&pixels, false));
                                            state_changed = true;
                                        }
                                        5 => {
                                            pause_display_open = true;
                                            pause_display_selection = 0;
                                            pause_transition =
                                                Some(PauseTransition::new(&pixels, false));
                                            state_changed = true;
                                        }
                                        6 => {
                                            pause_scores_open = true;
                                            pause_cursor_sound_due = None;
                                            pause_cursor_input_due = None;
                                            pause_transition =
                                                Some(PauseTransition::enter_high_scores(&pixels));
                                            state_changed = true;
                                        }
                                        7 => {
                                            if let Some(art) = pause_new_game_art.as_ref() {
                                                let initial_step = if art.episode_options.len() > 1
                                                {
                                                    PauseNewGameStep::Episode
                                                } else {
                                                    PauseNewGameStep::Confirm
                                                };
                                                pause_new_game = Some(PauseNewGameSelection {
                                                    step: initial_step,
                                                    episode: 0,
                                                    selected: 0,
                                                });
                                                pause_confirming_new_game =
                                                    initial_step == PauseNewGameStep::Confirm;
                                                pause_quit_confirm_selection = 1;
                                                pause_transition =
                                                    Some(PauseTransition::new(&pixels, false));
                                            } else {
                                                runtime_notice =
                                                    Some("NEW GAME DATA IS UNAVAILABLE".into());
                                            }
                                            state_changed = true;
                                        }
                                        _ => {
                                            pause_confirming_quit = true;
                                            pause_quit_confirm_selection = 1;
                                            fast_end_requested = false;
                                            state_changed = true;
                                        }
                                    }
                                }
                            }
                            _ if game_paused => {}
                            _ if enemies.is_boss_death_interstitial_active() => {
                                enemies.skip_boss_death_interstitial();
                            }
                            _ => {}
                        }
                    }
                }
            }
            if let Some(due) = pause_cursor_sound_due {
                if !game_paused || pause_view_size_open || pause_transition.is_some() {
                    pause_cursor_sound_due = None;
                    pause_cursor_input_due = None;
                } else if Instant::now() >= due {
                    queue_mixer_pc_sound(audio, audio_device, &audio_mixer, 4);
                    pause_cursor_sound_due = None;
                }
            }
            if pause_cursor_input_due.is_some_and(|due| Instant::now() >= due) {
                pause_cursor_input_due = None;
            }
            let mut keyboard_key_count = 0;
            let keys = SDL_GetKeyboardState(&mut keyboard_key_count);
            let pause_repeat_allowed = game_paused
                && !pause_key_screen_open
                && !pause_view_size_open
                && !pause_scores_open
                && help_article_page.is_none()
                && pause_transition.is_none()
                && pause_control_capture.is_none()
                && pause_button_capture.is_none()
                && pause_move_key_capture.is_none()
                && !editing_save_name;
            if !pause_repeat_allowed {
                pause_navigation_repeat_scancode = None;
                pause_navigation_repeat_due = None;
            } else if let (Some(scancode), Some(due)) = (
                pause_navigation_repeat_scancode,
                pause_navigation_repeat_due,
            ) {
                if Instant::now() >= due {
                    let is_down = !keys.is_null()
                        && scancode >= 0
                        && (scancode as usize) < keyboard_key_count.max(0) as usize
                        && unsafe { *keys.add(scancode as usize) != 0 };
                    if is_down {
                        unsafe { push_repeated_keydown_scancode(scancode) };
                        pause_navigation_repeat_due =
                            Some(Instant::now() + Duration::from_secs_f64(8.0 / 70.0));
                    } else {
                        pause_navigation_repeat_scancode = None;
                        pause_navigation_repeat_due = None;
                    }
                }
            }
            let keyboard_released = keys.is_null()
                || (0..keyboard_key_count.max(0) as usize)
                    .all(|index| unsafe { *keys.add(index) == 0 });
            if pause_key_screen_open && keyboard_released && mouse_buttons == 0 {
                pause_key_screen_ready = true;
            }
            if pause_key_clear_controls && keyboard_released && mouse_buttons == 0 {
                pause_key_clear_controls = false;
            }
            let pressed = |key: usize| {
                !keys.is_null()
                    && key < keyboard_key_count.max(0) as usize
                    && unsafe { *keys.add(key) != 0 }
            };
            if pause_view_size_open {
                let held_direction = [
                    SDL_SCANCODE_UP as i32,
                    SDL_SCANCODE_RIGHT,
                    SDL_SCANCODE_DOWN as i32,
                    SDL_SCANCODE_LEFT,
                ]
                .into_iter()
                .find(|&scancode| {
                    pressed(scancode as usize)
                        || (crate::settings::controller_input_enabled()
                            && controller.direction_held(scancode))
                });
                if let Some(scancode) = held_direction {
                    let now = Instant::now();
                    if pause_view_size_repeat_due.is_none() {
                        pause_view_size_repeat_due =
                            Some(now + Duration::from_secs_f64(10.0 / 70.0));
                    } else if pause_view_size_repeat_due.is_some_and(|due| now >= due) {
                        let increasing =
                            scancode == SDL_SCANCODE_UP as i32 || scancode == SDL_SCANCODE_RIGHT;
                        let current = crate::settings::view_size();
                        let next = if increasing {
                            current
                                .saturating_add(1)
                                .min(crate::settings::MAX_VIEW_SIZE)
                        } else {
                            current
                                .saturating_sub(1)
                                .max(crate::settings::MIN_VIEW_SIZE)
                        };
                        crate::settings::preview_view_size(next)?;
                        queue_mixer_pc_sound(audio, audio_device, &audio_mixer, 0);
                        pause_view_size_repeat_due =
                            Some(now + Duration::from_secs_f64(10.0 / 70.0));
                        state_changed = true;
                    }
                } else {
                    pause_view_size_repeat_due = None;
                }
            } else {
                pause_view_size_repeat_due = None;
            }
            let key_bindings = crate::settings::get_controls();
            let keyboard_move_bindings = crate::settings::get_keyboard_move_bindings();
            let controller_controls = if crate::settings::controller_input_enabled() {
                controller.controls()
            } else {
                ControllerInput::default()
            };
            let control_pressed = |action: usize| {
                controller_controls
                    .actions
                    .get(action)
                    .copied()
                    .unwrap_or(false)
                    || key_bindings.scancodes.get(action).is_some_and(|keys| {
                        keys.iter().any(|&key| key != 0 && pressed(key as usize))
                    })
            };
            let keyboard_move_pressed = |direction: usize| {
                keyboard_move_bindings
                    .scancodes
                    .get(direction)
                    .is_some_and(|&key| key != 0 && pressed(key as usize))
            };
            let controls_enabled = pause_transition.is_none()
                && demo_playback.is_none()
                && death_elapsed.is_none()
                && bj_victory.is_none()
                && !pause_key_screen_open
                && !pause_key_clear_controls
                && !editing_save_name
                && !game_paused
                && help_article_page.is_none();
            let save_triggered = pause_save_requested || pause_quicksave_requested;
            let load_pressed = pause_load_requested || pending_restore.is_some();
            if save_triggered {
                if save_path.exists()
                    && overwrite_pending != Some(selected_save_slot)
                    && !pause_quicksave_requested
                {
                    overwrite_pending = Some(selected_save_slot);
                    runtime_notice = Some(if pause_save_requested {
                        format!(
                            "SLOT {} EXISTS; SELECT SAVE GAME AGAIN TO OVERWRITE",
                            selected_save_slot + 1
                        )
                    } else {
                        format!(
                            "SLOT {} EXISTS; PRESS F5 AGAIN TO OVERWRITE",
                            selected_save_slot + 1
                        )
                    });
                    state_changed = true;
                } else {
                    overwrite_pending = None;
                    let minimum_visible_until = Instant::now() + Duration::from_millis(350);
                    present_pause_io_progress(
                        renderer,
                        texture,
                        &mut pixels,
                        font,
                        pause_sound_pictures,
                        false,
                        0,
                    )?;
                    let mut saved_campaign = campaign.clone();
                    saved_campaign.health = health;
                    saved_campaign.arsenal = arsenal.clone();
                    saved_campaign.score = score;
                    saved_campaign.next_extra_life_score = next_extra_life_score;
                    saved_campaign.kills = kills;
                    saved_campaign.lives = lives;
                    saved_campaign.keys_owned = keys_owned;
                    let state = LevelSaveState {
                        map_number: map.number,
                        source_map_signature: source_map_signature(&initial_map),
                        data_variant: data_variant.to_owned(),
                        app_version: env!("CARGO_PKG_VERSION").to_owned(),
                        save_name: save_name.clone(),
                        difficulty,
                        map: map.clone(),
                        doors: doors.clone(),
                        item_pickups: item_pickups.clone(),
                        active_actors: active_actors.clone(),
                        enemies: enemies.clone(),
                        pushwalls: pushwalls.clone(),
                        player,
                        campaign: saved_campaign,
                        weapon_attack,
                        keys_owned,
                        health,
                        arsenal: arsenal.clone(),
                        score,
                        next_extra_life_score,
                        kills,
                        lives,
                        level_kills,
                        treasures_found,
                        elapsed_seconds: level_tics as f64 / 70.0,
                        simulation_accumulator,
                    };
                    let mut progress_frame = 0;
                    match crate::savegame::save_with_progress(&save_path, &state, || {
                        progress_frame ^= 1;
                        present_pause_io_progress(
                            renderer,
                            texture,
                            &mut pixels,
                            font,
                            pause_sound_pictures,
                            false,
                            progress_frame,
                        )
                    }) {
                        Ok(()) => {
                            pause_slot_labels[selected_save_slot] = format!(
                                "SLOT {}: {}",
                                selected_save_slot + 1,
                                save_name.chars().take(22).collect::<String>()
                            );
                            runtime_notice = Some(format!(
                                "SLOT {} SAVED: {save_name}",
                                selected_save_slot + 1
                            ))
                        }
                        Err(error) => runtime_notice = Some(format!("SAVE FAILED: {error}")),
                    }
                    hold_pause_io_progress_visible(
                        renderer,
                        texture,
                        &mut pixels,
                        font,
                        pause_sound_pictures,
                        false,
                        &mut progress_frame,
                        minimum_visible_until,
                    )?;
                    state_changed = true;
                }
            }
            if load_pressed && !load_was_pressed {
                let minimum_visible_until = Instant::now() + Duration::from_millis(350);
                let loading_card_shown = pending_restore.is_some() || save_path.exists();
                if loading_card_shown {
                    present_pause_io_progress(
                        renderer,
                        texture,
                        &mut pixels,
                        font,
                        pause_sound_pictures,
                        true,
                        0,
                    )?;
                }
                let mut progress_frame = 0;
                let load_result = match pending_restore.take() {
                    Some(saved) => Ok(saved),
                    None => {
                        crate::savegame::load_with_progress::<LevelSaveState>(&save_path, || {
                            progress_frame ^= 1;
                            present_pause_io_progress(
                                renderer,
                                texture,
                                &mut pixels,
                                font,
                                pause_sound_pictures,
                                true,
                                progress_frame,
                            )
                        })
                    }
                };
                if loading_card_shown {
                    hold_pause_io_progress_visible(
                        renderer,
                        texture,
                        &mut pixels,
                        font,
                        pause_sound_pictures,
                        true,
                        &mut progress_frame,
                        minimum_visible_until,
                    )?;
                }
                match load_result {
                    Ok(saved)
                        if saved.is_compatible_with(
                            &initial_map,
                            &initial_doors,
                            difficulty,
                            data_variant,
                        ) =>
                    {
                        save_name = if saved.save_name.is_empty() {
                            format!("Save Slot {}", selected_save_slot + 1)
                        } else {
                            saved.save_name.clone()
                        };
                        let LevelSaveState {
                            map: saved_map,
                            doors: saved_doors,
                            item_pickups: saved_pickups,
                            active_actors: saved_actors,
                            enemies: saved_enemies,
                            pushwalls: saved_pushwalls,
                            player: saved_player,
                            campaign: saved_campaign,
                            weapon_attack: saved_weapon_attack,
                            keys_owned: saved_keys,
                            health: saved_health,
                            arsenal: saved_arsenal,
                            score: saved_score,
                            next_extra_life_score: saved_next_extra_life_score,
                            kills: saved_kills,
                            lives: saved_lives,
                            level_kills: saved_level_kills,
                            treasures_found: saved_treasures,
                            elapsed_seconds: saved_elapsed,
                            simulation_accumulator: saved_accumulator,
                            ..
                        } = saved;
                        *map = saved_map;
                        doors = saved_doors;
                        item_pickups = saved_pickups;
                        active_actors = saved_actors;
                        enemies = saved_enemies;
                        pushwalls = saved_pushwalls;
                        player = saved_player;
                        *campaign = saved_campaign;
                        weapon_attack = saved_weapon_attack;
                        keys_owned = saved_keys;
                        health = saved_health;
                        arsenal = saved_arsenal;
                        score = saved_score;
                        next_extra_life_score = saved_next_extra_life_score;
                        kills = saved_kills;
                        lives = saved_lives;
                        level_kills = saved_level_kills;
                        treasures_found = saved_treasures;
                        level_tics = (saved_elapsed * 70.0).round() as u64;
                        simulation_accumulator =
                            saved_accumulator.clamp(0.0, (1.0 / 70.0) * 0.999_999);
                        use_consumed = false;
                        fire_consumed = false;
                        interstitial_was_active = false;
                        camera_fade_in_pending = false;
                        level_reveal_pending = true;
                        reveal_from_death = false;
                        damage_flash_tics = 0;
                        bonus_flash_tics = 0;
                        displayed_flash = (0, 0);
                        gatling_pickup_face = false;
                        previous = Instant::now();
                        runtime_notice = Some(format!(
                            "SLOT {} RESTORED: {save_name}",
                            selected_save_slot + 1
                        ));
                        state_changed = true;
                    }
                    Ok(_) => {
                        runtime_notice = Some("SAVE DOES NOT MATCH THIS LEVEL".into());
                        state_changed = true;
                    }
                    Err(error) => {
                        runtime_notice = Some(format!("LOAD FAILED: {error}"));
                        state_changed = true;
                    }
                }
                if quickload_audio_resume {
                    quickload_audio_resume = false;
                    let level_music =
                        audio.and_then(|archive| archive.music_player_for_map(map.number));
                    replace_mixer_music(audio_device, &audio_mixer, level_music);
                    if audio_device != 0 {
                        SDL_PauseAudioDevice(audio_device, 0);
                    }
                    set_gameplay_mouse_capture();
                    previous = Instant::now();
                }
            }
            load_was_pressed = load_pressed;
            if state_changed {
                if let Some(notice) = runtime_notice.as_deref() {
                    let status = CString::new(format!("{} | {notice}", title.to_string_lossy()))?;
                    SDL_SetWindowTitle(window, status.as_ptr());
                }
            }
            let forward = if !controls_enabled {
                0.0
            } else {
                control_pressed(0) as i32 as f64 - control_pressed(1) as i32 as f64
                    + keyboard_move_pressed(0) as i32 as f64
                    - keyboard_move_pressed(2) as i32 as f64
                    + controller_controls.forward
            };
            let run_mode = controls_enabled && control_pressed(4);
            let mouse_enabled = controls_enabled && crate::settings::mouse_input_enabled();
            let mouse_bindings = crate::settings::button_bindings().mouse;
            let mouse_action_pressed = |action: u8| {
                mouse_enabled
                    && mouse_bindings
                        .iter()
                        .enumerate()
                        .any(|(index, &bound_action)| {
                            bound_action == action && mouse_buttons & (1 << index) != 0
                        })
            };
            let mouse_strafe_button = mouse_action_pressed(crate::settings::BUTTON_ACTION_STRAFE);
            let strafing = controls_enabled && (control_pressed(5) || mouse_strafe_button);
            let horizontal_keyboard = control_pressed(3) as i32 as f64
                - control_pressed(2) as i32 as f64
                + keyboard_move_pressed(1) as i32 as f64
                - keyboard_move_pressed(3) as i32 as f64
                + controller_controls.horizontal;
            let strafe = if strafing { horizontal_keyboard } else { 0.0 };
            let turn = if !controls_enabled || strafing {
                0.0
            } else {
                horizontal_keyboard
            };
            let use_pressed = controls_enabled
                && (control_pressed(6) || mouse_action_pressed(crate::settings::BUTTON_ACTION_USE));
            let fire_pressed = controls_enabled
                && (control_pressed(7)
                    || mouse_action_pressed(crate::settings::BUTTON_ACTION_FIRE));
            for slot in 1..=4 {
                if controls_enabled && control_pressed(7 + slot) && weapon_attack.is_idle() {
                    if let Some(weapon) = Weapon::from_slot(slot) {
                        arsenal.select(weapon);
                    }
                }
            }
            let now = Instant::now();
            let frame_dt = now.duration_since(previous).as_secs_f64().min(0.25);
            previous = now;
            if !game_paused && !pause_key_screen_open && help_article_page.is_none() {
                simulation_accumulator += frame_dt;
            }
            let simulation_ticks = if game_paused
                || pause_key_screen_open
                || help_article_page.is_some()
                || pause_transition.is_some()
                || level_reveal_pending
            {
                0
            } else {
                (simulation_accumulator / simulation_tic).floor() as usize
            };
            let mouse_ticks = simulation_ticks.max(1) as f64;
            let mouse_sensitivity_scale =
                1.0 / (13.0 - crate::settings::mouse_sensitivity() as f64);
            let mouse_turn_per_tick = if mouse_enabled && !strafing && simulation_ticks > 0 {
                mouse_motion_x as f64 * 10.0 * mouse_sensitivity_scale / mouse_ticks
            } else {
                0.0
            };
            let mouse_strafe_per_tick = if mouse_enabled && strafing && simulation_ticks > 0 {
                mouse_motion_x as f64 * 10.0 * mouse_sensitivity_scale / mouse_ticks
            } else {
                0.0
            };
            let mouse_forward_per_tick = if mouse_enabled && simulation_ticks > 0 {
                mouse_motion_y as f64 * 20.0 * mouse_sensitivity_scale / mouse_ticks
            } else {
                0.0
            };
            simulation_accumulator -= simulation_ticks as f64 * simulation_tic;
            if simulation_ticks > 0 {
                mouse_motion_x = 0;
                mouse_motion_y = 0;
            }
            for _ in 0..simulation_ticks {
                if !running {
                    break;
                }
                if death_elapsed.is_none() {
                    level_tics = level_tics.saturating_add(1);
                }
                let victory_active_at_tick_start = bj_victory.is_some();
                if enemies.is_death_camera_active() {
                    let camera_update = enemies.update(
                        map,
                        &mut doors,
                        &pushwalls,
                        &player,
                        health,
                        false,
                        simulation_tic,
                        map9_death_scream_available,
                        true,
                        false,
                    );
                    let mut camera_cut = false;
                    if let Some(camera) = camera_update.boss_death_camera {
                        position_boss_death_camera(
                            map,
                            &doors,
                            &pushwalls,
                            &enemies,
                            &mut player,
                            camera,
                        );
                        camera_fade_in_pending = true;
                        camera_cut = true;
                        state_changed = true;
                    }
                    if camera_update.victory {
                        victory = true;
                        level_result = Some(LevelResult::Victory);
                        running = false;
                        state_changed = true;
                    }
                    if camera_cut {
                        break;
                    }
                    continue;
                }
                let demo_input = if let Some(playback) = demo_playback.as_mut() {
                    match playback.next_tic() {
                        Some(command) => Some(demo_player_input(command)),
                        None => {
                            level_result = Some(LevelResult::DemoFinished);
                            running = false;
                            break;
                        }
                    }
                } else {
                    None
                };
                let mut tick_forward = demo_input.map_or(forward, |input| input.forward);
                let mut tick_strafe = demo_input.map_or(strafe, |input| input.strafe);
                let mut tick_turn = demo_input.map_or(turn, |input| input.turn);
                let tick_run = demo_input.map_or(run_mode, |input| input.run);
                let tick_use = demo_input.map_or(use_pressed, |input| input.use_pressed);
                let tick_fire = demo_input.map_or(fire_pressed, |input| input.fire_pressed);
                let tick_mouse_forward = demo_input.map_or(mouse_forward_per_tick, |_| 0.0);
                let tick_mouse_strafe = demo_input.map_or(mouse_strafe_per_tick, |_| 0.0);
                let tick_mouse_turn = demo_input.map_or(mouse_turn_per_tick, |_| 0.0);
                if demo_input.is_none() {
                    // DOS PollControls clamps the combined keyboard, joystick,
                    // and mouse axes to +/-100 before ControlMovement. Demos
                    // return before that clamp, so preserve their recorded axes.
                    let control_step = if tick_run { 70.0 } else { 35.0 };
                    let forward_control =
                        (tick_forward * control_step - tick_mouse_forward).clamp(-100.0, 100.0);
                    let strafe_control =
                        (tick_strafe * control_step - tick_mouse_strafe).clamp(-100.0, 100.0);
                    let turn_scale = control_step * 70.0 * simulation_tic;
                    let turn_control =
                        (tick_turn * turn_scale + tick_mouse_turn).clamp(-100.0, 100.0);
                    tick_forward = (forward_control + tick_mouse_forward) / control_step;
                    tick_strafe = (strafe_control + tick_mouse_strafe) / control_step;
                    tick_turn = (turn_control - tick_mouse_turn) / turn_scale;
                }
                let tick_player_running = dos_player_is_running(
                    tick_forward,
                    tick_strafe,
                    tick_run,
                    tick_mouse_forward,
                    tick_mouse_strafe,
                );
                if let Some(slot) = demo_input.and_then(|input| input.weapon_slot) {
                    if weapon_attack.is_idle() {
                        if let Some(weapon) = Weapon::from_slot(slot + 1) {
                            arsenal.select(weapon);
                        }
                    }
                }
                use_consumed &= tick_use;
                fire_consumed &= tick_fire;
                if let Some(elapsed) = death_elapsed {
                    if let Some(target) = death_turn_target {
                        let clockwise = (target - player.angle).rem_euclid(std::f64::consts::TAU);
                        let turn = if clockwise <= std::f64::consts::PI {
                            clockwise
                        } else {
                            clockwise - std::f64::consts::TAU
                        };
                        let step = 2.0_f64.to_radians();
                        if turn.abs() <= step {
                            player.angle = target;
                            update_mixer_listener(audio_device, &audio_mixer, &player);
                            death_turn_target = None;
                            death_fade_pending = true;
                            break;
                        }
                        player.angle =
                            (player.angle + step.copysign(turn)).rem_euclid(std::f64::consts::TAU);
                        update_mixer_listener(audio_device, &audio_mixer, &player);
                        continue;
                    }
                    if death_fade_pending {
                        break;
                    }
                    let elapsed = elapsed + simulation_tic;
                    if elapsed < 100.0 / 70.0 {
                        death_elapsed = Some(elapsed);
                        continue;
                    }
                    death_elapsed = None;
                    if lives == 0 {
                        game_over = true;
                        running = false;
                        state_changed = true;
                        continue;
                    }
                    lives -= 1;
                    health = 100;
                    gatling_pickup_face = false;
                    keys_owned = 0;
                    arsenal = Arsenal::default();
                    weapon_attack = WeaponAttack::default();
                    level_kills = 0;
                    treasures_found = 0;
                    level_tics = 0;
                    *map = initial_map.clone();
                    doors = initial_doors.clone();
                    item_pickups = initial_pickups.clone();
                    active_actors = static_actor_spawns(statics);
                    enemies = EnemyRuntime::from_spawns(
                        statics,
                        difficulty,
                        demo_playback.is_some(),
                        crate::settings::digitized_effects_enabled(),
                    );
                    pushwalls = PushWallSystem::default();
                    player = initial_player;
                    level_reveal_pending = true;
                    reveal_from_death = true;
                    state_changed = true;
                    break;
                }
                // DOS MoveDoors and MovePWalls run before DoActor(player).
                let enemy_blockers = enemies.player_blockers();
                let door_sounds = if victory_active_at_tick_start {
                    Vec::new()
                } else {
                    doors.update_with_sounds(simulation_tic, &player, &enemy_blockers)
                };
                if !door_sounds.is_empty() {
                    let reachable_tiles =
                        crate::actors::area_reachable_tiles(map, &doors, &pushwalls, &player);
                    for sound in door_sounds {
                        if crate::actors::tile_is_reachable(&reachable_tiles, map, sound.x, sound.y)
                        {
                            queue_mixer_positioned_sound(
                                audio,
                                audio_device,
                                &audio_mixer,
                                sound.sound,
                                (sound.x, sound.y),
                                &player,
                            );
                        }
                    }
                }
                let enemy_positions = enemies.views();
                let enemy_tiles = enemies.pushwall_blocking_tiles();
                pushwalls.update(map, simulation_tic, &active_actors, &enemy_tiles);
                if (bj_victory.is_none() || !weapon_attack.is_idle())
                    && enemies.update_hud_face(
                        1,
                        mixer_playing_sound(audio_device, &audio_mixer) == Some(38),
                    )
                {
                    gatling_pickup_face = false;
                }
                let attack_idle = weapon_attack.is_idle();
                // T_Player operates the tile it faces before ControlMovement.
                // T_Attack leaves use commands for the next idle tic.
                if !victory_active_at_tick_start && attack_idle && tick_use {
                    let pushed = pushwalls.interact(map, &player, &active_actors, &enemy_tiles);
                    match pushed {
                        crate::pushwalls::PushWallInteraction::Started => {
                            state_changed = true;
                            queue_mixer_pc_sound(audio, audio_device, &audio_mixer, 46);
                        }
                        crate::pushwalls::PushWallInteraction::Blocked => {
                            queue_mixer_pc_sound(audio, audio_device, &audio_mixer, 6);
                        }
                        crate::pushwalls::PushWallInteraction::NotPushwall if !use_consumed => {
                            if let Some(secret) = doors.use_elevator(map, &player) {
                                use_consumed = true;
                                level_complete = true;
                                keys_owned = 0;
                                level_result = Some(if secret {
                                    LevelResult::Secret
                                } else {
                                    LevelResult::Complete
                                });
                                running = false;
                                state_changed = true;
                            } else if doors.interact(map, &player, keys_owned).is_some() {
                                use_consumed = true;
                                state_changed = true;
                            } else if doors.is_locked_in_front(map, &player, keys_owned) {
                                use_consumed = true;
                                queue_mixer_pc_sound(audio, audio_device, &audio_mixer, 6);
                            } else {
                                queue_mixer_pc_sound(audio, audio_device, &audio_mixer, 20);
                            }
                        }
                        _ => {}
                    }
                }
                let fire_started =
                    !victory_active_at_tick_start && attack_idle && tick_fire && !fire_consumed;
                fire_consumed |= fire_started;
                let clipped = if victory_active_at_tick_start {
                    if let Some(target_y) = victory_spin_target_y {
                        let target_angle = 270.0_f64.to_radians();
                        let angle_step = 3.0_f64.to_radians() * 70.0 * simulation_tic;
                        if player.angle > target_angle {
                            player.angle = (player.angle - angle_step).max(target_angle);
                        } else if player.angle < target_angle {
                            player.angle = (player.angle + angle_step).min(target_angle);
                        }
                        if player.y > target_y {
                            player.y = (player.y - 4096.0 / 65_536.0 * 70.0 * simulation_tic)
                                .max(target_y);
                        }
                    }
                    false
                } else {
                    player.update(
                        map,
                        &doors,
                        &pushwalls,
                        tick_forward,
                        tick_strafe,
                        tick_turn,
                        tick_mouse_forward,
                        tick_mouse_strafe,
                        tick_mouse_turn,
                        tick_run,
                        &enemy_blockers,
                        simulation_tic,
                    )
                };
                if clipped && mixer_playing_sound(audio_device, &audio_mixer).is_none() {
                    queue_mixer_pc_sound(audio, audio_device, &audio_mixer, 0);
                }
                let player_tile_x = player.x.floor() as usize;
                let player_tile_y = player.y.floor() as usize;
                if bj_victory.is_none()
                    && map.info_tile(player_tile_x, player_tile_y) == crate::map::EXIT_TILE
                {
                    player.freeze_actor_tile(player_tile_x, player_tile_y);
                    victory_player_running = Some(tick_player_running);
                    let initial_run_countdown = enemies.next_dos_random() % 12;
                    bj_victory = Some(crate::actors::BjVictory::new(
                        player.x,
                        player.y,
                        initial_run_countdown,
                    ));
                    // VictorySpin uses the player's tile from the exit-entry
                    // tic and advances it on following actor passes.
                    victory_spin_target_y =
                        Some(player_tile_y as f64 - 5.0 - 0x3000 as f64 / 65_536.0);
                    state_changed = true;
                }
                update_mixer_listener(audio_device, &audio_mixer, &player);
                // DOS DoActor advances the player's weapon attack before
                // ThreeDRefresh collects static bonuses. Keep shot timing and
                // GiveAmmo's frame-zero check in that same tick order.
                let weapon_events = if bj_victory.is_none() {
                    weapon_attack.update(
                        &mut arsenal,
                        tick_fire && fire_consumed,
                        fire_started,
                        simulation_tic,
                    )
                } else {
                    Vec::new()
                };
                for weapon in weapon_events {
                    // DOS GunAttack/KnifeAttack emits the weapon sound before
                    // resolving damage and any target-side sound effects.
                    queue_mixer_pc_sound(audio, audio_device, &audio_mixer, weapon.attack_sound());
                    let outcome = enemies.shoot(
                        map,
                        &doors,
                        &pushwalls,
                        &player,
                        weapon,
                        arsenal.best(),
                        map9_death_scream_available,
                    );
                    if let Some(sound) = outcome.sound {
                        if let Some(source) = sound.position {
                            queue_mixer_positioned_sound(
                                audio,
                                audio_device,
                                &audio_mixer,
                                sound.sound,
                                source,
                                &player,
                            );
                        } else {
                            queue_mixer_pc_sound(audio, audio_device, &audio_mixer, sound.sound);
                        }
                    }
                    if let Some(reward) = outcome.kill {
                        if award_points(
                            reward.points,
                            &mut score,
                            &mut next_extra_life_score,
                            &mut lives,
                        ) > 0
                        {
                            queue_mixer_pc_sound(audio, audio_device, &audio_mixer, 44);
                        }
                        kills = kills.saturating_add(1);
                        level_kills = level_kills.saturating_add(1);
                        if let Some(kind) = reward.pickup {
                            item_pickups.push(ItemPickup {
                                x: reward.x,
                                y: reward.y,
                                kind,
                            });
                            let sprite_index = pickup_sprite_index(kind);
                            active_actors.push(ActorSpawn {
                                x: reward.x,
                                y: reward.y,
                                source_tile: 23 + sprite_index,
                                kind: ActorKind::Decoration(sprite_index),
                                mode: SpawnMode::Static,
                                direction: 0,
                                ambush: false,
                            });
                        }
                    }
                    state_changed = true;
                }
                let enemy_update = enemies.update(
                    map,
                    &mut doors,
                    &pushwalls,
                    &player,
                    health,
                    victory_player_running.unwrap_or(tick_player_running),
                    simulation_tic,
                    map9_death_scream_available,
                    bj_victory.is_none(),
                    bj_victory.is_some(),
                );
                if enemy_update.victory {
                    victory = true;
                    level_result = Some(LevelResult::Victory);
                    running = false;
                    state_changed = true;
                }
                for sound in enemy_update.sounds {
                    if let Some(source) = sound.position {
                        queue_mixer_positioned_sound(
                            audio,
                            audio_device,
                            &audio_mixer,
                            sound.sound,
                            source,
                            &player,
                        );
                    } else {
                        queue_mixer_pc_sound(audio, audio_device, &audio_mixer, sound.sound);
                    }
                }
                // The newly spawned BJ is appended after existing actors, so
                // DOS thinks him after their actions in this same actor pass.
                if let Some(sequence) = bj_victory.as_mut() {
                    if let Some(sound) = sequence.update_tic(map) {
                        if let Some(position) = sound.position {
                            queue_mixer_positioned_sound(
                                audio,
                                audio_device,
                                &audio_mixer,
                                sound.sound,
                                position,
                                &player,
                            );
                        } else {
                            queue_mixer_pc_sound(audio, audio_device, &audio_mixer, sound.sound);
                        }
                    }
                    if sequence.is_finished() {
                        victory = true;
                        level_result = Some(LevelResult::Victory);
                        running = false;
                        state_changed = true;
                    }
                }
                if enemy_update.damage > 0 && !enemy_update.victory && bj_victory.is_none() {
                    gatling_pickup_face = false;
                    damage_flash_tics =
                        damage_flash_tics.saturating_add(enemy_update.damage as u32);
                    health = health.saturating_sub(enemy_update.damage.min(u8::MAX as u16) as u8);
                    state_changed = true;
                }
                if health == 0 && death_elapsed.is_none() {
                    queue_mixer_pc_sound(audio, audio_device, &audio_mixer, 9);
                    death_elapsed = Some(0.0);
                    death_turn_target = enemy_update.fatal_hit_position.map(|(x, y)| {
                        // DOS quantizes the target to degrees with +Y pointing up.
                        let degrees = (player.y - y).atan2(x - player.x).to_degrees();
                        (-degrees.rem_euclid(360.0).floor())
                            .rem_euclid(360.0)
                            .to_radians()
                    });
                    death_fade_pending = death_turn_target.is_none();
                    state_changed = true;
                }
                // UpdatePaletteShifts samples both counters after DoActor,
                // then ThreeDRefresh can start a bonus flash for the next tic.
                displayed_flash = (damage_flash_tics, bonus_flash_tics);
                damage_flash_tics = damage_flash_tics.saturating_sub(1);
                bonus_flash_tics = bonus_flash_tics.saturating_sub(1);
                let visible_pickups = (health > 0
                    && item_pickups
                        .iter()
                        .any(|pickup| world::pickup_reachable(&player, pickup.x, pickup.y)))
                .then(|| world::visible_floor_tiles(map, &doors, &pushwalls, &player));
                let mut pickup_index = 0;
                while pickup_index < item_pickups.len() {
                    let pickup = item_pickups[pickup_index];
                    if visible_pickups.as_ref().is_some_and(|visible| {
                        visible[pickup.y * map.width + pickup.x]
                            && world::pickup_reachable(&player, pickup.x, pickup.y)
                    }) {
                        let collected = match pickup.kind {
                            PickupKind::Key(key) => {
                                keys_owned |= 1 << key;
                                true
                            }
                            PickupKind::Heal(amount)
                                if health < 100 && (amount != 1 || health <= 10) =>
                            {
                                health = health.saturating_add(amount).min(100);
                                gatling_pickup_face = false;
                                true
                            }
                            PickupKind::Ammo(amount) if arsenal.ammo() < 99 => {
                                arsenal.add_ammo(amount, weapon_attack.is_initial_attack_frame());
                                true
                            }
                            PickupKind::ExtraLife => {
                                lives = lives.saturating_add(1).min(9);
                                health = health.saturating_add(99).min(100);
                                gatling_pickup_face = false;
                                arsenal.add_ammo(25, weapon_attack.is_initial_attack_frame());
                                treasures_found = treasures_found.saturating_add(1);
                                true
                            }
                            PickupKind::Treasure(points) => {
                                let sound = match points {
                                    100 => 35,
                                    500 => 36,
                                    1_000 => 37,
                                    _ => 45,
                                };
                                queue_mixer_pc_sound(audio, audio_device, &audio_mixer, sound);
                                if award_points(
                                    points,
                                    &mut score,
                                    &mut next_extra_life_score,
                                    &mut lives,
                                ) > 0
                                {
                                    queue_mixer_pc_sound(audio, audio_device, &audio_mixer, 44);
                                }
                                treasures_found = treasures_found.saturating_add(1);
                                true
                            }
                            PickupKind::Weapon(weapon) => {
                                arsenal
                                    .grant_weapon(weapon, weapon_attack.is_initial_attack_frame());
                                if weapon == Weapon::Chaingun {
                                    gatling_pickup_face = true;
                                    enemies.reset_hud_face_timer();
                                }
                                true
                            }
                            _ => false,
                        };
                        if collected {
                            bonus_flash_tics = 18;
                            item_pickups.remove(pickup_index);
                            let shape = ActorKind::Decoration(pickup_sprite_index(pickup.kind));
                            if let Some(index) = active_actors.iter().position(|actor| {
                                actor.x == pickup.x
                                    && actor.y == pickup.y
                                    && (actor.kind == shape
                                        || (pickup.kind == PickupKind::Heal(1)
                                            && actor.source_tile == 61))
                            }) {
                                active_actors.remove(index);
                            }
                            state_changed = true;
                            let sound = match pickup.kind {
                                PickupKind::Key(_) => 12,
                                PickupKind::Heal(1) => 61,
                                PickupKind::Heal(25) => 34,
                                PickupKind::Heal(_) => 33,
                                PickupKind::Ammo(_) => 31,
                                PickupKind::ExtraLife => 44,
                                PickupKind::Treasure(_) => continue,
                                PickupKind::Weapon(Weapon::MachineGun) => 30,
                                PickupKind::Weapon(Weapon::Chaingun) => 38,
                                PickupKind::Weapon(Weapon::Knife | Weapon::Pistol) => 31,
                            };
                            queue_mixer_pc_sound(audio, audio_device, &audio_mixer, sound);
                        } else {
                            pickup_index += 1;
                        }
                    } else {
                        pickup_index += 1;
                    }
                }
                if state_changed {
                    let status = CString::new(format!(
                        "{} | {}{}{}Score {score}  Kills {kills}  Secrets {}  Health {health}  {}  Ammo {}  Lives {lives}  Keys {keys_owned:02X}",
                        title.to_string_lossy(),
                        runtime_notice
                            .as_deref()
                            .map(|notice| format!("{notice} | "))
                            .unwrap_or_default(),
                        if game_over { "GAME OVER | " } else { "" },
                        if victory {
                            "VICTORY | "
                        } else if level_complete {
                            "LEVEL COMPLETE | "
                        } else {
                            ""
                        },
                        pushwalls.secrets_found,
                        arsenal.selected().label(),
                        arsenal.ammo()
                    ))?;
                    SDL_SetWindowTitle(window, status.as_ptr());
                }
            }
            let mut enemy_views = enemies.views();
            if let Some(sequence) = bj_victory.as_ref() {
                enemy_views.push(sequence.view());
            }
            let hud_text = format!(
                "HP {health:3}  AMMO {:2}  {}  LIVES {lives}\nSCORE {score:6}  KILLS {kills:3}  KEYS {keys_owned:02X}  SECRETS {}{}{}",
                arsenal.ammo(),
                arsenal.selected().label().to_ascii_uppercase(),
                pushwalls.secrets_found,
                if game_over { "  GAME OVER" } else { "" },
                if victory {
                    "  VICTORY"
                } else if level_complete {
                    "  LEVEL COMPLETE"
                } else {
                    ""
                }
            );
            let boss_interstitial_active = enemies.is_boss_death_interstitial_active();
            let mut transition_from = (!death_fade_pending
                && (camera_fade_in_pending
                    || (boss_interstitial_active && !interstitial_was_active)))
                .then(|| pixels.clone());
            let mut high_resolution_frame = world::render(
                map,
                textures,
                &doors,
                &pushwalls,
                &player,
                &active_actors,
                &enemy_views,
                sprites,
                font,
                status_bar,
                hud_pictures,
                weapon_pictures
                    .get(arsenal.selected().slot())
                    .filter(|_| bj_victory.is_none()),
                textures
                    .filter(|_| death_elapsed.is_none() && bj_victory.is_none())
                    .map(|_| {
                        arsenal.selected().sprite_base() + weapon_attack.display_frame() as usize
                    }),
                world::HudValues {
                    health,
                    face_frame: enemies.hud_face_frame(),
                    mutant_death_face: enemies.last_damage_was_needle(),
                    gatling_pickup_face,
                    ammo: arsenal.ammo(),
                    lives,
                    score,
                    map_number: map.number,
                    keys: keys_owned,
                },
                &hud_text,
                &mut pixels,
            );
            if !game_paused
                && pause_transition.is_none()
                && death_elapsed.is_none()
                && bj_victory.is_none()
                && !boss_interstitial_active
            {
                enemies.refresh_drawn_visibility(map, &doors, &pushwalls, &player);
            }
            if boss_interstitial_active {
                world::render_boss_death_interstitial(&mut pixels, font);
            }
            if death_fade_pending {
                damage_flash_tics = 0;
                bonus_flash_tics = 0;
                displayed_flash = (0, 0);
                transition_from = Some(pixels.clone());
                world::render_death_view(&mut pixels);
            } else if level_reveal_pending && running {
                let mut initial_frame = pixels.clone();
                if reveal_from_death {
                    world::render_death_view(&mut initial_frame);
                } else {
                    world::render_empty_view(&mut initial_frame);
                }
                transition_from = Some(initial_frame);
            } else {
                world::render_palette_shift(&mut pixels, displayed_flash.0, displayed_flash.1);
                if let Some((_, _, frame)) = high_resolution_frame.as_mut() {
                    let scale = crate::settings::render_resolution().scale();
                    world::render_palette_shift_scaled(
                        frame,
                        scale,
                        displayed_flash.0,
                        displayed_flash.1,
                    );
                }
            }
            let frame_transition_pending = transition_from.is_some();
            if let Some(previous_frame) = transition_from {
                let target_frame = pixels.clone();
                let (bounds, frames) = if death_fade_pending {
                    (world::view_bounds(), 70)
                } else if camera_fade_in_pending || level_reveal_pending {
                    // ThreeDRefresh reveals the view through a 20-frame fizzle.
                    (world::view_bounds(), 20)
                } else {
                    // A_StartDeathCam clears the whole area above the status bar.
                    ((0, 0, WIDTH, HEIGHT - 40), 70)
                };
                unsafe {
                    fizzle_transition(
                        renderer,
                        texture,
                        &previous_frame,
                        &target_frame,
                        &mut pixels,
                        bounds,
                        frames,
                    )?;
                }
                previous = Instant::now();
                simulation_accumulator = 0.0;
                death_fade_pending = false;
                camera_fade_in_pending = false;
                level_reveal_pending = false;
                reveal_from_death = false;
            }
            interstitial_was_active = boss_interstitial_active;
            if death_elapsed.is_some() && death_turn_target.is_none() {
                world::render_death_view(&mut pixels);
            }
            let preserve_status_bar = game_paused
                || pause_confirming_load
                || pause_key_screen_open
                || help_article_page.is_some();
            let status_bar_frame =
                preserve_status_bar.then(|| pixels[(HEIGHT - 40) * WIDTH..HEIGHT * WIDTH].to_vec());
            let pause_fading_out = pause_transition.as_ref().is_some_and(|transition| {
                matches!(transition.phase, PauseTransitionPhase::FadeOut)
            });
            if game_paused && !pause_fading_out && !pause_confirming_load {
                if let Some(font) = font {
                    if pause_scores_open {
                        if let Some(art) = pause_high_score_art.as_ref() {
                            world::render_high_score_screen(
                                &mut pixels,
                                font,
                                &pause_high_score_table,
                                None,
                                None,
                                art.title.as_ref(),
                                [
                                    art.columns[0].as_ref(),
                                    art.columns[1].as_ref(),
                                    art.columns[2].as_ref(),
                                ],
                                art.shareware,
                            );
                        } else {
                            world::render_high_score_screen(
                                &mut pixels,
                                font,
                                &pause_high_score_table,
                                None,
                                None,
                                None,
                                [None, None, None],
                                false,
                            );
                        }
                    } else if pause_quick_end_overlay {
                        world::render_quick_end_confirmation(
                            &mut pixels,
                            font,
                            pause_quit_confirm_selection,
                        );
                    } else if pause_quick_quit_overlay {
                        world::render_quick_quit_confirmation(
                            &mut pixels,
                            font,
                            pause_quick_quit_prompt,
                            pause_quit_confirm_selection,
                        );
                    } else if let Some(loading) = pause_slot_mode {
                        world::render_pause_slot_menu(
                            &mut pixels,
                            font,
                            pause_sound_pictures,
                            loading,
                            pause_slot_selection,
                            &pause_slot_labels,
                        );
                    } else if pause_sound_settings_open {
                        world::render_pause_sound_menu(
                            &mut pixels,
                            font,
                            pause_sound_pictures,
                            pause_sound_selection,
                        );
                    } else if pause_controls_open {
                        let options = runtime_pause_control_labels(
                            pause_control_capture,
                            pause_button_mapping,
                            pause_button_capture,
                            pause_movement_mapping_open,
                            pause_move_key_capture,
                        );
                        world::render_pause_controls_menu(
                            &mut pixels,
                            font,
                            pause_sound_pictures,
                            &options,
                            pause_controls_selection,
                            if pause_button_capture.is_some() {
                                Some("PRESS A BUTTON (ESC CANCELS)")
                            } else if pause_control_capture.is_some()
                                || pause_move_key_capture.is_some()
                            {
                                Some("PRESS A KEY (ESC CANCELS)")
                            } else {
                                None
                            },
                        );
                    } else if pause_view_size_open {
                        world::render_pause_view_size_menu(&mut pixels, font);
                    } else if pause_display_open {
                        let options = runtime_pause_display_options();
                        world::render_pause_display_menu(
                            &mut pixels,
                            font,
                            &options,
                            pause_display_selection,
                        );
                    } else if let (Some(menu), Some(art)) =
                        (pause_new_game.as_ref(), pause_new_game_art.as_ref())
                    {
                        match menu.step {
                            PauseNewGameStep::Episode => world::render_choice_menu(
                                &mut pixels,
                                font,
                                "Which episode to play?",
                                &art.episode_options,
                                world::ChoiceMenuLayout::Episode,
                                &art.background,
                                &art.episode_illustrations,
                                menu.selected,
                                0,
                            ),
                            PauseNewGameStep::Difficulty => world::render_choice_menu(
                                &mut pixels,
                                font,
                                "How tough are you?",
                                &art.difficulty_options,
                                world::ChoiceMenuLayout::Difficulty,
                                &art.background,
                                &art.difficulty_illustrations,
                                menu.selected,
                                0,
                            ),
                            PauseNewGameStep::Confirm => world::render_pause_menu(
                                &mut pixels,
                                font,
                                pause_sound_pictures,
                                pause_selection,
                                Some("START NEW GAME?"),
                                pause_quit_confirm_selection,
                            ),
                        }
                    } else {
                        world::render_pause_menu(
                            &mut pixels,
                            font,
                            pause_sound_pictures,
                            pause_selection,
                            if pause_confirming_application_quit {
                                Some("QUIT GAME?")
                            } else if pause_confirming_new_game {
                                Some("START NEW GAME?")
                            } else if pause_confirming_quit {
                                Some("END GAME?")
                            } else {
                                None
                            },
                            pause_quit_confirm_selection,
                        );
                    }
                }
            }
            if pause_confirming_load {
                if let Some(font) = font {
                    world::render_pause_load_confirmation(
                        &mut pixels,
                        font,
                        selected_save_slot,
                        pause_load_confirm_selection,
                    );
                }
            }
            if pause_key_screen_open {
                if let Some(pictures) = pause_sound_pictures {
                    world::render_pause_key_confirmation(&mut pixels, pictures);
                }
            }
            if let (Some(page), Some(help), Some(font)) = (help_article_page, help_articles, font) {
                help.pages[page].render(
                    &mut pixels,
                    font,
                    &help.frame,
                    page + 1,
                    help.pages.len(),
                    help_page_started.elapsed().as_secs_f64() * 70.0,
                );
            }
            if let Some(status_bar_frame) = status_bar_frame {
                pixels[(HEIGHT - 40) * WIDTH..HEIGHT * WIDTH].copy_from_slice(&status_bar_frame);
            }
            let mut resume_audio_after_fade = None;
            if let Some(transition) = pause_transition.as_mut() {
                let duration = match transition.phase {
                    PauseTransitionPhase::FadeOut => transition.fade_out_duration,
                    PauseTransitionPhase::FadeIn => transition.fade_in_duration,
                };
                let elapsed = transition.started.elapsed();
                let elapsed = if matches!(transition.phase, PauseTransitionPhase::FadeIn) {
                    elapsed.saturating_sub(transition.fade_in_delay)
                } else {
                    elapsed
                };
                let progress = (elapsed.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0);
                match transition.phase {
                    PauseTransitionPhase::FadeOut => {
                        let frame_len = pixels.len();
                        pixels.copy_from_slice(&transition.frame[..frame_len]);
                        fade_frame(&mut pixels, 1.0 - progress);
                        if progress >= 1.0 {
                            if transition.exit_after_fade_out {
                                running = false;
                            } else {
                                if transition.start_high_score_music {
                                    let score_music = audio
                                        .and_then(|archive| archive.music_player_for_track(23));
                                    replace_mixer_music(audio_device, &audio_mixer, score_music);
                                    if audio_device != 0 {
                                        SDL_PauseAudioDevice(audio_device, 0);
                                    }
                                    transition.start_high_score_music = false;
                                }
                                if transition.start_control_panel_music {
                                    let menu_music = audio
                                        .and_then(|archive| archive.music_player_for_track(14));
                                    replace_mixer_music(audio_device, &audio_mixer, menu_music);
                                    if audio_device != 0 {
                                        SDL_PauseAudioDevice(audio_device, 0);
                                    }
                                    queue_mixer_pc_sound(audio, audio_device, &audio_mixer, 4);
                                    transition.start_control_panel_music = false;
                                }
                                if transition.start_help_music {
                                    let help_music =
                                        audio.and_then(|archive| archive.music_player_for_track(0));
                                    replace_mixer_music(audio_device, &audio_mixer, help_music);
                                    if audio_device != 0 {
                                        SDL_PauseAudioDevice(audio_device, 0);
                                    }
                                    transition.start_help_music = false;
                                }
                                transition.phase = PauseTransitionPhase::FadeIn;
                                let fade_in_started = Instant::now();
                                transition.started = fade_in_started;
                                if let (Some(page), Some(help)) =
                                    (help_article_page, help_articles.as_ref())
                                {
                                    help_page_started = fade_in_started;
                                    transition.fade_in_delay = Duration::from_secs_f64(
                                        help.pages[page].layout_delay_tics() as f64 / 70.0,
                                    );
                                }
                            }
                        }
                    }
                    PauseTransitionPhase::FadeIn => {
                        fade_frame(&mut pixels, progress);
                        if progress >= 1.0 {
                            resume_audio_after_fade = Some(transition.resume_audio);
                        }
                    }
                }
            }
            if let Some(resume_audio) = resume_audio_after_fade {
                pause_transition = None;
                if resume_audio {
                    let level_music =
                        audio.and_then(|archive| archive.music_player_for_map(map.number));
                    replace_mixer_music(audio_device, &audio_mixer, level_music);
                    if audio_device != 0 {
                        SDL_PauseAudioDevice(audio_device, 0);
                    }
                }
                if resume_audio && demo_playback.is_none() {
                    set_gameplay_mouse_capture();
                }
            }
            let show_high_resolution_frame = high_resolution_frame.is_some()
                && !game_paused
                && pause_transition.is_none()
                && !pause_confirming_load
                && !pause_key_screen_open
                && help_article_page.is_none()
                && !boss_interstitial_active
                && death_elapsed.is_none()
                && !death_fade_pending
                && !level_reveal_pending
                && !frame_transition_pending;
            let presentation_failed = if show_high_resolution_frame {
                let (source_width, source_height, frame) =
                    high_resolution_frame.as_ref().expect("checked above");
                let high_texture = high_resolution_texture(renderer);
                if high_texture.is_null() {
                    true
                } else {
                    let source = SdlRect {
                        x: 0,
                        y: 0,
                        w: *source_width as i32,
                        h: *source_height as i32,
                    };
                    SDL_UpdateTextureNative(
                        high_texture,
                        &source as *const SdlRect as *const (),
                        frame.as_ptr(),
                        (*source_width * std::mem::size_of::<u32>()) as i32,
                    ) != 0
                        || SDL_RenderClear(renderer) != 0
                        || render_frame_region(
                            renderer,
                            high_texture,
                            *source_width,
                            *source_height,
                        ) != 0
                }
            } else {
                SDL_UpdateTexture(
                    texture,
                    std::ptr::null(),
                    pixels.as_ptr(),
                    (WIDTH * 4) as i32,
                ) != 0
                    || SDL_RenderClear(renderer) != 0
                    || render_frame(renderer, texture) != 0
            };
            if presentation_failed {
                let error = sdl_error();
                SDL_DestroyTexture(texture);
                SDL_DestroyRenderer(renderer);
                SDL_DestroyWindow(window);
                return Err(error);
            }
            SDL_RenderPresent(renderer);
            if game_over || level_complete || victory {
                if level_complete {
                    queue_mixer_pc_sound(audio, audio_device, &audio_mixer, 40);
                    while mixer_effect_is_active(audio_device, &audio_mixer, 40) {
                        while SDL_PollEvent(&mut event) != 0 {
                            if u32::from_ne_bytes(event.bytes[0..4].try_into().unwrap()) == SDL_QUIT
                            {
                                SDL_DestroyTexture(texture);
                                SDL_DestroyRenderer(renderer);
                                SDL_DestroyWindow(window);
                                return Ok(LevelResult::Quit);
                            }
                        }
                        SDL_Delay(1);
                    }
                }
                if audio_device != 0 {
                    SDL_LockAudioDevice(audio_device);
                    (&mut *audio_mixer.get()).music = None;
                    SDL_UnlockAudioDevice(audio_device);
                }
                let final_frame = pixels.clone();
                if let Some(frame) = last_level_frame.as_deref_mut() {
                    if frame.len() >= WIDTH * HEIGHT {
                        frame[WIDTH * 160..WIDTH * HEIGHT]
                            .copy_from_slice(&final_frame[WIDTH * 160..WIDTH * HEIGHT]);
                    }
                }
                for step in 1..=30 {
                    while SDL_PollEvent(&mut event) != 0 {
                        if u32::from_ne_bytes(event.bytes[0..4].try_into().unwrap()) == SDL_QUIT {
                            SDL_DestroyTexture(texture);
                            SDL_DestroyRenderer(renderer);
                            SDL_DestroyWindow(window);
                            return Ok(LevelResult::Quit);
                        }
                    }
                    pixels.copy_from_slice(&final_frame);
                    fade_frame(&mut pixels, 1.0 - f64::from(step) / 30.0);
                    if SDL_UpdateTexture(
                        texture,
                        std::ptr::null(),
                        pixels.as_ptr(),
                        (WIDTH * 4) as i32,
                    ) != 0
                        || SDL_RenderClear(renderer) != 0
                        || render_frame(renderer, texture) != 0
                    {
                        let error = sdl_error();
                        SDL_DestroyTexture(texture);
                        SDL_DestroyRenderer(renderer);
                        SDL_DestroyWindow(window);
                        return Err(error);
                    }
                    SDL_RenderPresent(renderer);
                    SDL_Delay(14);
                }
                break;
            }
            SDL_Delay(1);
        }
        SDL_DestroyTexture(texture);
        SDL_DestroyRenderer(renderer);
        SDL_DestroyWindow(window);
        campaign.health = health;
        campaign.arsenal = arsenal;
        campaign.score = score;
        campaign.next_extra_life_score = next_extra_life_score;
        campaign.kills = kills;
        campaign.lives = lives;
        campaign.keys_owned = if level_complete { 0 } else { keys_owned };
        if game_over {
            LevelResult::GameOver
        } else if let Some(result) = level_result {
            result
        } else {
            LevelResult::Quit
        }
    };
    if matches!(outcome, LevelResult::Complete | LevelResult::Secret) {
        campaign.last_level_stats = Some(LevelStats {
            elapsed_seconds: (level_tics / 70).min(99 * 60) as u32,
            kills: level_kills,
            total_kills,
            secrets: pushwalls.secrets_found,
            total_secrets,
            treasures: treasures_found,
            total_treasures,
        });
    }
    Ok(outcome)
}

fn sdl_error() -> Box<dyn Error> {
    let message = unsafe {
        let ptr = SDL_GetError();
        if ptr.is_null() {
            "unknown SDL2 error".to_owned()
        } else {
            std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    };
    std::io::Error::other(message).into()
}

fn position_boss_death_camera(
    map: &Map,
    doors: &DoorSystem,
    pushwalls: &PushWallSystem,
    enemies: &EnemyRuntime,
    player: &mut Player,
    camera: crate::actors::BossDeathCamera,
) {
    let angle = (camera.origin_y - camera.boss_y)
        .atan2(camera.boss_x - camera.origin_x)
        .rem_euclid(std::f64::consts::TAU);
    let blockers = enemies.player_blockers();
    let mut distance = 0x14000 as f64 / 65_536.0;
    while distance <= 32.0 {
        let x = camera.boss_x - angle.cos() * distance;
        let y = camera.boss_y + angle.sin() * distance;
        if world::position_is_clear(map, doors, pushwalls, &blockers, x, y) {
            player.x = x;
            player.y = y;
            break;
        }
        distance += 0x1000 as f64 / 65_536.0;
    }
    player.angle = angle;
}

unsafe fn fizzle_transition(
    renderer: *mut SdlRenderer,
    texture: *mut SdlTexture,
    previous: &[u32],
    target: &[u32],
    pixels: &mut [u32],
    (left, top, width, height): (usize, usize, usize, usize),
    frames: usize,
) -> Result<(), Box<dyn Error>> {
    let pixels_per_frame = 64_000 / frames;
    if previous.len() < WIDTH * HEIGHT
        || target.len() < WIDTH * HEIGHT
        || pixels.len() < WIDTH * HEIGHT
    {
        return Ok(());
    }
    let mut random = 1u32;
    pixels[..WIDTH * HEIGHT].copy_from_slice(&previous[..WIDTH * HEIGHT]);
    loop {
        let frame_started = Instant::now();
        let mut finished = false;
        for _ in 0..pixels_per_frame {
            // DOS forms x from AH and DL, retaining all 16 bits and rejecting
            // out-of-range values after extraction; masking to nine bits would
            // fold those rejected values back into the visible area.
            let x = ((random >> 8) & 0xffff) as usize;
            let y = (random as u8).wrapping_sub(1) as usize;
            if x <= width && y <= height {
                let index = (top + y) * WIDTH + left + x;
                if index < WIDTH * HEIGHT {
                    pixels[index] = target[index];
                }
            }
            random = (random >> 1) ^ if random & 1 != 0 { 0x1_2000 } else { 0 };
            if random == 1 {
                finished = true;
                break;
            }
        }
        unsafe {
            if SDL_UpdateTexture(
                texture,
                std::ptr::null(),
                pixels.as_ptr(),
                (WIDTH * 4) as i32,
            ) != 0
                || SDL_RenderClear(renderer) != 0
                || render_frame(renderer, texture) != 0
            {
                return Err(sdl_error());
            }
            SDL_RenderPresent(renderer);
            while frame_started.elapsed() < Duration::from_nanos(14_285_714) {
                SDL_Delay(1);
            }
        }
        if finished {
            break;
        }
    }
    pixels[..WIDTH * HEIGHT].copy_from_slice(&target[..WIDTH * HEIGHT]);
    Ok(())
}

fn queue_mixer_pc_sound(
    audio: Option<&AudioArchive>,
    device: u32,
    mixer: &UnsafeCell<AudioMixer>,
    sound: usize,
) {
    queue_mixer_sound(audio, device, mixer, sound, None);
}

fn queue_mixer_positioned_sound(
    audio: Option<&AudioArchive>,
    device: u32,
    mixer: &UnsafeCell<AudioMixer>,
    sound: usize,
    source: (f64, f64),
    listener: &Player,
) {
    update_mixer_listener(device, mixer, listener);
    queue_mixer_sound(audio, device, mixer, sound, Some(source));
}

fn update_mixer_listener(device: u32, mixer: &UnsafeCell<AudioMixer>, listener: &Player) {
    if device == 0 {
        return;
    }
    unsafe { SDL_LockAudioDevice(device) };
    unsafe {
        (&mut *mixer.get()).listener = (listener.x, listener.y, listener.angle);
    }
    unsafe { SDL_UnlockAudioDevice(device) };
}

fn stop_mixer_digitized_effect(device: u32, mixer: &UnsafeCell<AudioMixer>) {
    if device != 0 {
        unsafe { SDL_LockAudioDevice(device) };
    }
    unsafe {
        (&mut *mixer.get()).digitized_effect = None;
    }
    if device != 0 {
        unsafe { SDL_UnlockAudioDevice(device) };
    }
}

fn stop_mixer_synthesized_effect(device: u32, mixer: &UnsafeCell<AudioMixer>) {
    if device != 0 {
        unsafe { SDL_LockAudioDevice(device) };
    }
    unsafe {
        (&mut *mixer.get()).stop_synthesized_effect();
    }
    if device != 0 {
        unsafe { SDL_UnlockAudioDevice(device) };
    }
}

fn replace_mixer_music(
    device: u32,
    mixer: &UnsafeCell<AudioMixer>,
    music: Option<crate::opl2::ImfPlayer>,
) {
    if device != 0 {
        unsafe { SDL_LockAudioDevice(device) };
    }
    let previous = unsafe {
        let mixer = &mut *mixer.get();
        let previous = mixer.stop_music();
        mixer.music = music;
        previous
    };
    if device != 0 {
        unsafe { SDL_UnlockAudioDevice(device) };
    }
    drop(previous);
}

fn stop_mixer_music(device: u32, mixer: &UnsafeCell<AudioMixer>) {
    if device != 0 {
        unsafe { SDL_LockAudioDevice(device) };
    }
    let previous = unsafe { (&mut *mixer.get()).stop_music() };
    if device != 0 {
        unsafe { SDL_UnlockAudioDevice(device) };
    }
    drop(previous);
}

fn mixer_playing_sound(device: u32, mixer: &UnsafeCell<AudioMixer>) -> Option<usize> {
    if device == 0 {
        return None;
    }
    unsafe { SDL_LockAudioDevice(device) };
    let playing = unsafe {
        (&*mixer.get())
            .synthesized_effect
            .as_ref()
            .filter(|active| active.is_playing())
            .map(|active| active.sound)
    };
    unsafe { SDL_UnlockAudioDevice(device) };
    playing
}

fn mixer_effect_is_active(device: u32, mixer: &UnsafeCell<AudioMixer>, sound: usize) -> bool {
    if device == 0 {
        return false;
    }
    unsafe { SDL_LockAudioDevice(device) };
    let mixer = unsafe { &*mixer.get() };
    let active = [&mixer.synthesized_effect, &mixer.digitized_effect]
        .into_iter()
        .filter_map(Option::as_ref)
        .any(|effect| effect.sound == sound && effect.is_playing());
    unsafe { SDL_UnlockAudioDevice(device) };
    active
}

fn mixer_has_active_effect(device: u32, mixer: &UnsafeCell<AudioMixer>) -> bool {
    if device == 0 {
        return false;
    }
    unsafe { SDL_LockAudioDevice(device) };
    let mixer = unsafe { &*mixer.get() };
    let active = [&mixer.synthesized_effect, &mixer.digitized_effect]
        .into_iter()
        .filter_map(Option::as_ref)
        .any(ActiveEffect::is_playing);
    unsafe { SDL_UnlockAudioDevice(device) };
    active
}

fn gain_for_attenuation(level: u8) -> f64 {
    // WL_GAME's attenuation tables are fed to SDL_PositionSBP, which writes
    // (15 - attenuation) into each four-bit mixer nibble. On the CT1345,
    // only the upper three bits are the channel volume level; the low bit is
    // reserved in the documented register map. Approximate the CT1345 voice
    // curve with the documented SB Pro-compatible levels, normalized so the
    // highest mixer level is unity.
    const SB_PRO_VOICE_GAIN: [f64; 8] = [
        0.0,
        0.050_118_7,
        0.158_489_3,
        0.316_227_8,
        0.501_187_2,
        0.630_957_3,
        0.794_328_2,
        1.0,
    ];
    let mixer_nibble = 15u8.saturating_sub(level.min(15));
    let mixer_level = (mixer_nibble >> 1) as usize;
    SB_PRO_VOICE_GAIN[mixer_level]
}

fn queue_mixer_sound(
    audio: Option<&AudioArchive>,
    device: u32,
    mixer: &UnsafeCell<AudioMixer>,
    sound: usize,
    position: Option<(f64, f64)>,
) {
    if device == 0 {
        return;
    }
    let Some(archive) = audio else { return };
    let (pcm, digitized) =
        effect_pcm(archive, sound).unwrap_or_else(|| (Cow::Owned(Vec::new()), false));
    if (digitized && !crate::settings::digitized_effects_enabled())
        || (!digitized && !crate::settings::effects_enabled())
    {
        return;
    }
    let priority = effect_priority(archive, sound);
    unsafe { SDL_LockAudioDevice(device) };
    let mixer = unsafe { &mut *mixer.get() };
    if !mixer.accepts_effect(digitized, priority) {
        unsafe { SDL_UnlockAudioDevice(device) };
        return;
    }
    let mut samples = pcm.into_owned();
    let mut adlib = None;
    if !digitized && crate::settings::adlib_effects_enabled() {
        if let Some(effect) = archive.adlib_sound(sound) {
            adlib = crate::opl2::AdLibEffectPlayer::new(
                effect.instrument,
                effect.block,
                &effect.pitches,
                OUTPUT_RATE,
            );
            if adlib.is_some() {
                samples.clear();
            }
        }
    }
    if digitized || !samples.is_empty() || adlib.is_some() {
        mixer.play_effect(
            samples,
            adlib,
            priority,
            sound,
            position.filter(|_| digitized),
            digitized,
        );
    }
    unsafe { SDL_UnlockAudioDevice(device) };
}

fn effect_pcm(archive: &AudioArchive, sound: usize) -> Option<(Cow<'_, [i16]>, bool)> {
    if crate::settings::digitized_effects_enabled() {
        if let Some(digitized) = archive.digitized_sound_pcm(sound) {
            return Some((Cow::Borrowed(digitized), true));
        }
    }
    archive
        .pc_sound_pcm(sound)
        .map(|pcm| (Cow::Owned(pcm), false))
}

fn effect_priority(archive: &AudioArchive, sound: usize) -> u16 {
    if crate::settings::adlib_effects_enabled() {
        archive
            .adlib_priority(sound)
            .or_else(|| archive.priority(sound))
    } else {
        archive.priority(sound)
    }
    .unwrap_or_default()
}

fn pickup_sprite_index(kind: PickupKind) -> u16 {
    match kind {
        PickupKind::Key(0) => 20,
        PickupKind::Key(_) => 21,
        PickupKind::Heal(4) => 6,
        PickupKind::Heal(10) => 24,
        PickupKind::Heal(25) => 25,
        PickupKind::Heal(_) => 34,
        PickupKind::Ammo(_) => 26,
        PickupKind::ExtraLife => 33,
        PickupKind::Treasure(100) => 29,
        PickupKind::Treasure(500) => 30,
        PickupKind::Treasure(1_000) => 31,
        PickupKind::Treasure(_) => 32,
        PickupKind::Weapon(Weapon::MachineGun) => 27,
        PickupKind::Weapon(Weapon::Chaingun) => 28,
        PickupKind::Weapon(Weapon::Knife | Weapon::Pistol) => 26,
    }
}

#[cfg(test)]
mod tests {
    use super::{award_points, demo_player_input};
    use crate::demo::DemoCommand;

    #[test]
    fn score_thresholds_award_multiple_lives_and_advance() {
        let (mut score, mut next_life, mut lives) = (39_000, 40_000, 3);
        assert_eq!(
            award_points(41_000, &mut score, &mut next_life, &mut lives),
            2
        );
        assert_eq!((score, next_life, lives), (80_000, 120_000, 5));
    }

    #[test]
    fn extra_lives_are_capped_without_repeating_a_score_threshold() {
        let (mut score, mut next_life, mut lives) = (39_000, 40_000, 9);
        assert_eq!(
            award_points(1_000, &mut score, &mut next_life, &mut lives),
            1
        );
        assert_eq!((score, next_life, lives), (40_000, 80_000, 9));
    }

    #[test]
    fn demo_commands_map_dos_buttons_and_axes_to_player_controls() {
        let input = demo_player_input(DemoCommand {
            buttons: 0b1010_1111,
            control_x: 70,
            control_y: -70,
        });
        assert_eq!(input.forward, 1.0);
        assert_eq!(input.strafe, 1.0);
        assert_eq!(input.turn, 0.0);
        assert!(input.run);
        assert!(input.use_pressed);
        assert!(input.fire_pressed);
        assert_eq!(input.weapon_slot, Some(1));
    }
}
