use crate::{
    actors::{ActorKind, ActorSpawn, ActorView, static_blocks_tile},
    doors::DoorSystem,
    ending::{DrawOp, EndingPage},
    game_palette::GAME_PALETTE,
    map::Map,
    pushwalls::PushWallSystem,
    sprites::{Sprite, SpriteAtlas, shape_for_actor},
    vga::{BitmapFont, Picture},
    vswap::Vswap,
};
use std::{
    sync::{
        OnceLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    thread,
    time::Instant,
};

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 200;
pub const VIEW_WIDTH: usize = 240;
const VIEW_HEIGHT: usize = 120;
// Original CalcProjection uses FOCALLENGTH (0x5700) + MINDIST (0x5800).
pub(crate) const VIEW_FACE_DISTANCE: f64 = (0x5700u32 + 0x5800u32) as f64 / 65_536.0;
const VIEW_LEFT: usize = (WIDTH - VIEW_WIDTH) / 2;
const VIEW_TOP: usize = (HEIGHT - 40 - VIEW_HEIGHT) / 2;
const VIEW_RIGHT: usize = VIEW_LEFT + VIEW_WIDTH;
const VIEW_BOTTOM: usize = VIEW_TOP + VIEW_HEIGHT;
pub const VIEW_CENTER_X: f64 = (VIEW_LEFT + VIEW_WIDTH / 2 - 1) as f64;
pub(crate) const CAMERA_FORWARD_OFFSET: f64 = 0x5700 as f64 / 65_536.0;
pub(crate) const ACTOR_PROJECTION_OFFSET: f64 = 0x4000 as f64 / 65_536.0;
const STATIC_PROJECTION_OFFSET: f64 = 0x2000 as f64 / 65_536.0;
const CEILING_COLORS: [u8; 60] = [
    0x1d, 0x1d, 0x1d, 0x1d, 0x1d, 0x1d, 0x1d, 0x1d, 0x1d, 0xbf, 0x4e, 0x4e, 0x4e, 0x1d, 0x8d, 0x4e,
    0x1d, 0x2d, 0x1d, 0x8d, 0x1d, 0x1d, 0x1d, 0x1d, 0x1d, 0x2d, 0xdd, 0x1d, 0x1d, 0x98, 0x1d, 0x9d,
    0x2d, 0xdd, 0xdd, 0x9d, 0x2d, 0x4d, 0x1d, 0xdd, 0x7d, 0x1d, 0x2d, 0x2d, 0xdd, 0xd7, 0x1d, 0x1d,
    0x1d, 0x2d, 0x1d, 0x1d, 0x1d, 0x1d, 0xdd, 0xdd, 0x7d, 0xdd, 0xdd, 0xdd,
];
const FLOOR_COLOR: u8 = 0x19;
pub(crate) const PLAYER_RADIUS: f64 = 0x5800 as f64 / 65_536.0;
// WL_DEF.H's MINACTORDIST, applied independently to both axes in TryMove.
const MIN_ACTOR_DISTANCE: f64 = 1.0;

#[derive(Clone, Copy)]
struct Viewport {
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
    width: usize,
    height: usize,
    center_x: f64,
}

impl Viewport {
    fn current() -> Self {
        let width = view_width();
        let height = width / 2;
        let left = (WIDTH - width) / 2;
        let top = (HEIGHT - 40 - height) / 2;
        Self {
            left,
            top,
            right: left + width,
            bottom: top + height,
            width,
            height,
            center_x: (left + width / 2 - 1) as f64,
        }
    }

    fn scaled(scale: usize) -> Self {
        if scale == 1 {
            return Self::current();
        }
        let width = view_width() * scale;
        let height = width / 2;
        let screen_width = WIDTH * scale;
        let screen_height = HEIGHT * scale;
        let left = (screen_width - width) / 2;
        let top = (screen_height - 40 * scale - height) / 2;
        Self {
            left,
            top,
            right: left + width,
            bottom: top + height,
            width,
            height,
            center_x: (left + width / 2 - 1) as f64,
        }
    }
}

pub struct HudPictures {
    first_chunk: usize,
    pictures: Vec<Option<Picture>>,
    shareware: bool,
}

pub struct PauseSoundMenuPictures {
    paused: Picture,
    options_panel: Picture,
    control_panel: Picture,
    customize_panel: Picture,
    load_game: Picture,
    save_game: Picture,
    disk_loading1: Picture,
    disk_loading2: Picture,
    cursor: Picture,
    cursor_alt: Picture,
    not_selected: Picture,
    selected: Picture,
    effects_title: Picture,
    digitized_title: Picture,
    music_title: Picture,
    mouse_background: Picture,
}

impl PauseSoundMenuPictures {
    pub fn load(
        archive: &crate::vga::VgaArchive,
        extension: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let cursor_chunk = if extension.eq_ignore_ascii_case("WL1") {
            22
        } else {
            11
        };
        let customize_panel_chunk = if extension.eq_ignore_ascii_case("WL1") {
            38
        } else {
            27
        };
        let control_panel_chunk = if extension.eq_ignore_ascii_case("WL1") {
            37
        } else {
            26
        };
        let options_panel_chunk = if extension.eq_ignore_ascii_case("WL1") {
            21
        } else {
            10
        };
        let load_game_chunk = if extension.eq_ignore_ascii_case("WL1") {
            39
        } else {
            28
        };
        let disk_loading1_chunk = if extension.eq_ignore_ascii_case("WL1") {
            35
        } else {
            24
        };
        let paused_chunk = if extension.eq_ignore_ascii_case("WL1") {
            137
        } else {
            133
        };
        // These shared menu pictures follow C_* order in GFXV_WL1/WL6.EQU.
        let load_picture = |chunk| -> Result<Picture, Box<dyn std::error::Error>> {
            archive
                .picture(chunk)?
                .ok_or_else(|| format!("VGA menu picture chunk {chunk} is sparse").into())
        };
        Ok(Self {
            paused: load_picture(paused_chunk)?,
            options_panel: archive.options_title(options_panel_chunk)?,
            control_panel: load_picture(control_panel_chunk)?,
            customize_panel: load_picture(customize_panel_chunk)?,
            load_game: load_picture(load_game_chunk)?,
            save_game: load_picture(load_game_chunk + 1)?,
            disk_loading1: load_picture(disk_loading1_chunk)?,
            disk_loading2: load_picture(disk_loading1_chunk + 1)?,
            cursor: load_picture(cursor_chunk)?,
            cursor_alt: load_picture(cursor_chunk + 1)?,
            not_selected: load_picture(cursor_chunk + 2)?,
            selected: load_picture(cursor_chunk + 3)?,
            effects_title: load_picture(cursor_chunk + 4)?,
            digitized_title: load_picture(cursor_chunk + 5)?,
            music_title: load_picture(cursor_chunk + 6)?,
            mouse_background: load_picture(cursor_chunk + 7)?,
        })
    }

    pub fn controls_art(&self) -> [Picture; 2] {
        [self.customize_panel.clone(), self.mouse_background.clone()]
    }

    pub fn control_panel_art(&self) -> [Picture; 6] {
        [
            self.control_panel.clone(),
            self.mouse_background.clone(),
            self.cursor.clone(),
            self.cursor_alt.clone(),
            self.not_selected.clone(),
            self.selected.clone(),
        ]
    }

    pub fn sound_settings_art(&self) -> [Picture; 7] {
        [
            self.mouse_background.clone(),
            self.effects_title.clone(),
            self.digitized_title.clone(),
            self.music_title.clone(),
            self.not_selected.clone(),
            self.selected.clone(),
            self.cursor.clone(),
        ]
    }

    pub fn mouse_sensitivity_art(&self) -> [Picture; 1] {
        [self.mouse_background.clone()]
    }
}

pub(crate) fn render_pause_key_confirmation(pixels: &mut [u32], pictures: &PauseSoundMenuPictures) {
    draw_picture(&pictures.paused, 16, 64, pixels);
}

#[derive(Clone, Copy)]
pub struct HudValues {
    pub health: u8,
    pub face_frame: u8,
    pub mutant_death_face: bool,
    pub gatling_pickup_face: bool,
    pub ammo: u8,
    pub lives: u8,
    pub score: u32,
    pub map_number: usize,
    pub keys: u8,
}

impl HudPictures {
    pub fn load(
        archive: &crate::vga::VgaArchive,
        extension: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let shareware = extension.eq_ignore_ascii_case("WL1");
        let first_chunk = if shareware { 49 } else { 44 };
        let last_chunk = if shareware { 136 } else { 132 };
        let pictures = (first_chunk..=last_chunk)
            .map(|chunk| archive.picture(chunk))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            first_chunk,
            pictures,
            shareware,
        })
    }

    fn picture(&self, chunk: usize) -> Option<&Picture> {
        chunk
            .checked_sub(self.first_chunk)
            .and_then(|index| self.pictures.get(index))
            .and_then(Option::as_ref)
    }

    fn draw_dos_text(&self, pixels: &mut [u32], x: usize, y: usize, text: &str) {
        let glyph_base = if self.shareware { 49 } else { 44 };
        let (origin_x, mut cursor_x, mut cursor_y) = (x, x, y);
        for byte in text.bytes() {
            if byte == b'\n' {
                cursor_x = origin_x;
                cursor_y += 16;
                continue;
            }

            let uppercase = byte.to_ascii_uppercase();
            let (chunk, advance) = match uppercase {
                b'0'..=b'9' => (Some(glyph_base + 1 + usize::from(uppercase - b'0')), 16),
                b'A'..=b'Z' => (Some(glyph_base + 12 + usize::from(uppercase - b'A')), 16),
                b':' => (Some(glyph_base), 8),
                b'%' => (Some(glyph_base + 11), 16),
                b'!' => (Some(glyph_base + 38), 8),
                b'\'' if !self.shareware => (Some(glyph_base + 39), 8),
                _ => (None, 16),
            };
            if let Some(picture) = chunk.and_then(|chunk| self.picture(chunk)) {
                draw_picture(picture, cursor_x, cursor_y, pixels);
            }
            cursor_x += advance;
        }
    }

    fn chunk_offsets(&self) -> (usize, usize, usize, usize, usize) {
        if self.shareware {
            (102, 103, 50, 99, 113)
        } else {
            (98, 99, 45, 95, 109)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoiceMenuLayout {
    Main,
    StartupMain,
    Controls,
    StartupControls,
    StartupControlPanel,
    StartupSound,
    MouseSensitivity,
    Episode,
    Difficulty,
    Jukebox,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Player {
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    #[serde(default)]
    pub(crate) turn_remainder: i8,
    #[serde(skip)]
    pub(crate) actor_tile_override: Option<(usize, usize)>,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            x: 2.5,
            y: 2.5,
            angle: 0.0,
            turn_remainder: 0,
            actor_tile_override: None,
        }
    }
}

impl Player {
    pub fn at_map_start(map: &Map) -> Result<Self, String> {
        let (x, y, direction) = map
            .player_start()
            .ok_or_else(|| format!("map '{}' has no player start marker", map.name))?;
        // The original info plane uses 19=N, 20=E, 21=S, 22=W.
        let angle = (direction as i32 - 20) as f64 * std::f64::consts::FRAC_PI_2;
        Ok(Self {
            x: x as f64 + 0.5,
            y: y as f64 + 0.5,
            angle,
            turn_remainder: 0,
            actor_tile_override: None,
        })
    }

    pub(crate) fn actor_tile(&self) -> (usize, usize) {
        self.actor_tile_override
            .unwrap_or_else(|| (self.x.floor() as usize, self.y.floor() as usize))
    }

    pub(crate) fn freeze_actor_tile(&mut self, x: usize, y: usize) {
        self.actor_tile_override = Some((x, y));
    }

    pub(crate) fn use_direction(&self) -> (i8, i8) {
        // Cmd_Use compares whole DOS degrees, which increase toward north.
        let angle = (-self.angle.to_degrees().round()).rem_euclid(360.0) as u16;
        match angle {
            0..=44 | 316..=359 => (1, 0),
            45..=134 => (0, -1),
            135..=224 => (-1, 0),
            _ => (0, 1),
        }
    }

    pub fn update(
        &mut self,
        map: &Map,
        doors: &DoorSystem,
        pushwalls: &PushWallSystem,
        forward: f64,
        strafe: f64,
        turn: f64,
        mouse_forward_control: f64,
        mouse_strafe_control: f64,
        mouse_turn_control: f64,
        running: bool,
        actor_blockers: &[(f64, f64)],
        dt: f64,
    ) -> bool {
        let mut clipped = false;
        let steps = (dt / (1.0 / 70.0)).ceil().max(1.0) as usize;
        let step_dt = dt / steps as f64;
        for step in 0..steps {
            let control_step = if running { 70.0 } else { 35.0 };
            let mouse_turn_before =
                (mouse_turn_control * step as f64 / steps as f64).round() as i32;
            let mouse_turn_after =
                (mouse_turn_control * (step + 1) as f64 / steps as f64).round() as i32;
            let mouse_turn_step = mouse_turn_after - mouse_turn_before;
            let turn_units = (turn * control_step * 70.0 * step_dt).round() as i32
                // DOS angle increases toward north while this screen-space
                // angle increases toward south, so mouse X has the opposite
                // sign after translating the DOS control value.
                + mouse_turn_step
                + i32::from(self.turn_remainder);
            let degrees = turn_units / 20;
            self.turn_remainder = (turn_units % 20) as i8;
            let heading = (self.angle.to_degrees().round() + f64::from(degrees)).rem_euclid(360.0);
            self.angle = heading.to_radians();
            let thrust_scale = control_step * 70.0 / 65_536.0 * step_dt;
            let mouse_forward_before =
                (mouse_forward_control * step as f64 / steps as f64).round() as i32;
            let mouse_forward_after =
                (mouse_forward_control * (step + 1) as f64 / steps as f64).round() as i32;
            let mouse_forward_step = mouse_forward_after - mouse_forward_before;
            let total_forward = forward - f64::from(mouse_forward_step) / control_step;
            let forward_scale = if total_forward < 0.0 { 100.0 } else { 150.0 };
            let distance = total_forward * forward_scale * thrust_scale;
            let mouse_strafe_before =
                (mouse_strafe_control * step as f64 / steps as f64).round() as i32;
            let mouse_strafe_after =
                (mouse_strafe_control * (step + 1) as f64 / steps as f64).round() as i32;
            let mouse_strafe_step = mouse_strafe_after - mouse_strafe_before;
            let side_distance =
                (strafe - f64::from(mouse_strafe_step) / control_step) * 150.0 * thrust_scale;
            let (sin, cos) = self.angle.sin_cos();
            // ControlMovement applies and clips the two thrusts in this order.
            for (dx, dy) in [
                (-sin * side_distance, cos * side_distance),
                (cos * distance, sin * distance),
            ] {
                if dx == 0.0 && dy == 0.0 {
                    continue;
                }
                let (nx, ny) = (self.x + dx, self.y + dy);
                if !is_wall(map, doors, pushwalls, actor_blockers, nx, ny) {
                    self.x = nx;
                    self.y = ny;
                    continue;
                }
                clipped = true;
                if dx != 0.0 && !is_wall(map, doors, pushwalls, actor_blockers, nx, self.y) {
                    self.x = nx;
                } else if dy != 0.0 && !is_wall(map, doors, pushwalls, actor_blockers, self.x, ny) {
                    self.y = ny;
                }
            }
        }
        clipped
    }
}

fn is_wall(
    map: &Map,
    doors: &DoorSystem,
    pushwalls: &PushWallSystem,
    actor_blockers: &[(f64, f64)],
    x: f64,
    y: f64,
) -> bool {
    if pushwalls.blocks_point(x, y, PLAYER_RADIUS)
        || actor_blockers.iter().any(|&(actor_x, actor_y)| {
            (x - actor_x).abs() <= MIN_ACTOR_DISTANCE && (y - actor_y).abs() <= MIN_ACTOR_DISTANCE
        })
    {
        return true;
    }

    let min_x = (x - PLAYER_RADIUS).floor() as isize;
    let max_x = (x + PLAYER_RADIUS).floor() as isize;
    let min_y = (y - PLAYER_RADIUS).floor() as isize;
    let max_y = (y + PLAYER_RADIUS).floor() as isize;
    for tile_y in min_y..=max_y {
        for tile_x in min_x..=max_x {
            if tile_x < 0
                || tile_y < 0
                || tile_x >= map.width as isize
                || tile_y >= map.height as isize
            {
                return true;
            }
            let (tile_x, tile_y) = (tile_x as usize, tile_y as usize);
            if pushwalls.blocks_reserved_tile(tile_x, tile_y) {
                return true;
            }
            let tile = map.tile(tile_x, tile_y);
            let blocking = static_blocks_tile(map.planes[1][tile_y * 64 + tile_x]);
            let is_door = doors.at(tile_x, tile_y).is_some();
            if is_door && !doors.point_passable_at(tile_x, tile_y, x, y, PLAYER_RADIUS) {
                return true;
            }
            if (!is_door && tile != 0 && tile < 107) || blocking {
                // DOS TryMove checks every tile touched by the player's
                // axis-aligned PLAYERSIZE box; diagonal corner grazing blocks.
                return true;
            }
        }
    }
    false
}

pub fn position_is_clear(
    map: &Map,
    doors: &DoorSystem,
    pushwalls: &PushWallSystem,
    actor_blockers: &[(f64, f64)],
    x: f64,
    y: f64,
) -> bool {
    !is_wall(map, doors, pushwalls, actor_blockers, x, y)
}

/// Draws the active map into the original framebuffer and optionally produces a
/// separately rasterized high-resolution world frame for presentation.
pub fn render(
    map: &Map,
    textures: Option<&Vswap>,
    doors: &DoorSystem,
    pushwalls: &PushWallSystem,
    player: &Player,
    statics: &[ActorSpawn],
    enemies: &[ActorView],
    sprites: &SpriteAtlas,
    font: Option<&BitmapFont>,
    status_bar: Option<&Picture>,
    hud_pictures: Option<&HudPictures>,
    weapon_picture: Option<&Picture>,
    player_weapon_shape: Option<usize>,
    hud_values: HudValues,
    hud_text: &str,
    pixels: &mut [u32],
) -> Option<(usize, usize, Vec<u32>)> {
    let profile_started = profiling_enabled().then(Instant::now);
    if pixels.len() < WIDTH * HEIGHT {
        return None;
    }
    let show_historical_symbols = crate::settings::show_historical_symbols();
    let ceiling = CEILING_COLORS.get(map.number).copied().unwrap_or(0x1d);
    let viewport = Viewport::current();
    let focal = viewport.width as f64 * VIEW_FACE_DISTANCE;
    let renderer = crate::settings::world_renderer();
    let sample_count = renderer.sample_count();
    let subpixel_rendering = sample_count > 1;
    let ray_origin = view_origin(player);
    draw_play_border(pixels, viewport);
    let mut wall_depths = vec![f64::INFINITY; WIDTH];
    let mut visible_tiles = vec![false; map.width * map.height];
    for x in viewport.left..viewport.right {
        let view_x = x - viewport.left;
        let mut sample_depths = [f64::INFINITY; 4];
        let mut sample_columns = [[[0xff00_0000; HEIGHT]; 4]; 4];
        for sample_index in 0..sample_count {
            let sample_offset = (sample_index as f64 + 0.5) / sample_count as f64;
            let pixel_offset = view_x as f64 + sample_offset - viewport.width as f64 / 2.0;
            let ray_angle = player.angle + (pixel_offset / focal).atan();
            let hit = cast_ray_from_origin(
                map,
                doors,
                pushwalls,
                ray_origin,
                ray_angle,
                pushwalls.tilemap_blocking_cell(),
                &mut visible_tiles,
            );
            let corrected = (hit.distance * (ray_angle - player.angle).cos()).max(0.05);
            sample_depths[sample_index] = corrected;
            let wall = prepare_wall_sampler(
                &hit,
                corrected,
                viewport,
                focal,
                ceiling,
                textures,
                doors,
                show_historical_symbols,
                subpixel_rendering,
            );
            for (row, y) in (viewport.top..viewport.bottom).enumerate() {
                for vertical_index in 0..sample_count {
                    let vertical_offset = if subpixel_rendering {
                        (vertical_index as f64 + 0.5) / sample_count as f64
                    } else {
                        0.0
                    };
                    sample_columns[sample_index][vertical_index][row] =
                        wall.color_at(y as f64 + vertical_offset);
                }
            }
        }
        for row in 0..viewport.height {
            let color = average_sample_grid(&sample_columns, sample_count, row);
            pixels[(viewport.top + row) * WIDTH + x] = color;
        }
        wall_depths[x] = sample_depths[..sample_count]
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
    }
    draw_statics(
        statics,
        enemies,
        sprites,
        player,
        &wall_depths,
        &visible_tiles,
        map,
        viewport,
        WIDTH,
        pixels,
    );
    if let Some(sprite) = player_weapon_shape.and_then(|shape| sprites.sprite(shape)) {
        draw_player_weapon(sprite, viewport, WIDTH, pixels);
    }
    let hud_y = if let Some(status_bar) = status_bar {
        let y = HEIGHT.saturating_sub(status_bar.height);
        draw_picture(status_bar, 0, y, pixels);
        if let Some(weapon_picture) = weapon_picture {
            draw_picture(weapon_picture, 32 * 8, y + 8, pixels);
        }
        y.saturating_add(2)
    } else if let Some(font) = font {
        let bar_height = (font.height() * 2 + 3).min(HEIGHT);
        pixels[..WIDTH * bar_height].fill(0xFF000000);
        1
    } else {
        0
    };
    if let (Some(hud), Some(status_bar)) = (hud_pictures, status_bar) {
        draw_status_hud(pixels, hud, hud_values, status_bar.height);
    } else if let Some(font) = font {
        font.draw_text(pixels, WIDTH, HEIGHT, 4, hud_y, hud_text, 0xFFFFFFFF);
    }
    let base_render_ns = profile_started.map(|started| started.elapsed().as_nanos() as u64);
    let scale = crate::settings::render_resolution().scale();
    let high_resolution_frame = (scale > 1).then(|| {
        let frame = render_scaled_scene(
            scale,
            map,
            textures,
            doors,
            pushwalls,
            player,
            statics,
            enemies,
            sprites,
            player_weapon_shape,
            pixels,
        );
        (WIDTH * scale, HEIGHT * scale, frame)
    });
    if let (Some(started), Some(base_ns)) = (profile_started, base_render_ns) {
        record_render_profile(
            base_ns,
            started.elapsed().as_nanos() as u64 - base_ns,
            scale,
        );
    }
    high_resolution_frame
}

fn profiling_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("WOLF3D_PROFILE_RENDER").is_some())
}

fn record_render_profile(base_ns: u64, high_resolution_ns: u64, scale: usize) {
    static FRAMES: AtomicUsize = AtomicUsize::new(0);
    static BASE_NS: AtomicU64 = AtomicU64::new(0);
    static HIGH_RES_NS: AtomicU64 = AtomicU64::new(0);
    BASE_NS.fetch_add(base_ns, Ordering::Relaxed);
    HIGH_RES_NS.fetch_add(high_resolution_ns, Ordering::Relaxed);
    let frames = FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
    if frames == 120 {
        let base_ms = BASE_NS.swap(0, Ordering::Relaxed) as f64 / 120_000_000.0;
        let high_res_ms = HIGH_RES_NS.swap(0, Ordering::Relaxed) as f64 / 120_000_000.0;
        FRAMES.store(0, Ordering::Relaxed);
        eprintln!(
            "[render profile, 120 frames] base={base_ms:.2} ms/frame, high-resolution={high_res_ms:.2} ms/frame (scale {scale})"
        );
    }
}

fn record_scaled_scene_profile(upscale_ns: u64, walls_ns: u64, overlays_ns: u64, scale: usize) {
    static FRAMES: AtomicUsize = AtomicUsize::new(0);
    static UPSCALE_NS: AtomicU64 = AtomicU64::new(0);
    static WALLS_NS: AtomicU64 = AtomicU64::new(0);
    static OVERLAYS_NS: AtomicU64 = AtomicU64::new(0);
    UPSCALE_NS.fetch_add(upscale_ns, Ordering::Relaxed);
    WALLS_NS.fetch_add(walls_ns, Ordering::Relaxed);
    OVERLAYS_NS.fetch_add(overlays_ns, Ordering::Relaxed);
    let frames = FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
    if frames == 120 {
        let upscale_ms = UPSCALE_NS.swap(0, Ordering::Relaxed) as f64 / 120_000_000.0;
        let walls_ms = WALLS_NS.swap(0, Ordering::Relaxed) as f64 / 120_000_000.0;
        let overlays_ms = OVERLAYS_NS.swap(0, Ordering::Relaxed) as f64 / 120_000_000.0;
        FRAMES.store(0, Ordering::Relaxed);
        eprintln!(
            "[supersampling profile, 120 frames] base-image={upscale_ms:.2} ms/frame, rays-and-walls={walls_ms:.2} ms/frame, sprites-and-weapon={overlays_ms:.2} ms/frame (scale {scale})"
        );
    }
}

fn render_scaled_scene(
    scale: usize,
    map: &Map,
    textures: Option<&Vswap>,
    doors: &DoorSystem,
    pushwalls: &PushWallSystem,
    player: &Player,
    statics: &[ActorSpawn],
    enemies: &[ActorView],
    sprites: &SpriteAtlas,
    player_weapon_shape: Option<usize>,
    base_frame: &[u32],
) -> Vec<u32> {
    let profile_started = profiling_enabled().then(Instant::now);
    let width = WIDTH * scale;
    let height = HEIGHT * scale;
    let mut pixels = vec![0xff00_0000; width * height];
    for source_y in 0..HEIGHT {
        let source_row = source_y * WIDTH;
        for vertical_sample in 0..scale {
            let target_row = (source_y * scale + vertical_sample) * width;
            for source_x in 0..WIDTH {
                let target_x = source_x * scale;
                pixels[target_row + target_x..target_row + target_x + scale]
                    .fill(base_frame[source_row + source_x]);
            }
        }
    }
    let upscale_ns = profile_started.map(|started| started.elapsed().as_nanos() as u64);
    let viewport = Viewport::scaled(scale);
    let focal = viewport.width as f64 * VIEW_FACE_DISTANCE;
    let ray_origin = view_origin(player);
    let ceiling = CEILING_COLORS.get(map.number).copied().unwrap_or(0x1d);
    let show_historical_symbols = crate::settings::show_historical_symbols();
    let sample_count = crate::settings::world_renderer().sample_count();
    let worker_count = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .min(viewport.width)
        .max(1);
    let columns_per_worker = viewport.width.div_ceil(worker_count);
    let chunks = thread::scope(|scope| {
        let mut workers = Vec::with_capacity(worker_count);
        for worker in 0..worker_count {
            let x_start = viewport.left + worker * columns_per_worker;
            let x_end = (x_start + columns_per_worker).min(viewport.right);
            if x_start >= x_end {
                break;
            }
            workers.push(scope.spawn(move || {
                render_scaled_columns(
                    x_start,
                    x_end,
                    map,
                    textures,
                    doors,
                    pushwalls,
                    player,
                    ray_origin,
                    viewport,
                    focal,
                    ceiling,
                    sample_count,
                    show_historical_symbols,
                )
            }));
        }
        workers
            .into_iter()
            .map(|worker| worker.join().expect("scaled renderer worker panicked"))
            .collect::<Vec<_>>()
    });
    let mut wall_depths = vec![f64::INFINITY; width];
    let mut visible_tiles = vec![false; map.width * map.height];
    for chunk in chunks {
        let chunk_width = chunk.x_end - chunk.x_start;
        for row in 0..viewport.height {
            let target_start = (viewport.top + row) * width + chunk.x_start;
            let source_start = row * chunk_width;
            pixels[target_start..target_start + chunk_width]
                .copy_from_slice(&chunk.pixels[source_start..source_start + chunk_width]);
        }
        wall_depths[chunk.x_start..chunk.x_end].copy_from_slice(&chunk.wall_depths);
        for (visible, chunk_visible) in visible_tiles.iter_mut().zip(chunk.visible_tiles) {
            *visible |= chunk_visible;
        }
    }
    let wall_end_ns = profile_started.map(|started| started.elapsed().as_nanos() as u64);
    draw_statics(
        statics,
        enemies,
        sprites,
        player,
        &wall_depths,
        &visible_tiles,
        map,
        viewport,
        width,
        pixels.as_mut_slice(),
    );
    // HUD and menus remain sourced from the original 320x200 framebuffer.
    // The first-person weapon belongs to the world and is rasterized at target resolution.
    if let Some(sprite) = player_weapon_shape.and_then(|shape| sprites.sprite(shape)) {
        draw_player_weapon(sprite, viewport, width, pixels.as_mut_slice());
    }
    if let (Some(started), Some(upscale_ns), Some(wall_end_ns)) =
        (profile_started, upscale_ns, wall_end_ns)
    {
        let total_ns = started.elapsed().as_nanos() as u64;
        record_scaled_scene_profile(
            upscale_ns,
            wall_end_ns - upscale_ns,
            total_ns - wall_end_ns,
            scale,
        );
    }
    pixels
}

struct ScaledColumnChunk {
    x_start: usize,
    x_end: usize,
    pixels: Vec<u32>,
    wall_depths: Vec<f64>,
    visible_tiles: Vec<bool>,
}

#[allow(clippy::too_many_arguments)]
fn render_scaled_columns(
    x_start: usize,
    x_end: usize,
    map: &Map,
    textures: Option<&Vswap>,
    doors: &DoorSystem,
    pushwalls: &PushWallSystem,
    player: &Player,
    ray_origin: (f64, f64),
    viewport: Viewport,
    focal: f64,
    ceiling: u8,
    sample_count: usize,
    show_historical_symbols: bool,
) -> ScaledColumnChunk {
    let chunk_width = x_end - x_start;
    let mut pixels = vec![0xff00_0000; chunk_width * viewport.height];
    let mut wall_depths = vec![f64::INFINITY; chunk_width];
    let mut visible_tiles = vec![false; map.width * map.height];
    for x in x_start..x_end {
        let view_x = x - viewport.left;
        let mut sample_depths = [f64::INFINITY; 4];
        let mut sample_columns = [[[0xff00_0000; HEIGHT * 4]; 4]; 4];
        for horizontal in 0..sample_count {
            let pixel_offset = view_x as f64 + (horizontal as f64 + 0.5) / sample_count as f64
                - viewport.width as f64 / 2.0;
            let ray_angle = player.angle + (pixel_offset / focal).atan();
            let hit = cast_ray_from_origin(
                map,
                doors,
                pushwalls,
                ray_origin,
                ray_angle,
                pushwalls.tilemap_blocking_cell(),
                &mut visible_tiles,
            );
            let corrected = (hit.distance * (ray_angle - player.angle).cos()).max(0.05);
            sample_depths[horizontal] = corrected;
            let wall = prepare_wall_sampler(
                &hit,
                corrected,
                viewport,
                focal,
                ceiling,
                textures,
                doors,
                show_historical_symbols,
                true,
            );
            for (row, y) in (viewport.top..viewport.bottom).enumerate() {
                for vertical in 0..sample_count {
                    let sample_y = y as f64 + (vertical as f64 + 0.5) / sample_count as f64;
                    sample_columns[horizontal][vertical][row] = wall.color_at(sample_y);
                }
            }
        }
        let local_x = x - x_start;
        for row in 0..viewport.height {
            pixels[row * chunk_width + local_x] =
                average_sample_grid(&sample_columns, sample_count, row);
        }
        wall_depths[local_x] = sample_depths[..sample_count]
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
    }
    ScaledColumnChunk {
        x_start,
        x_end,
        pixels,
        wall_depths,
        visible_tiles,
    }
}

fn draw_play_border(pixels: &mut [u32], view: Viewport) {
    // WL_GAME.C's DrawPlayBorder uses the VGA palette directly.
    let shadow = palette_color(0, 1.0);
    let highlight = palette_color(125, 1.0);
    pixels[..WIDTH * HEIGHT].fill(palette_color(127, 1.0));

    let left = view.left - 1;
    let top = view.top - 1;
    pixels[top * WIDTH + left..=top * WIDTH + view.right].fill(shadow);
    pixels[view.bottom * WIDTH + left..=view.bottom * WIDTH + view.right].fill(highlight);
    for y in top..=view.bottom {
        pixels[y * WIDTH + left] = shadow;
        pixels[y * WIDTH + view.right] = highlight;
    }
    pixels[view.bottom * WIDTH + left] = palette_color(124, 1.0);
}

fn draw_status_hud(pixels: &mut [u32], hud: &HudPictures, values: HudValues, bar_height: usize) {
    let bar_y = HEIGHT.saturating_sub(bar_height);
    let (blank_chunk, digit_chunk, _, key_chunk, face_chunk) = hud.chunk_offsets();
    draw_latched_number(
        pixels,
        hud,
        6,
        16,
        6,
        values.score,
        digit_chunk,
        blank_chunk,
        bar_y,
    );
    draw_latched_number(
        pixels,
        hud,
        14,
        16,
        1,
        values.lives as u32,
        digit_chunk,
        blank_chunk,
        bar_y,
    );
    draw_latched_number(
        pixels,
        hud,
        21,
        16,
        3,
        values.health as u32,
        digit_chunk,
        blank_chunk,
        bar_y,
    );
    draw_latched_number(
        pixels,
        hud,
        27,
        16,
        2,
        values.ammo as u32,
        digit_chunk,
        blank_chunk,
        bar_y,
    );
    draw_latched_number(
        pixels,
        hud,
        2,
        16,
        2,
        values.map_number.saturating_add(1) as u32,
        digit_chunk,
        blank_chunk,
        bar_y,
    );
    let no_key = hud.picture(key_chunk);
    let gold_key = if values.keys & 1 != 0 {
        hud.picture(key_chunk + 1)
    } else {
        no_key
    };
    let silver_key = if values.keys & 2 != 0 {
        hud.picture(key_chunk + 2)
    } else {
        no_key
    };
    if let Some(picture) = gold_key {
        draw_picture(picture, 30 * 8, bar_y + 4, pixels);
    }
    if let Some(picture) = silver_key {
        draw_picture(picture, 30 * 8, bar_y + 20, pixels);
    }
    let face = if values.health == 0 {
        hud.picture(face_chunk + if values.mutant_death_face { 23 } else { 21 })
    } else if values.gatling_pickup_face {
        hud.picture(face_chunk + 22)
    } else {
        let expression = ((100usize.saturating_sub(values.health as usize)) / 16).min(6);
        hud.picture(face_chunk + expression * 3 + usize::from(values.face_frame.min(2)))
    };
    if let Some(picture) = face {
        draw_picture(picture, 17 * 8, bar_y + 4, pixels);
    }
}

fn draw_latched_number(
    pixels: &mut [u32],
    hud: &HudPictures,
    x: usize,
    y: usize,
    width: usize,
    value: u32,
    digit_chunk: usize,
    blank_chunk: usize,
    bar_y: usize,
) {
    let digits = value.to_string();
    let shown = if digits.len() > width {
        &digits[digits.len() - width..]
    } else {
        &digits
    };
    let blank_count = width.saturating_sub(shown.len());
    for index in 0..width {
        let chunk = if index < blank_count {
            blank_chunk
        } else {
            digit_chunk + (shown.as_bytes()[index - blank_count] - b'0') as usize
        };
        if let Some(picture) = hud.picture(chunk) {
            draw_picture(picture, (x + index) * 8, bar_y + y, pixels);
        }
    }
}

pub fn render_intermission(
    pixels: &mut [u32],
    hud: &HudPictures,
    portrait: &Picture,
    map_number: usize,
    rows: &[String],
    score: u32,
    lives: u8,
) {
    if pixels.len() < WIDTH * HEIGHT {
        return;
    }
    // DOS keeps the 40-line status panel outside the intermission clip area.
    pixels[..WIDTH * 160].fill(palette_color(127, 1.0));
    draw_picture(portrait, 0, 16, pixels);
    if map_number % 10 < 8 {
        hud.draw_dos_text(pixels, 14 * 8, 2 * 8, "FLOOR\nCOMPLETED");
        hud.draw_dos_text(pixels, 14 * 8, 7 * 8, "BONUS     ");
        hud.draw_dos_text(pixels, 16 * 8, 10 * 8, "TIME");
        hud.draw_dos_text(pixels, 16 * 8, 12 * 8, " PAR");
        hud.draw_dos_text(pixels, 9 * 8, 14 * 8, "KILL RATIO    %");
        hud.draw_dos_text(pixels, 5 * 8, 16 * 8, "SECRET RATIO    %");
        hud.draw_dos_text(pixels, 1 * 8, 18 * 8, "TREASURE RATIO    %");
        hud.draw_dos_text(pixels, 26 * 8, 2 * 8, &(map_number % 10 + 1).to_string());

        if let Some(row) = rows.first() {
            let fields = row.split_whitespace().collect::<Vec<_>>();
            if fields.len() >= 4 {
                // WL_INTER.C starts the current-time glyphs at i=26*8.
                hud.draw_dos_text(pixels, 26 * 8, 10 * 8, fields[1]);
                hud.draw_dos_text(pixels, 26 * 8, 12 * 8, fields[3]);
            }
        }

        if let Some(bonus) = rows.get(4).and_then(|row| row.split_whitespace().last()) {
            if let Ok(bonus) = bonus.parse::<u32>() {
                let bonus = bonus.to_string();
                let x = 36usize.saturating_sub(bonus.len() * 2) * 8;
                hud.draw_dos_text(pixels, x, 7 * 8, &bonus);
            }
        }

        for (index, y) in [14usize, 16, 18].into_iter().enumerate() {
            if let Some(ratio) = rows
                .get(index + 1)
                .and_then(|row| row.split_whitespace().nth(1))
                .and_then(|ratio| ratio.trim_end_matches('%').parse::<u8>().ok())
            {
                let ratio = ratio.to_string();
                let x = 37usize.saturating_sub(ratio.len() * 2) * 8;
                hud.draw_dos_text(pixels, x, y * 8, &ratio);
            }
        }
    } else {
        hud.draw_dos_text(pixels, 14 * 8, 4 * 8, "SECRET FLOOR\n COMPLETED!");
        hud.draw_dos_text(pixels, 10 * 8, 16 * 8, "15000 BONUS!");
    }

    let (blank_chunk, digit_chunk, _, _, _) = hud.chunk_offsets();
    draw_latched_number(pixels, hud, 6, 16, 6, score, digit_chunk, blank_chunk, 160);
    draw_latched_number(
        pixels,
        hud,
        14,
        16,
        1,
        lives as u32,
        digit_chunk,
        blank_chunk,
        160,
    );
}

pub fn render_victory_screen(
    pixels: &mut [u32],
    hud: &HudPictures,
    hero: &Picture,
    elapsed_seconds: u32,
    averages: [u8; 3],
) {
    if pixels.len() < WIDTH * HEIGHT {
        return;
    }
    pixels[..WIDTH * 160].fill(palette_color(127, 1.0));
    draw_picture(hero, 8, 4, pixels);
    let minutes = elapsed_seconds / 60;
    let (minutes, seconds) = if minutes > 99 {
        (99, 99)
    } else {
        (minutes, elapsed_seconds % 60)
    };
    // WL_INTER.C's Write draws these original 16x16 L_*PIC glyphs.
    hud.draw_dos_text(pixels, 18 * 8, 2 * 8, "YOU WIN!");
    hud.draw_dos_text(pixels, 14 * 8, 6 * 8, "TOTAL TIME");
    hud.draw_dos_text(
        pixels,
        14 * 8 + 1,
        8 * 8,
        &format!("{minutes:02}:{seconds:02}"),
    );
    hud.draw_dos_text(pixels, 12 * 8, 12 * 8, "AVERAGES");
    hud.draw_dos_text(pixels, 14 * 8, 14 * 8, "KILL");
    hud.draw_dos_text(pixels, 10 * 8, 16 * 8, "SECRET");
    hud.draw_dos_text(pixels, 6 * 8, 18 * 8, "TREASURE");
    for (index, y) in [112, 128, 144].into_iter().enumerate() {
        let value = averages[index].to_string();
        let x = (6 + 24 - value.len() * 2) * 8;
        hud.draw_dos_text(pixels, x, y, &value);
    }
}

pub fn render_high_score_screen(
    pixels: &mut [u32],
    font: &BitmapFont,
    table: &crate::highscores::Table,
    entry_name: Option<&str>,
    editing_rank: Option<usize>,
    title_picture: Option<&Picture>,
    column_pictures: [Option<&Picture>; 3],
    shareware: bool,
) {
    if pixels.len() < WIDTH * HEIGHT {
        return;
    }
    pixels[..WIDTH * HEIGHT].fill(palette_color(0x2d, 1.0));
    for stripe in 0..10 {
        let y = 16 + stripe * 16;
        draw_filled_rect(pixels, 0, y, WIDTH, 8, palette_color(0x29, 1.0));
    }
    if let Some(picture) = title_picture {
        draw_picture(picture, if shareware { 0 } else { 48 }, 0, pixels);
    } else {
        font.draw_text(
            pixels,
            WIDTH,
            HEIGHT,
            96,
            24,
            "HIGH SCORES",
            palette_color(15, 1.0),
        );
    }
    let headings = ["NAME", "LEVEL", "SCORE"];
    let heading_x = [32, 160, 224];
    for index in 0..3 {
        if let Some(picture) = column_pictures[index] {
            draw_picture(picture, heading_x[index], 68, pixels);
        } else {
            font.draw_text(
                pixels,
                WIDTH,
                HEIGHT,
                heading_x[index],
                68,
                headings[index],
                palette_color(15, 1.0),
            );
        }
    }
    for (index, entry) in table.entries.iter().enumerate() {
        let y = 76 + index * 16;
        let color = if editing_rank == Some(index) {
            palette_color(15, 1.0)
        } else {
            0xFFFFFFFF
        };
        let name = if editing_rank == Some(index) {
            entry_name.unwrap_or(&entry.name)
        } else {
            &entry.name
        };
        let mut display_name = name.to_owned();
        while font.text_width(&display_name) > 160 {
            display_name.pop();
        }
        font.draw_text(pixels, WIDTH, HEIGHT, 32, y, &display_name, color);
        let floor = format!("E{}/L{}", entry.episode.saturating_add(1), entry.completed);
        let floor_x = 184usize.saturating_sub(font.text_width(&floor));
        font.draw_text(pixels, WIDTH, HEIGHT, floor_x, y, &floor, color);
        let score = entry.score.to_string();
        let score_x = 272usize.saturating_sub(font.text_width(&score));
        font.draw_text(pixels, WIDTH, HEIGHT, score_x, y, &score, color);
    }
    font.draw_text(
        pixels,
        WIDTH,
        HEIGHT,
        32,
        184,
        if editing_rank.is_some() {
            "TYPE NAME, THEN ENTER"
        } else {
            "PRESS ANY KEY"
        },
        0xFFFFFFFF,
    );
}

pub fn render_death_view(pixels: &mut [u32]) {
    fill_view(pixels, 4);
}

pub(crate) fn render_empty_view(pixels: &mut [u32]) {
    fill_view(pixels, 0);
}

fn fill_view(pixels: &mut [u32], palette_index: u8) {
    let view = Viewport::current();
    let color = palette_color(palette_index, 1.0);
    for y in view.top..view.bottom {
        pixels[y * WIDTH + view.left..y * WIDTH + view.right].fill(color);
    }
}

/// Applies the VGA palette ramps used for player damage and accepted pickups.
/// Red takes precedence over white, while both timers continue to elapse.
pub fn render_palette_shift(pixels: &mut [u32], damage_tics: u32, bonus_tics: u32) {
    apply_palette_shift(pixels, WIDTH * HEIGHT, damage_tics, bonus_tics);
}

pub fn render_palette_shift_scaled(
    pixels: &mut [u32],
    scale: usize,
    damage_tics: u32,
    bonus_tics: u32,
) {
    apply_palette_shift(
        pixels,
        WIDTH * HEIGHT * scale * scale,
        damage_tics,
        bonus_tics,
    );
}

fn apply_palette_shift(pixels: &mut [u32], pixel_count: usize, damage_tics: u32, bonus_tics: u32) {
    let red = if damage_tics == 0 {
        0
    } else {
        (damage_tics / 10 + 1).min(6)
    };
    let white = if bonus_tics == 0 {
        0
    } else {
        (bonus_tics / 6 + 1).min(3)
    };
    let (target, numerator, denominator) = if red != 0 {
        ([64i32, 0, 0], red as i32, 8)
    } else if white != 0 {
        ([64i32, 62, 0], white as i32, 20)
    } else {
        return;
    };
    for pixel in pixels.iter_mut().take(pixel_count) {
        let channels = [
            ((*pixel >> 16) & 0xff) as i32,
            ((*pixel >> 8) & 0xff) as i32,
            (*pixel & 0xff) as i32,
        ];
        let shift = |channel: i32, target: i32| {
            // Recover the six-bit DAC value before applying DOS integer rounding.
            let dac = (channel * 63 + 127) / 255;
            ((dac + (target - dac) * numerator / denominator).clamp(0, 63) * 255 / 63) as u32
        };
        let shifted = [
            shift(channels[0], target[0]),
            shift(channels[1], target[1]),
            shift(channels[2], target[2]),
        ];
        *pixel = 0xff00_0000 | (shifted[0] << 16) | (shifted[1] << 8) | shifted[2];
    }
}

pub fn render_boss_death_interstitial(pixels: &mut [u32], font: Option<&BitmapFont>) {
    if pixels.len() < WIDTH * HEIGHT {
        return;
    }
    pixels[..WIDTH * (HEIGHT - 40)].fill(palette_color(127, 1.0));
    if let Some(font) = font {
        font.draw_text(
            pixels,
            WIDTH,
            HEIGHT,
            0,
            7 * 8,
            "LET'S SEE THAT AGAIN!",
            0xFFFFFFFF,
        );
    }
}

pub fn render_ending_page(
    pixels: &mut [u32],
    font: &BitmapFont,
    frame: &[Picture; 4],
    page: &EndingPage,
    page_number: usize,
    page_count: usize,
    elapsed_tics: f64,
) {
    if pixels.len() < WIDTH * HEIGHT {
        return;
    }
    pixels[..WIDTH * HEIGHT].fill(palette_color(0x11, 1.0));
    draw_picture(&frame[0], 0, 0, pixels);
    draw_picture(&frame[1], 0, 8, pixels);
    draw_picture(&frame[2], WIDTH.saturating_sub(frame[2].width), 8, pixels);
    draw_picture(&frame[3], 8, HEIGHT.saturating_sub(frame[3].height), pixels);
    for op in page.ops() {
        match op {
            DrawOp::Text {
                x,
                y,
                text,
                color,
                show_after_tics,
            } if elapsed_tics >= *show_after_tics as f64 => font.draw_text(
                pixels,
                WIDTH,
                HEIGHT,
                *x,
                *y,
                text,
                palette_color(*color, 1.0),
            ),
            DrawOp::Text { .. } => {}
            DrawOp::Picture {
                x,
                y,
                picture,
                show_after_tics,
            } if elapsed_tics >= *show_after_tics as f64 => draw_picture(picture, *x, *y, pixels),
            DrawOp::Picture { .. } => {}
            DrawOp::Bar {
                x,
                y,
                width,
                height,
                color,
                show_after_tics,
            } if elapsed_tics >= *show_after_tics as f64 => {
                let color = palette_color(*color, 1.0);
                let x_end = x.saturating_add(*width).min(WIDTH);
                let y_end = y.saturating_add(*height).min(HEIGHT);
                for row in (*y).min(HEIGHT)..y_end {
                    pixels[row * WIDTH + (*x).min(WIDTH)..row * WIDTH + x_end].fill(color);
                }
            }
            DrawOp::Bar { .. } => {}
        }
    }
    if elapsed_tics >= page.layout_delay_tics() as f64 {
        font.draw_text(
            pixels,
            WIDTH,
            HEIGHT,
            213,
            183,
            &format!("pg {page_number} of {page_count}"),
            palette_color(0x4f, 1.0),
        );
    }
}

pub fn render_title_screen(pixels: &mut [u32], title: &Picture) {
    if pixels.len() < WIDTH * HEIGHT {
        return;
    }
    pixels[..WIDTH * HEIGHT].fill(palette_color(0, 1.0));
    draw_picture(title, 0, 0, pixels);
}

pub fn render_preload_screen(
    pixels: &mut [u32],
    picture: &Picture,
    status_bar: &Picture,
    progress: usize,
) {
    if pixels.len() < WIDTH * HEIGHT {
        return;
    }
    pixels[..WIDTH * HEIGHT].fill(palette_color(0, 1.0));
    draw_filled_rect(pixels, 0, 0, WIDTH, HEIGHT - 40, palette_color(127, 1.0));
    // LatchDrawPic's x coordinate is measured in 8-pixel character cells;
    // DOS passes 20 - 14 here, which places the 224-pixel panel at x = 48.
    draw_picture(picture, 6 * 8, 56, pixels);
    draw_picture(status_bar, 0, HEIGHT - 40, pixels);

    let bar_x = 53;
    let bar_y = 101;
    let bar_width = 214;
    let filled = bar_width * progress.min(1000) / 1000;
    draw_filled_rect(pixels, bar_x, bar_y, bar_width, 2, palette_color(0, 1.0));
    if filled > 0 {
        draw_filled_rect(pixels, bar_x, bar_y, filled, 2, palette_color(0x37, 1.0));
        draw_filled_rect(
            pixels,
            bar_x,
            bar_y,
            filled.saturating_sub(1),
            1,
            palette_color(0x32, 1.0),
        );
    }
}

pub fn render_pause_menu(
    pixels: &mut [u32],
    font: &BitmapFont,
    pictures: Option<&PauseSoundMenuPictures>,
    selected: usize,
    confirmation_prompt: Option<&str>,
    confirm_selection: usize,
) {
    if pixels.len() < WIDTH * HEIGHT {
        return;
    }
    let picture_layout = pictures.is_some();
    if let Some(pictures) = pictures {
        pixels[..WIDTH * HEIGHT].fill(palette_color(0x29, 1.0));
        draw_filled_rect(pixels, 0, 10, WIDTH, 24, palette_color(0, 1.0));
        for x in 0..WIDTH {
            pixels[32 * WIDTH + x] = palette_color(0x2c, 1.0);
        }
        draw_picture(&pictures.mouse_background, 112, 184, pixels);
        draw_picture(&pictures.options_panel, 84, 0, pixels);
        draw_filled_rect(pixels, 68, 34, 178, 126, palette_color(0x2d, 1.0));
        draw_menu_border(pixels, 68, 34, 178, 126);
    } else {
        draw_filled_rect(pixels, 48, 22, 224, 156, palette_color(0x2d, 1.0));
        draw_menu_border(pixels, 48, 22, 224, 156);
    }
    if let Some(prompt) = confirmation_prompt {
        font.draw_text(
            pixels,
            WIDTH,
            HEIGHT,
            112,
            82,
            prompt,
            palette_color(0x4f, 1.0),
        );
        for (index, label) in ["YES", "NO"].into_iter().enumerate() {
            let color = if index == confirm_selection {
                palette_color(0x13, 1.0)
            } else {
                palette_color(0x17, 1.0)
            };
            font.draw_text(
                pixels,
                WIDTH,
                HEIGHT,
                120,
                102 + index * 14,
                &format!(
                    "{} {label}",
                    if index == confirm_selection { ">" } else { " " }
                ),
                color,
            );
        }
    } else {
        font.draw_text(
            pixels,
            WIDTH,
            HEIGHT,
            136,
            if picture_layout { 34 } else { 28 },
            "PAUSED",
            palette_color(0x4f, 1.0),
        );
        for (index, label) in [
            "RESUME",
            "SAVE GAME",
            "LOAD GAME",
            "SOUND",
            "CONTROLS",
            "DISPLAY",
            "VIEW SCORES",
            "NEW GAME",
            "END GAME",
        ]
        .into_iter()
        .enumerate()
        {
            let color = if index == selected {
                palette_color(0x13, 1.0)
            } else {
                palette_color(0x17, 1.0)
            };
            font.draw_text(
                pixels,
                WIDTH,
                HEIGHT,
                112,
                if picture_layout {
                    47 + index * 10
                } else {
                    39 + index * 11
                },
                &format!("{} {label}", if index == selected { ">" } else { " " }),
                color,
            );
        }
    }
}

pub(crate) fn render_pause_load_confirmation(
    pixels: &mut [u32],
    font: &BitmapFont,
    slot: usize,
    selected: usize,
) {
    if pixels.len() < WIDTH * HEIGHT {
        return;
    }
    draw_filled_rect(pixels, 42, 66, 236, 68, palette_color(0x2d, 1.0));
    draw_menu_border(pixels, 42, 66, 236, 68);
    font.draw_text(
        pixels,
        WIDTH,
        HEIGHT,
        96,
        78,
        &format!("LOAD SLOT {}?", slot + 1),
        palette_color(0x4f, 1.0),
    );
    for (index, label) in ["YES", "NO"].into_iter().enumerate() {
        font.draw_text(
            pixels,
            WIDTH,
            HEIGHT,
            112,
            102 + index * 14,
            &format!("{} {label}", if index == selected { ">" } else { " " }),
            palette_color(if index == selected { 0x13 } else { 0x17 }, 1.0),
        );
    }
}

pub(crate) fn render_quick_end_confirmation(
    pixels: &mut [u32],
    font: &BitmapFont,
    selected: usize,
) {
    if pixels.len() < WIDTH * HEIGHT {
        return;
    }
    draw_filled_rect(pixels, 48, 44, 224, 72, palette_color(0x2d, 1.0));
    draw_menu_border(pixels, 48, 44, 224, 72);
    font.draw_text(
        pixels,
        WIDTH,
        HEIGHT,
        76,
        52,
        "ARE YOU SURE YOU WANT",
        palette_color(0x4f, 1.0),
    );
    font.draw_text(
        pixels,
        WIDTH,
        HEIGHT,
        84,
        63,
        "TO END THE GAME YOU",
        palette_color(0x4f, 1.0),
    );
    font.draw_text(
        pixels,
        WIDTH,
        HEIGHT,
        128,
        74,
        "ARE PLAYING?",
        palette_color(0x4f, 1.0),
    );
    for (index, label) in ["YES", "NO"].into_iter().enumerate() {
        font.draw_text(
            pixels,
            WIDTH,
            HEIGHT,
            112 + index * 72,
            98,
            &format!("{} {label}", if index == selected { ">" } else { " " }),
            palette_color(if index == selected { 0x13 } else { 0x17 }, 1.0),
        );
    }
}

pub(crate) fn render_quick_quit_confirmation(
    pixels: &mut [u32],
    font: &BitmapFont,
    prompt: &str,
    selected: usize,
) {
    if pixels.len() < WIDTH * HEIGHT {
        return;
    }
    draw_filled_rect(pixels, 16, 38, 288, 88, palette_color(0x2d, 1.0));
    draw_menu_border(pixels, 16, 38, 288, 88);
    font.draw_text(
        pixels,
        WIDTH,
        HEIGHT,
        24,
        47,
        prompt,
        palette_color(0x4f, 1.0),
    );
    for (index, label) in ["YES", "NO"].into_iter().enumerate() {
        font.draw_text(
            pixels,
            WIDTH,
            HEIGHT,
            104 + index * 72,
            102,
            &format!("{} {label}", if index == selected { ">" } else { " " }),
            palette_color(if index == selected { 0x13 } else { 0x17 }, 1.0),
        );
    }
}

pub fn render_pause_sound_menu(
    pixels: &mut [u32],
    font: &BitmapFont,
    pictures: Option<&PauseSoundMenuPictures>,
    selected: usize,
) {
    if pixels.len() < WIDTH * HEIGHT {
        return;
    }
    if let Some(pictures) = pictures {
        for (x, y, width, height) in [(40, 17, 250, 45), (40, 82, 250, 45), (40, 147, 250, 32)] {
            draw_filled_rect(pixels, x, y, width, height, palette_color(0x2d, 1.0));
            draw_menu_border(pixels, x, y, width, height);
        }
        draw_picture(&pictures.mouse_background, 112, 184, pixels);
        draw_picture(&pictures.effects_title, 100, 0, pixels);
        draw_picture(&pictures.digitized_title, 100, 65, pixels);
        draw_picture(&pictures.music_title, 100, 130, pixels);

        let effects_mode = crate::settings::sound_effects_mode();
        let rows = [
            (
                0,
                "NONE",
                effects_mode == crate::settings::SoundEffectsMode::Off,
                20,
            ),
            (
                1,
                "PC",
                effects_mode == crate::settings::SoundEffectsMode::Pc,
                33,
            ),
            (
                2,
                "ADLIB/SB",
                effects_mode == crate::settings::SoundEffectsMode::Adlib,
                46,
            ),
            (3, "OFF", !crate::settings::digitized_effects_enabled(), 85),
            (4, "ON", crate::settings::digitized_effects_enabled(), 98),
        ];
        for (index, label, active, y) in rows {
            draw_picture(
                if active {
                    &pictures.selected
                } else {
                    &pictures.not_selected
                },
                72,
                y + 2,
                pixels,
            );
            let color = if index == selected {
                palette_color(0x4f, 1.0)
            } else {
                palette_color(0x17, 1.0)
            };
            font.draw_text(pixels, WIDTH, HEIGHT, 100, y, label, color);
            if index == selected {
                draw_picture(&pictures.cursor, 48, y.saturating_sub(2), pixels);
            }
        }
        for (index, y, label) in [(5, 150, "NONE"), (6, 163, "ADLIB/SB")] {
            let music_mode_selected = crate::settings::music_enabled() == (index == 6);
            draw_picture(
                if music_mode_selected {
                    &pictures.selected
                } else {
                    &pictures.not_selected
                },
                72,
                y + 2,
                pixels,
            );
            if index == selected {
                draw_picture(&pictures.cursor, 48, y.saturating_sub(2), pixels);
            }
            font.draw_text(
                pixels,
                WIDTH,
                HEIGHT,
                100,
                y,
                label,
                palette_color(if index == selected { 0x4f } else { 0x17 }, 1.0),
            );
        }
        let back_y = 176;
        font.draw_text(
            pixels,
            WIDTH,
            HEIGHT,
            100,
            back_y,
            "BACK",
            if selected == 7 {
                palette_color(0x4f, 1.0)
            } else {
                palette_color(0x17, 1.0)
            },
        );
        if selected == 7 {
            draw_picture(&pictures.cursor, 48, back_y - 2, pixels);
        }
        return;
    }
    draw_filled_rect(pixels, 32, 48, 256, 112, palette_color(0x2d, 1.0));
    draw_menu_border(pixels, 32, 48, 256, 112);
    font.draw_text(
        pixels,
        WIDTH,
        HEIGHT,
        118,
        57,
        "SOUND SETTINGS",
        palette_color(0x4f, 1.0),
    );
    let options = [
        "SOUND FX: NONE".to_owned(),
        "SOUND FX: PC".to_owned(),
        "SOUND FX: ADLIB/SB".to_owned(),
        "DIGITIZED: OFF".to_owned(),
        "DIGITIZED: ON".to_owned(),
        "MUSIC: NONE".to_owned(),
        "MUSIC: ADLIB/SB".to_owned(),
        "BACK".to_owned(),
    ];
    for (index, label) in options.iter().enumerate() {
        let color = if index == selected {
            palette_color(0x13, 1.0)
        } else {
            palette_color(0x17, 1.0)
        };
        font.draw_text(
            pixels,
            WIDTH,
            HEIGHT,
            64,
            62 + index * 13,
            &format!("{} {label}", if index == selected { ">" } else { " " }),
            color,
        );
    }
}

pub fn render_pause_slot_menu(
    pixels: &mut [u32],
    font: &BitmapFont,
    pictures: Option<&PauseSoundMenuPictures>,
    loading: bool,
    selected: usize,
    slots: &[String],
) {
    if pixels.len() < WIDTH * HEIGHT {
        return;
    }
    if let Some(pictures) = pictures {
        pixels[..WIDTH * HEIGHT].fill(palette_color(0x29, 1.0));
        draw_picture(&pictures.mouse_background, 112, 184, pixels);
        draw_filled_rect(pixels, 75, 50, 175, 140, palette_color(0x2d, 1.0));
        draw_menu_border(pixels, 75, 50, 175, 140);
        draw_filled_rect(pixels, 0, 10, WIDTH, 24, palette_color(0, 1.0));
        for x in 0..WIDTH {
            pixels[32 * WIDTH + x] = palette_color(0x2c, 1.0);
        }
        draw_picture(
            if loading {
                &pictures.load_game
            } else {
                &pictures.save_game
            },
            60,
            0,
            pixels,
        );
    } else {
        draw_filled_rect(pixels, 75, 50, 175, 140, palette_color(0x2d, 1.0));
        draw_menu_border(pixels, 75, 50, 175, 140);
        font.draw_text(
            pixels,
            WIDTH,
            HEIGHT,
            if loading { 111 } else { 105 },
            39,
            if loading { "LOAD GAME" } else { "SAVE GAME" },
            palette_color(0x4f, 1.0),
        );
    }
    if let Some(pictures) = pictures {
        for (index, label) in slots
            .iter()
            .take(crate::resources::SAVE_SLOT_COUNT)
            .enumerate()
        {
            draw_pause_slot_source_option(
                pixels,
                font,
                pictures,
                index,
                selected,
                label,
                55 + index * 13,
                true,
            );
        }
    } else {
        for (index, label) in slots
            .iter()
            .take(crate::resources::SAVE_SLOT_COUNT)
            .enumerate()
        {
            draw_pause_slot_option(pixels, font, index, selected, label, 91, 56 + index * 13);
        }
    }
}

pub fn render_pause_io_progress_overlay(
    pixels: &mut [u32],
    font: &BitmapFont,
    pictures: &PauseSoundMenuPictures,
    loading: bool,
    animation_frame: u8,
) {
    if pixels.len() < WIDTH * HEIGHT {
        return;
    }
    draw_filled_rect(pixels, 96, 80, 130, 42, palette_color(0x17, 1.0));
    draw_outline_rect(pixels, 96, 80, 130, 42, palette_color(0, 1.0));
    draw_outline_rect(pixels, 97, 81, 128, 40, palette_color(0x13, 1.0));
    draw_picture(
        if animation_frame & 1 == 0 {
            &pictures.disk_loading1
        } else {
            &pictures.disk_loading2
        },
        104,
        85,
        pixels,
    );
    font.draw_text(
        pixels,
        WIDTH,
        HEIGHT,
        142,
        93,
        if loading { "LOADING..." } else { "SAVING..." },
        palette_color(0, 1.0),
    );
}

pub fn render_pause_controls_menu(
    pixels: &mut [u32],
    font: &BitmapFont,
    pictures: Option<&PauseSoundMenuPictures>,
    options: &[String],
    selected: usize,
    capture_prompt: Option<&str>,
) {
    if pixels.len() < WIDTH * HEIGHT {
        return;
    }
    if let Some(pictures) = pictures {
        pixels[..WIDTH * HEIGHT].fill(palette_color(0x29, 1.0));
        draw_filled_rect(pixels, 0, 10, WIDTH, 24, palette_color(0, 1.0));
        for x in 0..WIDTH {
            pixels[32 * WIDTH + x] = palette_color(0x2c, 1.0);
        }
        draw_picture(&pictures.customize_panel, 80, 0, pixels);
        draw_picture(&pictures.mouse_background, 112, 184, pixels);
        draw_filled_rect(pixels, 4, 45, 312, 150, palette_color(0x2d, 1.0));
        draw_menu_border(pixels, 4, 45, 312, 150);
    } else {
        draw_filled_rect(pixels, 4, 20, 312, 160, palette_color(0x2d, 1.0));
        draw_menu_border(pixels, 4, 20, 312, 160);
    }
    let heading = capture_prompt.unwrap_or("CONTROL SETTINGS");
    let heading_x = WIDTH.saturating_sub(font.text_width(heading)) / 2;
    font.draw_text(
        pixels,
        WIDTH,
        HEIGHT,
        heading_x,
        if pictures.is_some() {
            if options.len() >= 20 { 28 } else { 35 }
        } else {
            6
        },
        heading,
        palette_color(0x4f, 1.0),
    );
    for (index, option) in options.iter().enumerate() {
        let y = if pictures.is_some() {
            (if options.len() >= 20 {
                40
            } else if options.len() > 17 {
                44
            } else {
                50
            }) + index * 8
        } else {
            24 + index * 10
        };
        if index == selected {
            font.draw_text(pixels, WIDTH, HEIGHT, 12, y, ">", palette_color(0x13, 1.0));
        }
        font.draw_text(
            pixels,
            WIDTH,
            HEIGHT,
            22,
            y,
            option,
            palette_color(if index == selected { 0x13 } else { 0x17 }, 1.0),
        );
    }
}

pub fn render_pause_display_menu(
    pixels: &mut [u32],
    font: &BitmapFont,
    options: &[String],
    selected: usize,
) {
    if pixels.len() < WIDTH * HEIGHT {
        return;
    }
    draw_filled_rect(pixels, 32, 28, 256, 140, palette_color(0x2d, 1.0));
    draw_menu_border(pixels, 32, 28, 256, 140);
    font.draw_text(
        pixels,
        WIDTH,
        HEIGHT,
        112,
        38,
        "DISPLAY SETTINGS",
        palette_color(0x4f, 1.0),
    );
    for (index, option) in options.iter().enumerate() {
        let color = if index == selected {
            palette_color(0x13, 1.0)
        } else {
            palette_color(0x17, 1.0)
        };
        font.draw_text(
            pixels,
            WIDTH,
            HEIGHT,
            56,
            62 + index * 12,
            &format!("{} {option}", if index == selected { ">" } else { " " }),
            color,
        );
    }
}

pub fn render_pause_view_size_menu(pixels: &mut [u32], font: &BitmapFont) {
    if pixels.len() < WIDTH * HEIGHT {
        return;
    }
    // CP_ChangeView's DOS panel replaces the bottom 40 VGA scanlines with a
    // solid VIEWCOLOR bar and three centered instruction lines. The active
    // viewport above it already reflects the temporary view-size selection.
    draw_filled_rect(pixels, 0, 160, WIDTH, 40, palette_color(0x7f, 1.0));
    let lines = [
        "ARROWS OR MOUSE TO SIZE",
        "ENTER OR LEFT CLICK TO ACCEPT",
        "ESC OR RIGHT CLICK TO CANCEL",
    ];
    for (line, text) in lines.iter().enumerate() {
        let x = WIDTH.saturating_sub(font.text_width(text)) / 2;
        font.draw_text(
            pixels,
            WIDTH,
            HEIGHT,
            x,
            161 + line * font.height(),
            text,
            palette_color(0x13, 1.0),
        );
    }
}

fn draw_pause_slot_option(
    pixels: &mut [u32],
    font: &BitmapFont,
    index: usize,
    selected: usize,
    label: &str,
    x: usize,
    y: usize,
) {
    let color = if index == selected {
        palette_color(0x13, 1.0)
    } else {
        palette_color(0x17, 1.0)
    };
    font.draw_text(
        pixels,
        WIDTH,
        HEIGHT,
        x,
        y,
        &format!("{} {label}", if index == selected { ">" } else { " " }),
        color,
    );
}

fn draw_pause_slot_source_option(
    pixels: &mut [u32],
    font: &BitmapFont,
    pictures: &PauseSoundMenuPictures,
    index: usize,
    selected: usize,
    label: &str,
    row_y: usize,
    outlined: bool,
) {
    if outlined {
        draw_outline_rect(pixels, 109, row_y, 136, 11, palette_color(0x17, 1.0));
    }
    if index == selected {
        draw_picture(&pictures.cursor, 85, row_y.saturating_sub(2), pixels);
    }
    font.draw_text(
        pixels,
        WIDTH,
        HEIGHT,
        111,
        row_y + 1,
        label,
        palette_color(if index == selected { 0x13 } else { 0x17 }, 1.0),
    );
}

fn on_off(enabled: bool) -> &'static str {
    if enabled { "ON" } else { "OFF" }
}

pub fn render_pg13_screen(pixels: &mut [u32], picture: &Picture, brightness: f64) {
    if pixels.len() < WIDTH * HEIGHT {
        return;
    }
    pixels[..WIDTH * HEIGHT].fill(palette_color(0x82, brightness));
    for source_y in 0..picture.height {
        for source_x in 0..picture.width {
            let x = 216 + source_x;
            let y = 110 + source_y;
            if x < WIDTH && y < HEIGHT {
                pixels[y * WIDTH + x] = palette_color(
                    picture.pixels[source_y * picture.width + source_x],
                    brightness,
                );
            }
        }
    }
}

pub fn render_signon_screen(pixels: &mut [u32], image: &[u8], font: &BitmapFont, message: &str) {
    if pixels.len() < WIDTH * HEIGHT || image.len() < WIDTH * HEIGHT {
        return;
    }
    for (target, &source) in pixels[..WIDTH * HEIGHT]
        .iter_mut()
        .zip(&image[..WIDTH * HEIGHT])
    {
        *target = palette_color(source, 1.0);
    }

    draw_filled_rect(pixels, 0, 189, 300, 11, palette_color(image[0], 1.0));
    let x = WIDTH.saturating_sub(font.text_width(message)) / 2;
    let color = if message == "Press a key" { 14 } else { 10 };
    font.draw_text(
        pixels,
        WIDTH,
        HEIGHT,
        x,
        190,
        message,
        palette_color(color, 1.0),
    );
}

pub fn render_choice_menu(
    pixels: &mut [u32],
    font: &BitmapFont,
    heading: &str,
    options: &[String],
    layout: ChoiceMenuLayout,
    background: &Picture,
    illustrations: &[Picture],
    selected: usize,
    cursor_frame: usize,
) {
    if pixels.len() < WIDTH * HEIGHT {
        return;
    }
    pixels[..WIDTH * HEIGHT].fill(palette_color(0x2d, 1.0));
    match layout {
        ChoiceMenuLayout::Main | ChoiceMenuLayout::StartupMain => {
            // Match DOS DrawMainMenu: title strip, OPTIONS artwork, and the
            // 10-item menu window. The modern menu exposes fewer entries, so
            // keep the original window bounds while rendering only available
            // actions.
            draw_picture(background, 112, 184, pixels);
            draw_filled_rect(pixels, 0, 10, WIDTH, 23, palette_color(0x00, 1.0));
            draw_filled_rect(pixels, 0, 32, WIDTH, 1, palette_color(0x2c, 1.0));
            if let Some(picture) = illustrations.first() {
                draw_picture(picture, 84, 0, pixels);
            } else {
                let heading_x = WIDTH.saturating_sub(font.text_width(heading)) / 2;
                font.draw_text(
                    pixels,
                    WIDTH,
                    HEIGHT,
                    heading_x,
                    18,
                    heading,
                    palette_color(0x47, 1.0),
                );
            }
            draw_filled_rect(pixels, 68, 52, 178, 136, palette_color(0x2d, 1.0));
            draw_menu_border(pixels, 68, 52, 178, 136);
            for (index, option) in options.iter().enumerate() {
                let y = 55 + index * 13;
                if index == selected {
                    if let Some(cursor) = illustrations.get(1 + cursor_frame.min(1)) {
                        // WL_MENU.C's HandleMenu aligns the cursor to
                        // iteminfo.x & -8 (76 rounded down to 72).
                        draw_picture(cursor, 72, y.saturating_sub(2), pixels);
                    } else {
                        font.draw_text(pixels, WIDTH, HEIGHT, 72, y, ">", palette_color(0x13, 1.0));
                    }
                }
                font.draw_text(
                    pixels,
                    WIDTH,
                    HEIGHT,
                    100,
                    y,
                    option,
                    palette_color(if index == selected { 0x13 } else { 0x17 }, 1.0),
                );
            }
        }
        ChoiceMenuLayout::Controls => {
            draw_filled_rect(pixels, 6, 25, 308, 151, palette_color(0x2d, 1.0));
            draw_menu_border(pixels, 6, 25, 308, 151);
            let heading_x = WIDTH.saturating_sub(font.text_width(heading)) / 2;
            font.draw_text(
                pixels,
                WIDTH,
                HEIGHT,
                heading_x,
                6,
                heading,
                palette_color(0x47, 1.0),
            );
            for (index, option) in options.iter().enumerate() {
                let y = 29 + index * 10;
                if index == selected {
                    font.draw_text(pixels, WIDTH, HEIGHT, 14, y, ">", palette_color(0x13, 1.0));
                }
                font.draw_text(
                    pixels,
                    WIDTH,
                    HEIGHT,
                    24,
                    y,
                    option,
                    palette_color(if index == selected { 0x13 } else { 0x17 }, 1.0),
                );
            }
        }
        ChoiceMenuLayout::StartupControls => {
            pixels[..WIDTH * HEIGHT].fill(palette_color(0x29, 1.0));
            draw_filled_rect(pixels, 0, 10, WIDTH, 24, palette_color(0, 1.0));
            draw_filled_rect(pixels, 0, 32, WIDTH, 1, palette_color(0x2c, 1.0));
            if let Some(picture) = illustrations.first() {
                draw_picture(picture, 80, 0, pixels);
            }
            if let Some(picture) = illustrations.get(1) {
                draw_picture(picture, 112, 184, pixels);
            }
            draw_filled_rect(pixels, 4, 45, 312, 150, palette_color(0x2d, 1.0));
            draw_menu_border(pixels, 4, 45, 312, 150);
            let compact = options.len() >= 20;
            let heading_x = WIDTH.saturating_sub(font.text_width(heading)) / 2;
            font.draw_text(
                pixels,
                WIDTH,
                HEIGHT,
                heading_x,
                if compact { 28 } else { 35 },
                heading,
                palette_color(0x4f, 1.0),
            );
            for (index, option) in options.iter().enumerate() {
                let y = (if compact { 40 } else { 50 }) + index * 8;
                if index == selected {
                    font.draw_text(pixels, WIDTH, HEIGHT, 12, y, ">", palette_color(0x13, 1.0));
                }
                font.draw_text(
                    pixels,
                    WIDTH,
                    HEIGHT,
                    22,
                    y,
                    option,
                    palette_color(if index == selected { 0x13 } else { 0x17 }, 1.0),
                );
            }
        }
        ChoiceMenuLayout::StartupControlPanel => {
            pixels[..WIDTH * HEIGHT].fill(palette_color(0x29, 1.0));
            draw_filled_rect(pixels, 0, 10, WIDTH, 24, palette_color(0, 1.0));
            draw_filled_rect(pixels, 0, 32, WIDTH, 1, palette_color(0x2c, 1.0));
            if let Some(picture) = illustrations.first() {
                draw_picture(picture, 80, 0, pixels);
            }
            if let Some(picture) = illustrations.get(1) {
                draw_picture(picture, 112, 184, pixels);
            }
            draw_filled_rect(pixels, 16, 65, 284, 84, palette_color(0x2d, 1.0));
            draw_menu_border(pixels, 16, 65, 284, 84);
            for (index, option) in options.iter().enumerate() {
                let y = 70 + index * 13;
                if index < 2 {
                    let checked = option.ends_with(": On");
                    if let Some(checkbox) = illustrations.get(if checked { 5 } else { 4 }) {
                        draw_picture(checkbox, 56, y + 3, pixels);
                    }
                }
                font.draw_text(
                    pixels,
                    WIDTH,
                    HEIGHT,
                    80,
                    y,
                    option,
                    palette_color(0x17, 1.0),
                );
            }
            let cursor = illustrations.get(2 + cursor_frame.min(1));
            if let Some(cursor) = cursor {
                draw_picture(cursor, 24, 68 + selected * 13, pixels);
            }
        }
        ChoiceMenuLayout::StartupSound => {
            render_original_sound_settings(pixels, font, options, illustrations, selected);
        }
        ChoiceMenuLayout::MouseSensitivity => {
            pixels[..WIDTH * HEIGHT].fill(palette_color(0x29, 1.0));
            if let Some(picture) = illustrations.first() {
                draw_picture(picture, 112, 184, pixels);
            }
            draw_filled_rect(pixels, 10, 80, 300, 30, palette_color(0x2d, 1.0));
            draw_menu_border(pixels, 10, 80, 300, 30);
            let heading_x = WIDTH.saturating_sub(font.text_width(heading)) / 2;
            font.draw_text(
                pixels,
                WIDTH,
                HEIGHT,
                heading_x,
                82,
                heading,
                palette_color(0x4a, 1.0),
            );
            font.draw_text(
                pixels,
                WIDTH,
                HEIGHT,
                14,
                95,
                "SLOW",
                palette_color(0x17, 1.0),
            );
            font.draw_text(
                pixels,
                WIDTH,
                HEIGHT,
                269,
                95,
                "FAST",
                palette_color(0x17, 1.0),
            );
            draw_filled_rect(pixels, 60, 97, 200, 10, palette_color(0x17, 1.0));
            draw_outline_rect(pixels, 60, 97, 200, 10, palette_color(0x13, 1.0));
            let cursor_x = 60 + selected.min(9) * 20;
            draw_outline_rect(pixels, cursor_x, 97, 20, 10, palette_color(0x4a, 1.0));
            draw_filled_rect(pixels, cursor_x + 1, 98, 18, 8, palette_color(0x47, 1.0));
        }
        ChoiceMenuLayout::Episode => {
            // DrawNewEpisode restores the shared mouse-button artwork after
            // clearing the menu screen and before placing its window.
            draw_picture(background, 112, 184, pixels);
            draw_filled_rect(pixels, 6, 19, 308, 162, palette_color(0x2d, 1.0));
            draw_menu_border(pixels, 6, 19, 308, 162);
            for (index, option) in options.iter().enumerate() {
                let x = 42;
                let y = 23 + index * 26;
                if let Some(picture) = illustrations.get(index) {
                    draw_picture(picture, x, y, pixels);
                }
                if index == selected {
                    if let Some(cursor) = illustrations.get(options.len() + cursor_frame.min(1)) {
                        draw_picture(cursor, 8, y.saturating_sub(2), pixels);
                    } else {
                        font.draw_text(pixels, WIDTH, HEIGHT, 8, y, ">", palette_color(0x13, 1.0));
                    }
                }
                font.draw_text(
                    pixels,
                    WIDTH,
                    HEIGHT,
                    98,
                    y,
                    option,
                    palette_color(if index == selected { 0x13 } else { 0x17 }, 1.0),
                );
            }
            let heading_x = WIDTH.saturating_sub(font.text_width(heading)) / 2;
            font.draw_text(
                pixels,
                WIDTH,
                HEIGHT,
                heading_x,
                2,
                heading,
                palette_color(0x47, 1.0),
            );
        }
        ChoiceMenuLayout::Difficulty => {
            // DrawNewGame uses the same C_MOUSELBACKPIC at the DOS position.
            draw_picture(background, 112, 184, pixels);
            draw_filled_rect(pixels, 45, 90, 225, 67, palette_color(0x2d, 1.0));
            draw_menu_border(pixels, 45, 90, 225, 67);
            font.draw_text(
                pixels,
                WIDTH,
                HEIGHT,
                70,
                68,
                heading,
                palette_color(0x47, 1.0),
            );
            for (index, option) in options.iter().enumerate() {
                if index == selected {
                    if let Some(cursor) = illustrations.get(options.len() + cursor_frame.min(1)) {
                        draw_picture(cursor, 48, 98 + index * 13, pixels);
                    } else {
                        font.draw_text(
                            pixels,
                            WIDTH,
                            HEIGHT,
                            48,
                            100 + index * 13,
                            ">",
                            palette_color(0x13, 1.0),
                        );
                    }
                }
                font.draw_text(
                    pixels,
                    WIDTH,
                    HEIGHT,
                    70,
                    100 + index * 13,
                    option,
                    palette_color(if index == selected { 0x13 } else { 0x17 }, 1.0),
                );
            }
            if let Some(picture) = illustrations.get(selected) {
                draw_picture(picture, 235, 107, pixels);
            }
        }
        ChoiceMenuLayout::Jukebox => {
            draw_filled_rect(pixels, 20, 42, 280, 112, palette_color(0x2d, 1.0));
            draw_menu_border(pixels, 20, 42, 280, 112);
            let heading_x = WIDTH.saturating_sub(font.text_width(heading)) / 2;
            font.draw_text(
                pixels,
                WIDTH,
                HEIGHT,
                heading_x,
                19,
                heading,
                palette_color(0x47, 1.0),
            );
            let page_start = selected / 6 * 6;
            for (row, (index, option)) in options
                .iter()
                .enumerate()
                .skip(page_start)
                .take(6)
                .enumerate()
            {
                let y = 48 + row * 13;
                if index == selected {
                    font.draw_text(pixels, WIDTH, HEIGHT, 31, y, ">", palette_color(0x13, 1.0));
                }
                font.draw_text(
                    pixels,
                    WIDTH,
                    HEIGHT,
                    43,
                    y,
                    option,
                    palette_color(if index == selected { 0x13 } else { 0x17 }, 1.0),
                );
            }
            let page_count = options.len().div_ceil(6);
            let page_label = format!("PAGE {}/{}", page_start / 6 + 1, page_count);
            let page_x = WIDTH.saturating_sub(font.text_width(&page_label)) / 2;
            font.draw_text(
                pixels,
                WIDTH,
                HEIGHT,
                page_x,
                163,
                &page_label,
                palette_color(0x17, 1.0),
            );
        }
    }
    draw_picture(background, 112, 184, pixels);
}

fn render_original_sound_settings(
    pixels: &mut [u32],
    font: &BitmapFont,
    options: &[String],
    illustrations: &[Picture],
    selected: usize,
) {
    pixels[..WIDTH * HEIGHT].fill(palette_color(0x29, 1.0));
    for (x, y, width, height) in [(40, 17, 250, 45), (40, 82, 250, 45), (40, 147, 250, 32)] {
        draw_filled_rect(pixels, x, y, width, height, palette_color(0x2d, 1.0));
        draw_menu_border(pixels, x, y, width, height);
    }
    if let Some(picture) = illustrations.first() {
        draw_picture(picture, 112, 184, pixels);
    }
    for (picture_index, x, y) in [(1, 100, 0), (2, 100, 65), (3, 100, 130)] {
        if let Some(picture) = illustrations.get(picture_index) {
            draw_picture(picture, x, y, pixels);
        }
    }
    let sound_mode = crate::settings::sound_effects_mode();
    let rows = [
        (
            0,
            "NONE",
            sound_mode == crate::settings::SoundEffectsMode::Off,
            20,
        ),
        (
            1,
            "PC",
            sound_mode == crate::settings::SoundEffectsMode::Pc,
            33,
        ),
        (
            2,
            "ADLIB/SB",
            sound_mode == crate::settings::SoundEffectsMode::Adlib,
            46,
        ),
        (3, "NONE", !crate::settings::digitized_effects_enabled(), 85),
        (4, "SDL", crate::settings::digitized_effects_enabled(), 98),
        (5, "NONE", !crate::settings::music_enabled(), 150),
        (6, "ADLIB/SB", crate::settings::music_enabled(), 163),
    ];
    for (index, label, active, y) in rows {
        if let Some(picture) = illustrations.get(if active { 5 } else { 4 }) {
            draw_picture(picture, 72, y + 2, pixels);
        }
        if index == selected {
            if let Some(cursor) = illustrations.get(6) {
                draw_picture(cursor, 48, y.saturating_sub(2), pixels);
            }
        }
        font.draw_text(
            pixels,
            WIDTH,
            HEIGHT,
            100,
            y,
            label,
            palette_color(if index == selected { 0x4f } else { 0x17 }, 1.0),
        );
    }
    for (index, y) in [(7, 176), (8, 189)] {
        let Some(label) = options.get(index) else {
            continue;
        };
        font.draw_text(
            pixels,
            WIDTH,
            HEIGHT,
            84,
            y,
            label,
            palette_color(if index == selected { 0x4f } else { 0x17 }, 1.0),
        );
        if index == selected {
            if let Some(cursor) = illustrations.get(6) {
                draw_picture(cursor, 54, y.saturating_sub(2), pixels);
            }
        }
    }
}

pub fn choice_menu_selection_at(
    layout: ChoiceMenuLayout,
    options_len: usize,
    selected: usize,
    x: usize,
    y: usize,
) -> Option<usize> {
    if layout == ChoiceMenuLayout::StartupSound {
        if !(40..290).contains(&x) {
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
            189..200 => 8,
            _ => return None,
        };
        return (index < options_len).then_some(index);
    }
    if layout == ChoiceMenuLayout::MouseSensitivity {
        if (97..107).contains(&y) && (60..260).contains(&x) {
            return Some(((x - 60) / 20).min(9));
        }
        return None;
    }
    let (left, right, first_row_y, row_height, visible_rows) = match layout {
        ChoiceMenuLayout::Main | ChoiceMenuLayout::StartupMain => (68, 246, 55, 13, options_len),
        ChoiceMenuLayout::Controls => (6, 314, 29, 10, options_len),
        ChoiceMenuLayout::StartupControls => (
            4,
            316,
            if options_len >= 20 { 40 } else { 50 },
            8,
            options_len,
        ),
        ChoiceMenuLayout::StartupControlPanel => (16, 300, 70, 13, options_len),
        ChoiceMenuLayout::StartupSound => unreachable!("handled above"),
        ChoiceMenuLayout::MouseSensitivity => unreachable!("handled above"),
        ChoiceMenuLayout::Episode => (6, 314, 23, 26, options_len),
        ChoiceMenuLayout::Difficulty => (45, 270, 100, 13, options_len),
        ChoiceMenuLayout::Jukebox => (20, 300, 48, 13, options_len.min(6)),
    };
    let page_start = if layout == ChoiceMenuLayout::Jukebox {
        selected / 6 * 6
    } else {
        0
    };
    if x < left || x >= right || y < first_row_y {
        return None;
    }
    let row = (y - first_row_y) / row_height;
    if row >= visible_rows {
        return None;
    }
    let index = page_start + row;
    (index < options_len).then_some(index)
}

fn draw_filled_rect(
    pixels: &mut [u32],
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    color: u32,
) {
    for row in y..y.saturating_add(height).min(HEIGHT) {
        let start = row * WIDTH + x.min(WIDTH);
        let end = row * WIDTH + x.saturating_add(width).min(WIDTH);
        if start < end {
            pixels[start..end].fill(color);
        }
    }
}

fn draw_menu_border(pixels: &mut [u32], x: usize, y: usize, width: usize, height: usize) {
    let outer = palette_color(0x29, 1.0);
    let inner = palette_color(0x23, 1.0);
    let right = x.saturating_add(width).saturating_sub(1).min(WIDTH - 1);
    let bottom = y.saturating_add(height).saturating_sub(1).min(HEIGHT - 1);
    if x >= WIDTH || y >= HEIGHT || width == 0 || height == 0 {
        return;
    }
    for px in x..=right {
        pixels[y * WIDTH + px] = outer;
        pixels[bottom * WIDTH + px] = inner;
    }
    for py in y..=bottom {
        pixels[py * WIDTH + x] = outer;
        pixels[py * WIDTH + right] = inner;
    }
}

fn draw_outline_rect(
    pixels: &mut [u32],
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    color: u32,
) {
    if width == 0 || height == 0 || x >= WIDTH || y >= HEIGHT {
        return;
    }
    let right = x.saturating_add(width - 1).min(WIDTH - 1);
    let bottom = y.saturating_add(height - 1).min(HEIGHT - 1);
    for px in x..=right {
        pixels[y * WIDTH + px] = color;
        pixels[bottom * WIDTH + px] = color;
    }
    for py in y..=bottom {
        pixels[py * WIDTH + x] = color;
        pixels[py * WIDTH + right] = color;
    }
}

pub(crate) fn draw_picture(picture: &Picture, x: usize, y: usize, pixels: &mut [u32]) {
    for source_y in 0..picture.height {
        for source_x in 0..picture.width {
            let target_x = x + source_x;
            let target_y = y + source_y;
            if target_x < WIDTH && target_y < HEIGHT {
                let color = picture.pixels[source_y * picture.width + source_x];
                pixels[target_y * WIDTH + target_x] = palette_color(color, 1.0);
            }
        }
    }
}

struct WallSampler<'a> {
    top: f64,
    wall_height: f64,
    ceiling: u8,
    tile: u16,
    page: Option<usize>,
    texture: Option<&'a [u8]>,
    texture_x: usize,
    show_historical_symbols: bool,
}

impl WallSampler<'_> {
    fn color_at(&self, y: f64) -> u32 {
        if y < self.top {
            return palette_color(self.ceiling, 1.0);
        }
        if y >= self.top + self.wall_height {
            return palette_color(FLOOR_COLOR, 1.0);
        }
        if let (Some(texture), Some(page)) = (self.texture, self.page) {
            let v = (((y - self.top) * 64.0) / self.wall_height.max(1.0)).clamp(0.0, 63.0) as usize;
            let source = texture[self.texture_x * 64 + v];
            palette_color(
                crate::symbols::wall_pixel(
                    self.show_historical_symbols,
                    page,
                    self.texture_x,
                    v,
                    source,
                ),
                1.0,
            )
        } else {
            let intensity = 220u32;
            let tint = 24 + (self.tile as u32 % 4) * 12;
            let red = intensity;
            let green = intensity.saturating_sub(tint);
            let blue = intensity.saturating_sub(tint * 2);
            0xFF000000 | (red << 16) | (green << 8) | blue
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_wall_sampler<'a>(
    hit: &RayHit,
    corrected: f64,
    viewport: Viewport,
    focal: f64,
    ceiling: u8,
    textures: Option<&'a Vswap>,
    doors: &DoorSystem,
    show_historical_symbols: bool,
    subpixel_rendering: bool,
) -> WallSampler<'a> {
    let mut wall_height = (focal / corrected).min((viewport.height * 2) as f64);
    let mut top = viewport.top as f64 + (viewport.height as f64 - wall_height) / 2.0;
    if !subpixel_rendering {
        wall_height = wall_height as isize as f64;
        top = top as isize as f64;
    }
    let page = if let (Some(index), Some(archive)) = (hit.door, textures) {
        doors
            .doors
            .get(index)
            .map(|door| DoorSystem::texture_page(door, archive.sprite_start(), hit.side))
    } else {
        let wall_id = (hit.tile & 0x3f).saturating_sub(1) as usize;
        Some(wall_id * 2 + usize::from(hit.side == 0))
    };
    let texture = textures.and_then(|archive| page.and_then(|page| archive.wall_texture(page)));
    let texture_x = hit
        .door
        .and_then(|index| doors.doors.get(index))
        .map(|door| (hit.door_texture_x + 64 - (door.position as usize >> 10)) & 63)
        .unwrap_or(hit.texture_x);
    WallSampler {
        top,
        wall_height,
        ceiling,
        tile: hit.tile,
        page,
        texture,
        texture_x,
        show_historical_symbols,
    }
}

fn average_sample_grid<const ROWS: usize>(
    samples: &[[[u32; ROWS]; 4]; 4],
    sample_count: usize,
    row: usize,
) -> u32 {
    if sample_count == 1 {
        return samples[0][0][row];
    }
    let mut red = 0u32;
    let mut green = 0u32;
    let mut blue = 0u32;
    for horizontal in 0..sample_count {
        for vertical in 0..sample_count {
            let color = samples[horizontal][vertical][row];
            red += (color >> 16) & 0xff;
            green += (color >> 8) & 0xff;
            blue += color & 0xff;
        }
    }
    let count = (sample_count * sample_count) as u32;
    0xff00_0000 | ((red / count) << 16) | ((green / count) << 8) | (blue / count)
}

fn draw_player_weapon(sprite: &Sprite, viewport: Viewport, stride: usize, pixels: &mut [u32]) {
    let scale = viewport.height as f64;
    let center_x = viewport.left as f64 + viewport.width as f64 / 2.0;
    let left = center_x - scale / 2.0;
    let top = viewport.bottom as f64 - scale;
    for column in sprite.left..=sprite.right {
        let x0 = (left + column as f64 * scale / 64.0).floor() as isize;
        let x1 = (left + (column + 1) as f64 * scale / 64.0).floor() as isize;
        for x in x0.max(viewport.left as isize)..x1.min(viewport.right as isize) {
            for post in &sprite.columns[column] {
                for (offset, &color) in post.pixels.iter().enumerate() {
                    let source_y = post.start + offset;
                    let y0 = (top + source_y as f64 * scale / 64.0).floor() as isize;
                    let y1 = (top + (source_y + 1) as f64 * scale / 64.0).floor() as isize;
                    for y in y0.max(viewport.top as isize)..y1.min(viewport.bottom as isize) {
                        pixels[y as usize * stride + x as usize] = palette_color(color, 1.0);
                    }
                }
            }
        }
    }
}

fn draw_statics(
    statics: &[ActorSpawn],
    enemies: &[ActorView],
    atlas: &SpriteAtlas,
    player: &Player,
    wall_depths: &[f64],
    visible_tiles: &[bool],
    map: &Map,
    viewport: Viewport,
    stride: usize,
    pixels: &mut [u32],
) {
    let (sin, cos) = (player.angle.sin(), player.angle.cos());
    let focal = viewport.width as f64 * VIEW_FACE_DISTANCE;
    let projection_scale = (viewport.width / view_width()).max(1) as f64;
    let mut visible = Vec::new();
    let static_views = statics.iter().filter_map(|actor| {
        let projection_offset = match actor.kind {
            ActorKind::Decoration(_)
                if visible_tiles
                    .get(actor.y * map.width + actor.x)
                    .copied()
                    .unwrap_or(false) =>
            {
                STATIC_PROJECTION_OFFSET
            }
            ActorKind::Enemy(crate::actors::EnemyKind::DeadGuard)
                if actor_tile_visible_or_open_neighbor(actor, map, visible_tiles) =>
            {
                ACTOR_PROJECTION_OFFSET
            }
            _ => return None,
        };
        let view = ActorView {
            x: actor.x as f64 + 0.5,
            y: actor.y as f64 + 0.5,
            kind: actor.kind,
            facing_degrees: u16::from(actor.direction) * 45,
            animation_frame: 0,
            moving: false,
            dying: false,
            attacking: false,
            pain: false,
        };
        Some((view, projection_offset))
    });
    let projected_actors = static_views.chain(
        enemies
            .iter()
            .copied()
            .map(|actor| (actor, ACTOR_PROJECTION_OFFSET)),
    );
    for (actor, projection_offset) in projected_actors {
        let dx = actor.x - player.x;
        let dy = actor.y - player.y;
        let actor_depth = dx * cos + dy * sin + CAMERA_FORWARD_OFFSET;
        let depth = actor_depth - projection_offset;
        if depth < PLAYER_RADIUS {
            continue;
        }
        let lateral = -dx * sin + dy * cos;
        let screen_offset = (lateral / depth * focal / projection_scale) as i32;
        let Some(shape) = shape_for_actor(
            actor.kind,
            actor.facing_degrees,
            player.angle,
            screen_offset,
            actor.animation_frame,
            actor.moving,
            actor.dying,
            actor.attacking,
            actor.pain,
        ) else {
            continue;
        };
        let Some(sprite) = atlas.sprite(shape) else {
            continue;
        };
        visible.push((depth, lateral, sprite));
    }
    visible.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (depth, lateral, sprite) in visible {
        draw_sprite(
            sprite,
            depth,
            lateral,
            focal,
            wall_depths,
            viewport,
            stride,
            pixels,
        );
    }
}

fn actor_tile_visible_or_open_neighbor(
    actor: &ActorSpawn,
    map: &Map,
    visible_tiles: &[bool],
) -> bool {
    let visible = |x: usize, y: usize| {
        visible_tiles
            .get(y * map.width + x)
            .copied()
            .unwrap_or(false)
    };
    if visible(actor.x, actor.y) {
        return true;
    }

    for dy in -1isize..=1 {
        for dx in -1isize..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let x = actor.x as isize + dx;
            let y = actor.y as isize + dy;
            if x < 0 || y < 0 || x >= map.width as isize || y >= map.height as isize {
                continue;
            }
            let (x, y) = (x as usize, y as usize);
            let tile = map.tile(x, y);
            // SetupGameLevel turns area-floor tiles into zero in DOS tilemap;
            // doors and walls remain nonzero, so they cannot expose neighbors.
            if (tile == 0 || tile >= 107) && visible(x, y) {
                return true;
            }
        }
    }
    false
}

fn draw_sprite(
    sprite: &Sprite,
    depth: f64,
    lateral: f64,
    focal: f64,
    wall_depths: &[f64],
    viewport: Viewport,
    stride: usize,
    pixels: &mut [u32],
) {
    // `ScaleShape` scales a 64x64 source to its projected full height and
    // centers it around the projected screen position.
    let scale = (focal / depth).clamp(0.25, (viewport.height * 2) as f64);
    let center_x = viewport.center_x + lateral / depth * focal;
    let left = center_x - scale / 2.0;
    let top = viewport.top as f64 + viewport.height as f64 / 2.0 - scale / 2.0;
    for column in sprite.left..=sprite.right {
        let x0 = (left + column as f64 * scale / 64.0).floor() as isize;
        let x1 = (left + (column + 1) as f64 * scale / 64.0).floor() as isize;
        for x in x0.max(viewport.left as isize)..x1.min(viewport.right as isize) {
            if wall_depths[x as usize] < depth {
                continue;
            }
            for post in &sprite.columns[column] {
                for (offset, &color) in post.pixels.iter().enumerate() {
                    let source_y = post.start + offset;
                    let y0 = (top + source_y as f64 * scale / 64.0).floor() as isize;
                    let y1 = (top + (source_y + 1) as f64 * scale / 64.0).floor() as isize;
                    for y in y0.max(viewport.top as isize)..y1.min(viewport.bottom as isize) {
                        pixels[y as usize * stride + x as usize] = palette_color(color, 1.0);
                    }
                }
            }
        }
    }
}

fn palette_color(index: u8, light: f64) -> u32 {
    let [red, green, blue] = GAME_PALETTE[index as usize];
    let expand = |value: u8| ((value as f64 * light).clamp(0.0, 63.0) as u32 * 255 / 63).min(255);
    0xFF000000 | (expand(red) << 16) | (expand(green) << 8) | expand(blue)
}

pub(crate) fn view_focal_length() -> f64 {
    view_width() as f64 * VIEW_FACE_DISTANCE
}

pub(crate) fn view_width() -> usize {
    usize::from(crate::settings::view_size()) * 16
}

pub(crate) fn view_bounds() -> (usize, usize, usize, usize) {
    let view = Viewport::current();
    (view.left, view.top, view.width, view.height)
}

struct RayHit {
    distance: f64,
    tile: u16,
    side: usize,
    texture_x: usize,
    /// DOS door drawing offsets the unflipped world intercept by doorposition.
    door_texture_x: usize,
    door: Option<usize>,
}

pub(crate) fn pickup_reachable(player: &Player, x: usize, y: usize) -> bool {
    let dx = x as f64 + 0.5 - player.x;
    let dy = y as f64 + 0.5 - player.y;
    let (sin, cos) = player.angle.sin_cos();
    // TransformTile offsets the tile center forward by the object's radius.
    let depth = dx * cos + dy * sin + CAMERA_FORWARD_OFFSET - STATIC_PROJECTION_OFFSET;
    let lateral = -dx * sin + dy * cos;
    (PLAYER_RADIUS..1.0).contains(&depth) && lateral.abs() < 0.5
}

pub(crate) fn visible_floor_tiles(
    map: &Map,
    doors: &DoorSystem,
    pushwalls: &PushWallSystem,
    player: &Player,
) -> Vec<bool> {
    let mut visible = vec![false; map.width * map.height];
    let view = Viewport::current();
    let focal = view.width as f64 * VIEW_FACE_DISTANCE;
    let blocked_tile = pushwalls.tilemap_blocking_cell();
    for x in 0..view.width {
        let offset = x as f64 + 0.5 - view.width as f64 / 2.0;
        let angle = player.angle + (offset / focal).atan();
        cast_ray(
            map,
            doors,
            pushwalls,
            player,
            angle,
            blocked_tile,
            &mut visible,
        );
    }
    visible
}

fn cast_ray(
    map: &Map,
    doors: &DoorSystem,
    pushwalls: &PushWallSystem,
    player: &Player,
    angle: f64,
    visibility_blocking_tile: Option<(usize, usize)>,
    visible: &mut [bool],
) -> RayHit {
    cast_ray_from_origin(
        map,
        doors,
        pushwalls,
        view_origin(player),
        angle,
        visibility_blocking_tile,
        visible,
    )
}

fn cast_ray_from_origin(
    map: &Map,
    doors: &DoorSystem,
    pushwalls: &PushWallSystem,
    (origin_x, origin_y): (f64, f64),
    angle: f64,
    visibility_blocking_tile: Option<(usize, usize)>,
    visible: &mut [bool],
) -> RayHit {
    let (dy, dx) = angle.sin_cos();
    let pushwall_hit = pushwalls.ray_hit(origin_x, origin_y, dx, dy);
    let (mut map_x, mut map_y) = (origin_x.floor() as isize, origin_y.floor() as isize);
    let delta_x = if dx == 0.0 {
        f64::INFINITY
    } else {
        (1.0 / dx).abs()
    };
    let delta_y = if dy == 0.0 {
        f64::INFINITY
    } else {
        (1.0 / dy).abs()
    };
    let step_x = if dx < 0.0 { -1 } else { 1 };
    let step_y = if dy < 0.0 { -1 } else { 1 };
    let mut side_x = if dx < 0.0 {
        (origin_x - map_x as f64) * delta_x
    } else {
        (map_x as f64 + 1.0 - origin_x) * delta_x
    };
    let mut side_y = if dy < 0.0 {
        (origin_y - map_y as f64) * delta_y
    } else {
        (map_y as f64 + 1.0 - origin_y) * delta_y
    };
    let mut side = 0usize;
    let mut tile = 0u16;
    let mut door_index = None;
    let mut distance = 32.0;
    let mut visibility_blocked = false;
    for _ in 0..(map.width + map.height) * 2 {
        if side_x < side_y {
            distance = side_x;
            side_x += delta_x;
            map_x += step_x;
            side = 0;
        } else {
            distance = side_y;
            side_y += delta_y;
            map_y += step_y;
            side = 1;
        }
        if map_x < 0 || map_y < 0 || map_x >= map.width as isize || map_y >= map.height as isize {
            break;
        }
        let in_blocking_tile = visibility_blocking_tile == Some((map_x as usize, map_y as usize));
        visibility_blocked |= in_blocking_tile;
        let tile_visible = !visibility_blocked
            && pushwall_hit
                .as_ref()
                .is_none_or(|hit| distance < hit.distance);
        tile = map.tile(map_x as usize, map_y as usize);
        if tile != 0 && tile < 107 {
            if pushwalls.masks_origin_tile(map_x as usize, map_y as usize) {
                // The pushwall has already left this part of its source tile.
                // Its moving box below supplies the ray hit and texture.
                if tile_visible {
                    visible[map_y as usize * map.width + map_x as usize] = true;
                }
                tile = 0;
                door_index = None;
                continue;
            }
            door_index = doors
                .at(map_x as usize, map_y as usize)
                .map(|(index, _)| index);
            let world_x = origin_x + distance * dx;
            let world_y = origin_y + distance * dy;
            if door_index.is_some_and(|index| doors.ray_passes(index, world_x, world_y)) {
                tile = 0;
                door_index = None;
                if tile_visible {
                    visible[map_y as usize * map.width + map_x as usize] = true;
                }
                continue;
            }
            break;
        }
        if tile_visible {
            visible[map_y as usize * map.width + map_x as usize] = true;
        }
    }
    let coordinate = if side == 0 {
        origin_y + distance * dy
    } else {
        origin_x + distance * dx
    };
    let door_texture_x = (((coordinate - coordinate.floor()) * 64.0) as usize).min(63);
    let mut texture_x = door_texture_x;
    if (side == 0 && dx < 0.0) || (side == 1 && dy > 0.0) {
        texture_x = 63 - texture_x;
    }
    let map_hit = RayHit {
        distance: distance.max(0.05),
        tile: tile.max(1),
        side,
        texture_x,
        door_texture_x,
        door: door_index,
    };
    if let Some(hit) = pushwall_hit {
        if hit.distance < map_hit.distance {
            return RayHit {
                distance: hit.distance.max(0.05),
                tile: hit.tile,
                side: hit.side,
                texture_x: hit.texture_x,
                door_texture_x: hit.texture_x,
                door: None,
            };
        }
    }
    map_hit
}

fn view_origin(player: &Player) -> (f64, f64) {
    (
        player.x - CAMERA_FORWARD_OFFSET * player.angle.cos(),
        player.y - CAMERA_FORWARD_OFFSET * player.angle.sin(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_view_matches_the_dos_layout() {
        assert_eq!(
            (VIEW_WIDTH, VIEW_HEIGHT, VIEW_LEFT, VIEW_TOP),
            (240, 120, 40, 20)
        );
        assert_eq!(VIEW_CENTER_X, 159.0);
        assert_eq!(VIEW_WIDTH / 10, 24);
        assert_eq!(ACTOR_PROJECTION_OFFSET, 0.25);
        let fov = 2.0 * (VIEW_WIDTH as f64 / (2.0 * view_focal_length())).atan();
        assert!((fov.to_degrees() - 72.0).abs() < 0.5);
    }

    #[test]
    fn renderer_origin_sits_focal_length_behind_the_player() {
        let player = Player {
            x: 5.0,
            y: 7.0,
            angle: 0.0,
            turn_remainder: 0,
            actor_tile_override: None,
        };
        let (x, y) = view_origin(&player);
        assert!((x - (5.0 - CAMERA_FORWARD_OFFSET)).abs() < 1e-12);
        assert!((y - 7.0).abs() < 1e-12);

        let player = Player {
            angle: std::f64::consts::FRAC_PI_2,
            ..player
        };
        let (x, y) = view_origin(&player);
        assert!((x - 5.0).abs() < 1e-12);
        assert!((y - (7.0 - CAMERA_FORWARD_OFFSET)).abs() < 1e-12);
    }

    #[test]
    fn original_info_plane_direction_is_converted() {
        let map = Map::demo();
        let player = Player::at_map_start(&map).unwrap();
        assert!((player.angle + std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn movement_stops_at_map_walls() {
        let map = Map::demo();
        let doors = DoorSystem::default();
        let pushwalls = PushWallSystem::default();
        let mut player = Player::default();
        for _ in 0..20 {
            player.update(
                &map,
                &doors,
                &pushwalls,
                1.0,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                false,
                &[],
                0.05,
            );
        }
        assert!(player.x < 2.7);
        assert!(player.x > 2.5);
        assert_eq!(player.y, 2.5);
    }

    #[test]
    fn movement_rates_match_the_original_tic_scales() {
        let map = Map::demo();
        let doors = DoorSystem::default();
        let pushwalls = PushWallSystem::default();
        let mut normal = Player {
            x: 1.5,
            y: 1.5,
            angle: 0.0,
            turn_remainder: 0,
            actor_tile_override: None,
        };
        normal.update(
            &map,
            &doors,
            &pushwalls,
            1.0,
            0.0,
            0.0,
            false,
            &[],
            1.0 / 70.0,
        );
        assert!((normal.x - 1.5801).abs() < 0.001);

        let mut running = Player { x: 1.5, ..normal };
        running.update(
            &map,
            &doors,
            &pushwalls,
            1.0,
            0.0,
            0.0,
            true,
            &[],
            1.0 / 70.0,
        );
        assert!((running.x - 1.6602).abs() < 0.001);

        let mut backwards = Player { x: 1.5, ..normal };
        backwards.update(
            &map,
            &doors,
            &pushwalls,
            -1.0,
            0.0,
            0.0,
            false,
            &[],
            1.0 / 70.0,
        );
        assert!((backwards.x - 1.4466).abs() < 0.001);

        let mut strafe_right = Player { x: 1.5, ..normal };
        strafe_right.update(
            &map,
            &doors,
            &pushwalls,
            0.0,
            1.0,
            0.0,
            false,
            &[],
            1.0 / 70.0,
        );
        assert!((strafe_right.y - 1.5801).abs() < 0.001);
    }

    #[test]
    fn renderer_fills_the_logical_frame() {
        let map = Map::demo();
        let doors = DoorSystem::default();
        let pushwalls = PushWallSystem::default();
        let mut pixels = vec![0; WIDTH * HEIGHT];
        render(
            &map,
            None,
            &doors,
            &pushwalls,
            &Player::default(),
            &[],
            &[],
            &SpriteAtlas::default(),
            None,
            None,
            None,
            "",
            &mut pixels,
        );
        assert!(pixels.iter().all(|pixel| *pixel != 0));
    }

    #[test]
    fn death_view_preserves_the_status_bar() {
        let mut pixels = vec![0xff20_4060; WIDTH * HEIGHT];
        render_death_view(&mut pixels);
        assert_eq!(pixels[80 * WIDTH + WIDTH / 2], palette_color(4, 1.0));
        assert_eq!(pixels[(HEIGHT - 1) * WIDTH], 0xff20_4060);
    }

    #[test]
    fn game_palette_is_a_full_vga_palette() {
        assert_eq!(GAME_PALETTE.len(), 256);
        assert!(
            GAME_PALETTE
                .iter()
                .flatten()
                .all(|component| *component <= 63)
        );
    }
}
