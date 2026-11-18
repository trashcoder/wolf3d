use crate::{
    doors::{DoorAction, DoorSystem},
    map::Map,
    pushwalls::PushWallSystem,
    weapons::Weapon,
    world::Player,
};
use std::collections::VecDeque;

const MECHA_MORPH_TICS: f64 = 30.0;
const WOLF_RANDOM_TABLE: [u8; 256] = [
    0, 8, 109, 220, 222, 241, 149, 107, 75, 248, 254, 140, 16, 66, 74, 21, 211, 47, 80, 242, 154,
    27, 205, 128, 161, 89, 77, 36, 95, 110, 85, 48, 212, 140, 211, 249, 22, 79, 200, 50, 28, 188,
    52, 140, 202, 120, 68, 145, 62, 70, 184, 190, 91, 197, 152, 224, 149, 104, 25, 178, 252, 182,
    202, 182, 141, 197, 4, 81, 181, 242, 145, 42, 39, 227, 156, 198, 225, 193, 219, 93, 122, 175,
    249, 0, 175, 143, 70, 239, 46, 246, 163, 53, 163, 109, 168, 135, 2, 235, 25, 92, 20, 145, 138,
    77, 69, 166, 78, 176, 173, 212, 166, 113, 94, 161, 41, 50, 239, 49, 111, 164, 70, 60, 2, 37,
    171, 75, 136, 156, 11, 56, 42, 146, 138, 229, 73, 146, 77, 61, 98, 196, 135, 106, 63, 197, 195,
    86, 96, 203, 113, 101, 170, 247, 181, 113, 80, 250, 108, 7, 255, 237, 129, 226, 79, 107, 112,
    166, 103, 241, 24, 223, 239, 120, 198, 58, 60, 82, 128, 3, 184, 66, 143, 224, 145, 224, 81,
    206, 163, 45, 63, 90, 168, 114, 59, 33, 159, 95, 28, 139, 123, 98, 125, 196, 15, 70, 194, 253,
    54, 14, 109, 226, 71, 17, 161, 93, 186, 87, 244, 138, 20, 52, 123, 251, 26, 36, 17, 46, 52,
    231, 232, 76, 31, 221, 84, 37, 216, 165, 212, 106, 197, 242, 98, 43, 39, 175, 254, 145, 190,
    84, 118, 222, 187, 136, 120, 163, 236, 249,
];

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
struct WolfRandom {
    cursor: u8,
}

impl WolfRandom {
    fn from_clock() -> Self {
        let cursor = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_millis()
            / 10) as u8;
        Self { cursor }
    }

    fn next(&mut self) -> u8 {
        self.cursor = self.cursor.wrapping_add(1);
        WOLF_RANDOM_TABLE[self.cursor as usize]
    }
}

/// Info-plane static object types marked as blocking by the original game.
/// Indices map to `statinfo[type].type == block` in `WL_ACT1.C`.
pub fn static_blocks_tile(tile: u16) -> bool {
    matches!(
        tile,
        24 | 25
            | 26
            | 28
            | 30
            | 31
            | 33
            | 34
            | 35
            | 36
            | 39
            | 40
            | 41
            | 45
            | 58
            | 59
            | 60
            | 62
            | 63
            | 68
            | 69
    )
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Difficulty {
    Baby,
    Easy,
    Medium,
    Hard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EnemyKind {
    Guard,
    Officer,
    Ss,
    Dog,
    Mutant,
    Boss(BossKind),
    Ghost(BossKind),
    DeadGuard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BossKind {
    Hans,
    Gretel,
    Gift,
    Fat,
    Schabbs,
    FakeHitler,
    MechaHitler,
    Hitler,
    GhostBlinky,
    GhostClyde,
    GhostPinky,
    GhostInky,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SpawnMode {
    Static,
    Stand,
    Patrol,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ActorKind {
    Decoration(u16),
    Enemy(EnemyKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ActorSpawn {
    pub x: usize,
    pub y: usize,
    pub source_tile: u16,
    pub kind: ActorKind,
    pub mode: SpawnMode,
    /// Info-plane direction offset: east, north, west, south, as in SpawnStand.
    pub direction: u8,
    pub ambush: bool,
}

#[derive(Debug, Default)]
pub struct ScanResult {
    pub actors: Vec<ActorSpawn>,
    pub has_pushwall: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ItemPickup {
    pub x: usize,
    pub y: usize,
    pub kind: PickupKind,
}

#[derive(Clone, Copy, Debug)]
pub struct ActorView {
    pub x: f64,
    pub y: f64,
    pub kind: ActorKind,
    pub facing_degrees: u16,
    pub animation_frame: u8,
    pub moving: bool,
    pub dying: bool,
    pub attacking: bool,
    pub pain: bool,
}

/// The original Wolf3D end-of-castle BJ run, jump, yell, and final pose.
pub struct BjVictory {
    x: f64,
    y: f64,
    direction: u8,
    run_tile: (isize, isize),
    run_distance_remaining: f64,
    run_initialized: bool,
    run_state: usize,
    run_state_tics: u8,
    initial_run_state_has_no_countdown: bool,
    tiles_left: u8,
    jump_state: u8,
    jump_state_tics: u16,
    finished: bool,
}

impl BjVictory {
    pub fn new(x: f64, y: f64, initial_run_countdown: u8) -> Self {
        Self {
            x,
            y,
            direction: 2,
            run_tile: (x.floor() as isize, y.floor() as isize + 1),
            run_distance_remaining: 0.0,
            run_initialized: false,
            run_state: 0,
            run_state_tics: if initial_run_countdown == 0 {
                0
            } else {
                12 - initial_run_countdown.min(11)
            },
            initial_run_state_has_no_countdown: initial_run_countdown == 0,
            tiles_left: 6,
            jump_state: 0,
            jump_state_tics: 0,
            finished: false,
        }
    }

    /// Advances one DOS simulation tic. Returns BJ's positioned YEAHSND cue.
    pub fn update_tic(&mut self, map: &Map) -> Option<SoundEvent> {
        if self.finished {
            return None;
        }
        if self.tiles_left > 0 {
            const RUN_STATES: [(u8, usize, bool); 6] = [
                (12, 408, true),
                (3, 408, false),
                (8, 409, true),
                (12, 410, true),
                (3, 410, false),
                (8, 411, true),
            ];
            if !self.initial_run_state_has_no_countdown {
                self.run_state_tics += 1;
                if self.run_state_tics >= RUN_STATES[self.run_state].0 {
                    self.run_state_tics = 0;
                    self.run_state = (self.run_state + 1) % RUN_STATES.len();
                }
            }
            if RUN_STATES[self.run_state].2 {
                self.move_for_tic(map, 2_048.0 / 65_536.0);
            }
            if self.tiles_left == 0 {
                self.jump_state = 0;
                self.jump_state_tics = 0;
            }
            return None;
        }

        if self.jump_state < 3 {
            self.move_jump_for_tic(680.0 / 65_536.0);
        }
        self.jump_state_tics += 1;
        if self.jump_state < 3 && self.jump_state_tics >= 14 {
            self.jump_state += 1;
            self.jump_state_tics = 0;
            return (self.jump_state == 1).then(|| SoundEvent::at(72, self.x, self.y));
        }
        if self.jump_state == 3 && self.jump_state_tics >= 300 {
            self.finished = true;
        } else if self.jump_state == 2 && self.jump_state_tics >= 14 {
            self.jump_state = 3;
            self.jump_state_tics = 0;
        }
        None
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn view(&self) -> ActorView {
        let sprite = if self.tiles_left > 0 {
            [408, 408, 409, 410, 410, 411][self.run_state]
        } else {
            412 + usize::from(self.jump_state.min(3))
        };
        ActorView {
            x: self.x,
            y: self.y,
            kind: ActorKind::Decoration((sprite - 2) as u16),
            facing_degrees: u16::from(self.direction) * 45,
            animation_frame: 0,
            moving: false,
            dying: false,
            attacking: false,
            pain: false,
        }
    }

    fn move_for_tic(&mut self, map: &Map, distance: f64) {
        const DX: [f64; 8] = [1.0, 1.0, 0.0, -1.0, -1.0, -1.0, 0.0, 1.0];
        const DY: [f64; 8] = [0.0, -1.0, -1.0, -1.0, 0.0, 1.0, 1.0, 1.0];
        if self.direction >= 8 {
            return;
        }
        if !self.run_initialized {
            self.x = self.run_tile.0 as f64 + 0.5;
            self.y = self.run_tile.1 as f64 + 0.5;
            self.run_initialized = true;
            self.arrive_at_run_tile(map);
            if self.tiles_left == 0 || self.direction >= 8 {
                return;
            }
        }

        let step = distance.min(self.run_distance_remaining);
        let direction = self.direction as usize;
        self.x += DX[direction] * step;
        self.y += DY[direction] * step;
        self.run_distance_remaining -= step;
        if self.run_distance_remaining <= f64::EPSILON {
            self.run_tile.0 += DX[direction] as isize;
            self.run_tile.1 += DY[direction] as isize;
            self.x = self.run_tile.0 as f64 + 0.5;
            self.y = self.run_tile.1 as f64 + 0.5;
            self.arrive_at_run_tile(map);
        }
    }

    fn move_jump_for_tic(&mut self, distance: f64) {
        const DX: [f64; 8] = [1.0, 1.0, 0.0, -1.0, -1.0, -1.0, 0.0, 1.0];
        const DY: [f64; 8] = [0.0, -1.0, -1.0, -1.0, 0.0, 1.0, 1.0, 1.0];
        if self.direction < 8 {
            self.x += DX[self.direction as usize] * distance;
            self.y += DY[self.direction as usize] * distance;
        }
    }

    fn arrive_at_run_tile(&mut self, map: &Map) {
        if self.run_tile.0 < 0
            || self.run_tile.1 < 0
            || self.run_tile.0 >= map.width as isize
            || self.run_tile.1 >= map.height as isize
        {
            self.direction = 8;
            return;
        }
        let tile_x = self.run_tile.0 as usize;
        let tile_y = self.run_tile.1 as usize;
        let path = map.info_tile(tile_x, tile_y);
        if (90..98).contains(&path) {
            const PATH_DIRECTIONS: [u8; 8] = [3, 2, 1, 4, 8, 0, 5, 6];
            self.direction = PATH_DIRECTIONS[(path - 90) as usize];
        }
        self.tiles_left = self.tiles_left.saturating_sub(1);
        if self.tiles_left == 0 {
            return;
        }
        if self.direction >= 8 {
            return;
        }
        let dx = [1isize, 1, 0, -1, -1, -1, 0, 1][self.direction as usize];
        let dy = [0isize, -1, -1, -1, 0, 1, 1, 1][self.direction as usize];
        let target_x = self.run_tile.0 + dx;
        let target_y = self.run_tile.1 + dy;
        if target_x < 0
            || target_y < 0
            || target_x >= map.width as isize
            || target_y >= map.height as isize
            || {
                let tile = map.tile(target_x as usize, target_y as usize);
                tile != 0 && tile < 107
            }
        {
            self.direction = 8;
        } else {
            self.run_distance_remaining = 1.0;
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
enum ProjectileKind {
    Needle,
    Rocket,
    Fire,
}

impl ProjectileKind {
    fn sprite(self, frame: u8) -> usize {
        match self {
            Self::Needle => 317 + (frame as usize % 4),
            Self::Rocket => 370,
            Self::Fire => 326 + (frame as usize % 2),
        }
    }

    fn six_tic_state_count(elapsed: f64) -> f64 {
        // Needles and fire start with ticcount = 1, then cycle every six tics.
        ((elapsed * 70.0 + 5.0 + 1e-9) / 6.0).floor()
    }

    fn speed(self) -> f64 {
        match self {
            Self::Needle | Self::Rocket => 0x2000 as f64 * 70.0 / 65_536.0,
            Self::Fire => 0x1200 as f64 * 70.0 / 65_536.0,
        }
    }

    fn damage(self, random: u8, difficulty: Difficulty) -> u16 {
        let damage = match self {
            Self::Needle => (random >> 3) as u16 + 20,
            Self::Rocket => (random >> 3) as u16 + 30,
            Self::Fire => (random >> 3) as u16,
        };
        scale_damage(damage, difficulty)
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
struct ActiveProjectile {
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
    elapsed: f64,
    explosion_elapsed: Option<f64>,
    kind: ProjectileKind,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
struct ActiveSmoke {
    x: f64,
    y: f64,
    elapsed: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct KillReward {
    pub x: usize,
    pub y: usize,
    pub points: u32,
    pub pickup: Option<PickupKind>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ShotOutcome {
    pub hit: bool,
    pub kill: Option<KillReward>,
    pub sound: Option<SoundEvent>,
}

#[derive(Debug, Default)]
pub struct EnemyUpdate {
    pub damage: u16,
    pub damage_source: Option<PlayerDamageSource>,
    pub fatal_hit_position: Option<(f64, f64)>,
    pub sounds: Vec<SoundEvent>,
    pub victory: bool,
    pub boss_death_interstitial: bool,
    pub boss_death_camera: Option<BossDeathCamera>,
}

impl EnemyUpdate {
    fn record_damage(
        &mut self,
        damage: u16,
        source: PlayerDamageSource,
        position: (f64, f64),
        player_health: u8,
        accept_player_damage: bool,
    ) {
        if damage == 0 || !accept_player_damage {
            return;
        }
        self.damage = self.damage.saturating_add(damage);
        self.damage_source = Some(source);
        if self.damage >= u16::from(player_health) {
            self.fatal_hit_position = Some(position);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerDamageSource {
    Needle,
    Other,
}

#[derive(Clone, Copy, Debug)]
pub struct SoundEvent {
    pub sound: usize,
    pub position: Option<(f64, f64)>,
}

impl SoundEvent {
    fn at(sound: usize, x: f64, y: f64) -> Self {
        Self {
            sound,
            position: Some((x, y)),
        }
    }

    fn global(sound: usize) -> Self {
        Self {
            sound,
            position: None,
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct BossDeathCamera {
    pub boss_x: f64,
    pub boss_y: f64,
    pub origin_x: f64,
    pub origin_y: f64,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
enum EnemyMode {
    Standing,
    Patrolling,
    Alerting(f64),
    Chasing,
    Attacking { elapsed: f64 },
    Pain { elapsed: f64, frame: u8 },
    Dying { elapsed: f64 },
    Dead,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ActiveEnemy {
    #[serde(default)]
    active: bool,
    x: f64,
    y: f64,
    kind: EnemyKind,
    direction: u8,
    last_decision_tile: Option<(isize, isize)>,
    ambush: bool,
    first_attack: bool,
    mode: EnemyMode,
    #[serde(default)]
    alerting_from_patrol: bool,
    animation_frame: u8,
    animation_elapsed: f64,
    patrol_elapsed: f64,
    #[serde(default)]
    initial_state_has_no_countdown: bool,
    #[serde(default)]
    last_drawn_visible: bool,
    death_screams_played: u8,
    death_camera_passes: u8,
    death_camera_origin: Option<(f64, f64)>,
    attack_remaining: f64,
    chase_elapsed: f64,
    contact_damage_remainder: f64,
    hitpoints: i32,
}

#[derive(Clone, Copy, Debug)]
struct ActorTileClaim {
    index: usize,
    tile_x: usize,
    tile_y: usize,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EnemyRuntime {
    enemies: Vec<ActiveEnemy>,
    projectiles: Vec<ActiveProjectile>,
    smoke: Vec<ActiveSmoke>,
    random: WolfRandom,
    #[serde(default = "digitized_effects_default")]
    digitized_effects_enabled_at_spawn: bool,
    #[serde(default)]
    face_frame: u8,
    #[serde(default)]
    face_count: u16,
    #[serde(default)]
    last_damage_was_needle: bool,
    difficulty: Difficulty,
    player_made_noise: bool,
    death_camera_pause_tics: u16,
    death_camera_interstitial_tics: u16,
    death_camera_terminal_pause_tics: u16,
    pending_death_camera: Option<(usize, BossDeathCamera)>,
}

impl EnemyRuntime {
    /// Advances the shared DOS random table for UI and actor-spawn calls that
    /// consume `US_RndT`.
    pub fn next_dos_random(&mut self) -> u8 {
        self.random.next()
    }

    pub fn is_valid_for_map(&self, map: &Map, difficulty: Difficulty) -> bool {
        let inside = |x: f64, y: f64| {
            x.is_finite()
                && y.is_finite()
                && x >= 0.0
                && y >= 0.0
                && x < map.width as f64
                && y < map.height as f64
        };
        let valid_timer = |timer: f64| timer.is_finite() && timer >= 0.0;
        if self.difficulty != difficulty
            || self.face_frame > 2
            || self.face_count > 255
            || self.enemies.len() > 4096
            || self.projectiles.len() > 4096
            || self.smoke.len() > 8192
            || self.death_camera_pause_tics > 300
            || self.death_camera_interstitial_tics > 300
            || self.death_camera_terminal_pause_tics > 300
        {
            return false;
        }
        let enemies_valid = self.enemies.iter().all(|enemy| {
            let mode_valid = match enemy.mode {
                EnemyMode::Alerting(elapsed)
                | EnemyMode::Attacking { elapsed }
                | EnemyMode::Pain { elapsed, .. }
                | EnemyMode::Dying { elapsed } => valid_timer(elapsed),
                EnemyMode::Standing
                | EnemyMode::Patrolling
                | EnemyMode::Chasing
                | EnemyMode::Dead => true,
            };
            inside(enemy.x, enemy.y)
                && enemy.direction < 8
                && enemy.last_decision_tile.is_none_or(|(x, y)| {
                    x >= 0 && y >= 0 && x < map.width as isize && y < map.height as isize
                })
                && mode_valid
                && (!enemy.alerting_from_patrol || matches!(enemy.mode, EnemyMode::Alerting(_)))
                && valid_timer(enemy.animation_elapsed)
                && valid_timer(enemy.patrol_elapsed)
                && valid_timer(enemy.attack_remaining)
                && valid_timer(enemy.chase_elapsed)
                && valid_timer(enemy.contact_damage_remainder)
                && enemy.death_camera_origin.is_none_or(|(x, y)| inside(x, y))
        });
        let projectiles_valid = self.projectiles.iter().all(|projectile| {
            inside(projectile.x, projectile.y)
                && projectile.dx.is_finite()
                && projectile.dy.is_finite()
                && (projectile.dx * projectile.dx + projectile.dy * projectile.dy - 1.0).abs()
                    < 0.01
                && valid_timer(projectile.elapsed)
                && projectile
                    .explosion_elapsed
                    .is_none_or(|elapsed| valid_timer(elapsed) && elapsed < 18.0 / 70.0)
        });
        let smoke_valid = self.smoke.iter().all(|smoke| {
            inside(smoke.x, smoke.y) && valid_timer(smoke.elapsed) && smoke.elapsed < 15.0 / 70.0
        });
        let pending_camera_valid = self.pending_death_camera.is_none_or(|(index, camera)| {
            index < self.enemies.len()
                && inside(camera.boss_x, camera.boss_y)
                && inside(camera.origin_x, camera.origin_y)
        });
        enemies_valid && projectiles_valid && smoke_valid && pending_camera_valid
    }

    pub fn from_spawns(
        spawns: &[ActorSpawn],
        difficulty: Difficulty,
        demo: bool,
        digitized_effects_enabled: bool,
    ) -> Self {
        let mut random = if demo {
            WolfRandom::default()
        } else {
            WolfRandom::from_clock()
        };
        let enemies = spawns
            .iter()
            .filter_map(|spawn| {
                let ActorKind::Enemy(kind) = spawn.kind else {
                    return None;
                };
                if kind == EnemyKind::DeadGuard {
                    return None;
                }
                let is_wolf_ghost = matches!(kind, EnemyKind::Ghost(_));
                let first_state_tics = if is_wolf_ghost {
                    10
                } else if spawn.mode == SpawnMode::Patrol {
                    20
                } else {
                    0
                };
                let countdown = (first_state_tics != 0).then(|| random.next() % first_state_tics);
                // DoActor keeps thinking in the initial state when ticcount is zero.
                let initial_state_has_no_countdown = countdown == Some(0);
                let elapsed = match countdown {
                    Some(remaining) if remaining > 0 => {
                        f64::from(first_state_tics - remaining) / 70.0
                    }
                    _ => 0.0,
                };
                let mode = if is_wolf_ghost {
                    // Wolf ghosts enter their chase state directly from the
                    // info plane instead of waiting for sight or player noise.
                    EnemyMode::Chasing
                } else if spawn.mode == SpawnMode::Patrol {
                    EnemyMode::Patrolling
                } else {
                    EnemyMode::Standing
                };
                Some(ActiveEnemy {
                    active: spawn.mode == SpawnMode::Patrol,
                    x: spawn.x as f64 + 0.5,
                    y: spawn.y as f64 + 0.5,
                    kind,
                    direction: if matches!(kind, EnemyKind::Ghost(_)) {
                        0 // SpawnGhosts starts them facing east.
                    } else {
                        (spawn.direction % 4) * 2
                    },
                    last_decision_tile: None,
                    ambush: spawn.ambush,
                    first_attack: false,
                    mode,
                    alerting_from_patrol: false,
                    animation_frame: 0,
                    animation_elapsed: 0.0,
                    patrol_elapsed: if is_wolf_ghost { 0.0 } else { elapsed },
                    initial_state_has_no_countdown,
                    last_drawn_visible: false,
                    death_screams_played: 0,
                    death_camera_passes: 0,
                    death_camera_origin: None,
                    attack_remaining: 0.0,
                    chase_elapsed: if is_wolf_ghost { elapsed } else { 0.0 },
                    contact_damage_remainder: 0.0,
                    hitpoints: enemy_hitpoints(kind, difficulty),
                })
            })
            .collect();
        Self {
            enemies,
            projectiles: Vec::new(),
            smoke: Vec::new(),
            random,
            digitized_effects_enabled_at_spawn: digitized_effects_enabled,
            face_frame: 0,
            face_count: 0,
            last_damage_was_needle: false,
            difficulty,
            player_made_noise: false,
            death_camera_pause_tics: 0,
            death_camera_interstitial_tics: 0,
            death_camera_terminal_pause_tics: 0,
            pending_death_camera: None,
        }
    }

    pub fn update_hud_face(&mut self, tics: u16, is_gatling_sound_playing: bool) -> bool {
        if is_gatling_sound_playing {
            return false;
        }
        self.face_count = self.face_count.saturating_add(tics);
        if self.face_count > u16::from(self.random.next()) {
            let frame = self.random.next() >> 6;
            self.face_frame = if frame == 3 { 1 } else { frame };
            self.face_count = 0;
            return true;
        }
        false
    }

    pub fn reset_hud_face_timer(&mut self) {
        self.face_count = 0;
    }

    pub fn hud_face_frame(&self) -> u8 {
        self.face_frame
    }

    pub fn last_damage_was_needle(&self) -> bool {
        self.last_damage_was_needle
    }

    pub fn is_death_camera_active(&self) -> bool {
        self.death_camera_pause_tics > 0
            || self.death_camera_interstitial_tics > 0
            || self.death_camera_terminal_pause_tics > 0
            || self.pending_death_camera.is_some()
    }

    pub fn is_boss_death_interstitial_active(&self) -> bool {
        self.death_camera_interstitial_tics > 0
    }

    pub fn skip_boss_death_interstitial(&mut self) {
        self.death_camera_interstitial_tics = 0;
    }

    fn begin_death_camera_replay(&mut self) -> Option<BossDeathCamera> {
        let (index, camera) = self.pending_death_camera.take()?;
        if let Some(enemy) = self.enemies.get_mut(index) {
            enemy.mode = EnemyMode::Dying { elapsed: 0.0 };
            enemy.animation_frame = 0;
            enemy.death_screams_played = death_camera_replay_screams(enemy.kind);
        }
        Some(camera)
    }

    pub fn player_blockers(&self) -> Vec<(f64, f64)> {
        self.enemies
            .iter()
            .filter(|enemy| {
                !matches!(enemy.kind, EnemyKind::Ghost(_))
                    && !matches!(enemy.mode, EnemyMode::Dying { .. } | EnemyMode::Dead)
            })
            .map(|enemy| (enemy.x, enemy.y))
            .collect()
    }

    /// Returns the live actorat cells used by `PushWall` to reject a target
    /// tile. DOS reserves an enemy's next tile as soon as `TryWalk` succeeds.
    pub fn pushwall_blocking_tiles(&self) -> Vec<(usize, usize)> {
        self.enemies
            .iter()
            .filter(|enemy| !matches!(enemy.mode, EnemyMode::Dying { .. } | EnemyMode::Dead))
            .map(enemy_route_tile)
            .collect()
    }

    pub fn update(
        &mut self,
        map: &Map,
        doors: &mut DoorSystem,
        pushwalls: &PushWallSystem,
        player: &Player,
        player_health: u8,
        player_running: bool,
        dt: f64,
        map9_death_scream_available: bool,
        accept_player_damage: bool,
        victory_active: bool,
    ) -> EnemyUpdate {
        let dt = dt.clamp(0.0, 0.25);
        let mut update = EnemyUpdate::default();
        if self.death_camera_terminal_pause_tics > 0 {
            let elapsed_tics = (dt * 70.0).round() as u16;
            self.death_camera_terminal_pause_tics = self
                .death_camera_terminal_pause_tics
                .saturating_sub(elapsed_tics);
            if self.death_camera_terminal_pause_tics == 0 {
                update.victory = true;
            }
            return update;
        }
        if self.death_camera_pause_tics > 0 {
            let elapsed_tics = (dt * 70.0).round() as u16;
            self.death_camera_pause_tics =
                self.death_camera_pause_tics.saturating_sub(elapsed_tics);
            if self.death_camera_pause_tics == 0 {
                self.death_camera_interstitial_tics = 300;
                update.boss_death_interstitial = true;
            }
            return update;
        }
        if self.death_camera_interstitial_tics > 0 {
            let elapsed_tics = (dt * 70.0).round() as u16;
            self.death_camera_interstitial_tics = self
                .death_camera_interstitial_tics
                .saturating_sub(elapsed_tics);
            if self.death_camera_interstitial_tics == 0 {
                update.boss_death_camera = self.begin_death_camera_replay();
            }
            return update;
        }
        if self.pending_death_camera.is_some() {
            update.boss_death_camera = self.begin_death_camera_replay();
            return update;
        }
        let player_made_noise = std::mem::take(&mut self.player_made_noise);
        let connected_tiles = area_reachable_tiles(map, doors, pushwalls, player);
        // DOS DoActor refreshes actorat between actors, reserving the route's
        // destination tile as soon as TryWalk succeeds.
        let mut other_positions: Vec<_> = self
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, enemy)| !matches!(enemy.mode, EnemyMode::Dying { .. } | EnemyMode::Dead))
            .map(|(index, enemy)| actor_tile_claim(index, enemy))
            .collect();
        for index in 0..self.enemies.len() {
            let mut chase_elapsed_after_transition = None;
            let mut enemy = self.enemies[index].clone();
            if !enemy.active && !tile_is_reachable(&connected_tiles, map, enemy.x, enemy.y) {
                continue;
            }
            if matches!(enemy.mode, EnemyMode::Dead) {
                self.enemies[index] = enemy;
                continue;
            }
            if let EnemyMode::Dying { elapsed } = enemy.mode {
                let old_tics = elapsed * 70.0;
                let new_elapsed = elapsed + dt;
                let new_tics = new_elapsed * 70.0;
                let replay_offset = if enemy.death_camera_passes > 0 {
                    death_camera_replay_prefix(enemy.kind)
                } else {
                    0.0
                };
                let next_scream = death_scream_tic(enemy.kind, enemy.death_screams_played as usize)
                    .map(|tics| tics + replay_offset);
                if next_scream.is_some_and(|threshold| {
                    (threshold == 0.0 && old_tics == 0.0)
                        || (old_tics < threshold && new_tics >= threshold)
                }) {
                    if let Some(sound) = enemy_death_sound(
                        enemy.kind,
                        map.number,
                        map9_death_scream_available,
                        &mut self.random,
                    ) {
                        let event = if enemy_death_sound_is_local(enemy.kind) {
                            SoundEvent::at(sound, enemy.x, enemy.y)
                        } else {
                            SoundEvent::global(sound)
                        };
                        update.sounds.push(event);
                    }
                    enemy.death_screams_played = enemy.death_screams_played.saturating_add(1);
                }
                if enemy.kind == EnemyKind::Boss(BossKind::Hitler) {
                    let slurpie_tic = 1.0
                        + boss_death_wait_tics(self.digitized_effects_enabled_at_spawn)
                        + 10.0
                        + replay_offset;
                    if old_tics < slurpie_tic && new_tics >= slurpie_tic {
                        update.sounds.push(SoundEvent::global(61));
                    }
                }
                if enemy.kind == EnemyKind::Boss(BossKind::MechaHitler)
                    && elapsed < MECHA_MORPH_TICS / 70.0
                    && new_elapsed >= MECHA_MORPH_TICS / 70.0
                {
                    let mut hitler = enemy.clone();
                    let first_chase_tics_remaining = self.random.next() % 6;
                    hitler.kind = EnemyKind::Boss(BossKind::Hitler);
                    hitler.hitpoints = transformed_hitler_hitpoints(self.difficulty);
                    hitler.mode = EnemyMode::Chasing;
                    hitler.animation_frame = 0;
                    hitler.animation_elapsed = 0.0;
                    hitler.initial_state_has_no_countdown = first_chase_tics_remaining == 0;
                    hitler.death_screams_played = 0;
                    hitler.attack_remaining = 0.0;
                    // SpawnNewObj randomizes s_hitlerchase1's six-tic timer;
                    // DOS processes the appended actor later in this same tic.
                    hitler.chase_elapsed = if first_chase_tics_remaining == 0 {
                        0.0
                    } else {
                        f64::from(7 - first_chase_tics_remaining) / 70.0
                    };
                    self.enemies.push(hitler);
                    let spawned_index = self.enemies.len() - 1;
                    other_positions.push(actor_tile_claim(
                        spawned_index,
                        &self.enemies[spawned_index],
                    ));
                }
                let (frame, finished) = death_animation_frame(
                    enemy.kind,
                    new_elapsed,
                    enemy.death_camera_passes > 0,
                    self.digitized_effects_enabled_at_spawn,
                );
                enemy.animation_frame = frame;
                enemy.mode = if finished {
                    if has_death_camera(enemy.kind) && enemy.death_camera_passes == 0 {
                        enemy.death_camera_passes = 1;
                        let (origin_x, origin_y) =
                            enemy.death_camera_origin.unwrap_or((player.x, player.y));
                        self.pending_death_camera = Some((
                            index,
                            BossDeathCamera {
                                boss_x: enemy.x,
                                boss_y: enemy.y,
                                origin_x,
                                origin_y,
                            },
                        ));
                        self.death_camera_pause_tics = 100;
                    } else if starts_victory_sequence(enemy.kind) {
                        self.death_camera_terminal_pause_tics = 100;
                    }
                    EnemyMode::Dead
                } else {
                    EnemyMode::Dying {
                        elapsed: new_elapsed,
                    }
                };
                self.enemies[index] = enemy;
                update_actor_collision_position(&mut other_positions, index, &self.enemies[index]);
                continue;
            }
            if let EnemyMode::Attacking { elapsed } = enemy.mode {
                let sequence = attack_sequence(enemy.kind).expect("attack mode has a sequence");
                let old_tics = elapsed * 70.0;
                let new_elapsed = elapsed + dt;
                let new_tics = new_elapsed * 70.0;
                for (stage_index, boundary, active) in attack_stage_boundaries(sequence) {
                    // These routines occupy the DOS state action field, so
                    // DoActor calls them once when the state timer expires.
                    if !active || old_tics + 1e-9 >= boundary || new_tics + 1e-9 < boundary {
                        continue;
                    }
                    let projectile = projectile_kind(enemy.kind).filter(|_| {
                        enemy.kind != EnemyKind::Boss(BossKind::Fat) || stage_index == 1
                    });
                    let hitscan_action = projectile.is_none() && enemy.kind != EnemyKind::Dog;
                    let connected_to_player =
                        tile_is_reachable(&connected_tiles, map, enemy.x, enemy.y);
                    let line_is_clear = has_line_of_sight(
                        map, doors, pushwalls, enemy.x, enemy.y, player.x, player.y,
                    );
                    let shot_clear = line_is_clear && (!hitscan_action || connected_to_player);
                    if let Some(kind) = projectile {
                        let dx = player.x - enemy.x;
                        let dy = enemy.y - player.y;
                        let angle = f64::from(projectile_angle(dx, dy)).to_radians();
                        self.projectiles.push(ActiveProjectile {
                            x: enemy.x,
                            y: enemy.y,
                            dx: angle.cos(),
                            dy: -angle.sin(),
                            elapsed: 0.0,
                            explosion_elapsed: None,
                            kind,
                        });
                    } else {
                        let player_visible = enemy.last_drawn_visible;
                        let damage = attack_player(
                            &mut enemy,
                            player,
                            self.difficulty,
                            shot_clear,
                            player_running,
                            player_visible,
                            &mut self.random,
                        );
                        update.record_damage(
                            damage,
                            PlayerDamageSource::Other,
                            (enemy.x, enemy.y),
                            player_health,
                            accept_player_damage,
                        );
                    }
                    if let Some(sound) = enemy_attack_sound(enemy.kind) {
                        if !hitscan_action || shot_clear {
                            update.sounds.push(SoundEvent::at(sound, enemy.x, enemy.y));
                        }
                    }
                }
                enemy.animation_frame = attack_stage_index(sequence, new_tics);
                let total_tics: f64 = sequence.iter().map(|stage| stage.tics as f64).sum();
                if new_tics + 1e-9 >= total_tics {
                    enemy.mode = EnemyMode::Chasing;
                    chase_elapsed_after_transition = Some((new_tics - total_tics).max(0.0) / 70.0);
                    enemy.attack_remaining = 0.0;
                    enemy.animation_frame = 0;
                    // NewState(s_hitlerchase1) installs its six-tic timer
                    // after Hitler's first attack, even if the morph's
                    // initial chase state had a zero-tic countdown.
                    enemy.initial_state_has_no_countdown = false;
                } else {
                    enemy.mode = EnemyMode::Attacking {
                        elapsed: new_elapsed,
                    };
                    self.enemies[index] = enemy;
                    update_actor_collision_position(
                        &mut other_positions,
                        index,
                        &self.enemies[index],
                    );
                    continue;
                }
            }
            if let EnemyMode::Pain { elapsed, frame } = enemy.mode {
                let elapsed = elapsed + dt;
                if elapsed * 70.0 + 1e-9 >= 10.0 {
                    enemy.mode = EnemyMode::Chasing;
                    chase_elapsed_after_transition = Some((elapsed - 10.0 / 70.0).max(0.0));
                    enemy.animation_frame = 0;
                } else {
                    enemy.mode = EnemyMode::Pain { elapsed, frame };
                    enemy.animation_frame = frame;
                    self.enemies[index] = enemy;
                    update_actor_collision_position(
                        &mut other_positions,
                        index,
                        &self.enemies[index],
                    );
                    continue;
                }
            }
            if enemy_is_patrolling(&enemy) && !enemy.initial_state_has_no_countdown {
                enemy.patrol_elapsed += dt;
            }
            let patrol_think_active =
                enemy_is_patrolling(&enemy) && patrol_state_thinks(enemy.patrol_elapsed);
            let sees_player = matches!(enemy.mode, EnemyMode::Standing | EnemyMode::Patrolling)
                && (!matches!(enemy.mode, EnemyMode::Patrolling) || patrol_think_active)
                && (check_sight(
                    map,
                    doors,
                    pushwalls,
                    &connected_tiles,
                    enemy.x,
                    enemy.y,
                    enemy.direction,
                    player,
                ) || (!enemy.ambush
                    && player_made_noise
                    && tile_is_reachable(&connected_tiles, map, enemy.x, enemy.y)));
            let mut entered_chase_this_update = false;
            match enemy.mode {
                EnemyMode::Standing | EnemyMode::Patrolling if sees_player => {
                    let delay = reaction_delay(enemy.kind, &mut self.random);
                    enemy.alerting_from_patrol = matches!(enemy.mode, EnemyMode::Patrolling);
                    enemy.ambush = false;
                    enemy.mode = EnemyMode::Alerting(delay);
                    enemy.attack_remaining = 0.0;
                }
                EnemyMode::Alerting(remaining) => {
                    let reaction_tick = !enemy.alerting_from_patrol || patrol_think_active;
                    if let Some(remaining) =
                        reaction_time_after_update(remaining, dt, reaction_tick)
                    {
                        enemy.mode = EnemyMode::Alerting(remaining);
                    } else {
                        enemy.mode = EnemyMode::Chasing;
                        enemy.alerting_from_patrol = false;
                        enemy.chase_elapsed = 0.0;
                        enemy.initial_state_has_no_countdown = false;
                        entered_chase_this_update = true;
                        enemy.first_attack = true;
                        enemy.last_decision_tile = None;
                        if let Some(sound) = first_sighting_sound(enemy.kind) {
                            let event = if first_sighting_sound_is_local(enemy.kind) {
                                SoundEvent::at(sound, enemy.x, enemy.y)
                            } else {
                                SoundEvent::global(sound)
                            };
                            update.sounds.push(event);
                        }
                    }
                }
                _ => {}
            }

            enemy.attack_remaining = (enemy.attack_remaining - dt).max(0.0);
            let next_chase_elapsed = chase_elapsed_after_transition.unwrap_or_else(|| {
                if enemy.initial_state_has_no_countdown {
                    enemy.chase_elapsed
                } else {
                    enemy.chase_elapsed + dt
                }
            });
            let is_chasing = matches!(enemy.mode, EnemyMode::Chasing);
            let chase_think_active = is_chasing
                && !(victory_active
                    && enemy.kind != EnemyKind::Dog
                    && !matches!(enemy.kind, EnemyKind::Ghost(_)))
                && !entered_chase_this_update
                && chase_state_thinks(enemy.kind, next_chase_elapsed);
            let player_in_sight = chase_think_active
                && has_line_of_sight(map, doors, pushwalls, enemy.x, enemy.y, player.x, player.y);
            let decision_tile = (enemy.x.floor() as isize, enemy.y.floor() as isize);
            let dog_route_finished = enemy.last_decision_tile.is_none_or(|(route_x, route_y)| {
                route_reached_tile_center(enemy.x, enemy.y, route_x, route_y, enemy.direction)
            });
            if enemy.kind == EnemyKind::Dog
                && chase_think_active
                && enemy.last_decision_tile != Some(decision_tile)
                && dog_route_finished
            {
                if let Some(direction) = select_dodge_direction(
                    map,
                    doors,
                    pushwalls,
                    &mut enemy,
                    player,
                    &mut self.random,
                    index,
                    &other_positions,
                ) {
                    enemy.direction = direction;
                    enemy.last_decision_tile = Some(decision_tile);
                } else {
                    enemy.direction = 8;
                    enemy.last_decision_tile = None;
                }
            }
            let dog_attack_ready = enemy.kind == EnemyKind::Dog
                && chase_think_active
                && enemy.last_decision_tile.is_some()
                && (enemy.x - player.x).abs() - enemy_speed(enemy.kind, true) * dt <= 1.0
                && (enemy.y - player.y).abs() - enemy_speed(enemy.kind, true) * dt <= 1.0;
            let waiting_for_door = enemy_waiting_for_door(&enemy, doors);
            let mut attack_started = false;
            let attack_triggered = dog_attack_ready
                || (player_in_sight
                    && enemy.attack_remaining == 0.0
                    && !matches!(enemy.kind, EnemyKind::Ghost(_))
                    && enemy.kind != EnemyKind::Dog
                    && attack_roll_succeeds(
                        &mut enemy,
                        player,
                        dt,
                        waiting_for_door,
                        &mut self.random,
                    ));
            if attack_triggered {
                attack_started = true;
                if attack_sequence(enemy.kind).is_some() {
                    enemy.mode = EnemyMode::Attacking { elapsed: 0.0 };
                    enemy.animation_frame = 0;
                } else {
                    let player_visible = enemy.last_drawn_visible;
                    let damage = attack_player(
                        &mut enemy,
                        player,
                        self.difficulty,
                        true,
                        player_running,
                        player_visible,
                        &mut self.random,
                    );
                    update.record_damage(
                        damage,
                        PlayerDamageSource::Other,
                        (enemy.x, enemy.y),
                        player_health,
                        accept_player_damage,
                    );
                    enemy.attack_remaining = 0.0;
                }
            }
            let use_dodge_direction =
                should_use_dodge_direction(enemy.kind, player_in_sight, attack_started);
            let (dx, dy) = match enemy.mode {
                EnemyMode::Patrolling | EnemyMode::Alerting(_) if patrol_think_active => {
                    let route_finished =
                        enemy.last_decision_tile.is_none_or(|(route_x, route_y)| {
                            route_reached_tile_center(
                                enemy.x,
                                enemy.y,
                                route_x,
                                route_y,
                                enemy.direction,
                            )
                        });
                    if enemy.last_decision_tile != Some(decision_tile) && !route_finished {
                        direction_vector(enemy.direction)
                    } else if enemy.last_decision_tile != Some(decision_tile) {
                        let cell_x = decision_tile.0 as usize;
                        let cell_y = decision_tile.1 as usize;
                        let path = map.planes[1][cell_y * 64 + cell_x];
                        let direction = if (90..=97).contains(&path) {
                            (path - 90) as u8
                        } else {
                            enemy.direction
                        };
                        if direction < 8
                            && chase_step_is_open(
                                map,
                                doors,
                                pushwalls,
                                decision_tile.0,
                                decision_tile.1,
                                direction,
                                enemy_can_open_doors(enemy.kind),
                                index,
                                &other_positions,
                            )
                        {
                            enemy.direction = direction;
                            enemy.last_decision_tile = Some(decision_tile);
                            direction_vector(direction)
                        } else {
                            enemy.direction = 8;
                            enemy.last_decision_tile = None;
                            (0.0, 0.0)
                        }
                    } else {
                        direction_vector(enemy.direction)
                    }
                }
                EnemyMode::Chasing if chase_think_active => {
                    let route_finished =
                        enemy.last_decision_tile.is_none_or(|(route_x, route_y)| {
                            route_reached_tile_center(
                                enemy.x,
                                enemy.y,
                                route_x,
                                route_y,
                                enemy.direction,
                            )
                        });
                    if enemy.last_decision_tile != Some(decision_tile) && !route_finished {
                        direction_vector(enemy.direction)
                    } else if enemy.kind == EnemyKind::Dog
                        && enemy.last_decision_tile != Some(decision_tile)
                    {
                        (0.0, 0.0)
                    } else if enemy.last_decision_tile == Some(decision_tile) {
                        direction_vector(enemy.direction)
                    } else {
                        let (player_tile_x, player_tile_y) = player.actor_tile();
                        let delta_x = player_tile_x as isize - decision_tile.0;
                        let delta_y = player_tile_y as isize - decision_tile.1;
                        // DOS bosses choose a chase or dodge direction while
                        // `dir == nodir`; SelectRunDir is only used after they
                        // consume a tile of their current route.
                        let run_from_player = enemy.last_decision_tile.is_some()
                            && matches!(
                                enemy.kind,
                                EnemyKind::Boss(BossKind::Schabbs | BossKind::Gift | BossKind::Fat)
                            )
                            && delta_x.unsigned_abs().max(delta_y.unsigned_abs()) < 4;
                        let direction = if run_from_player {
                            select_run_direction(
                                map,
                                doors,
                                pushwalls,
                                &enemy,
                                player,
                                &mut self.random,
                                index,
                                &other_positions,
                            )
                        } else if use_dodge_direction {
                            select_dodge_direction(
                                map,
                                doors,
                                pushwalls,
                                &mut enemy,
                                player,
                                &mut self.random,
                                index,
                                &other_positions,
                            )
                        } else {
                            select_chase_direction(
                                map,
                                doors,
                                pushwalls,
                                &mut enemy,
                                player,
                                &mut self.random,
                                index,
                                &other_positions,
                            )
                        };
                        if let Some(direction) = direction {
                            enemy.direction = direction;
                            enemy.last_decision_tile = Some(decision_tile);
                            direction_vector(direction)
                        } else {
                            enemy.direction = 8;
                            enemy.last_decision_tile = None;
                            (0.0, 0.0)
                        }
                    }
                }
                _ => (0.0, 0.0),
            };
            let moving = (dx != 0.0 || dy != 0.0)
                && !waiting_for_door
                && !attack_started
                && !matches!(enemy.mode, EnemyMode::Attacking { .. });
            if moving {
                if enemy_is_patrolling(&enemy) {
                    enemy.animation_frame = patrol_animation_frame(enemy.patrol_elapsed);
                } else {
                    enemy.animation_elapsed += dt;
                    let animation_time = if matches!(enemy.mode, EnemyMode::Chasing) {
                        next_chase_elapsed
                    } else {
                        enemy.animation_elapsed
                    };
                    enemy.animation_frame = walk_animation_frame(enemy.kind, animation_time);
                }
                let speed = enemy_speed(enemy.kind, matches!(enemy.mode, EnemyMode::Chasing));
                let distance = speed * dt;
                let steps = (distance / 0.08).ceil().max(1.0) as usize;
                let step_x = dx * distance / steps as f64;
                let step_y = dy * distance / steps as f64;
                let can_open_doors = enemy_can_open_doors(enemy.kind);
                let actor_area_connected =
                    tile_is_reachable(&connected_tiles, map, enemy.x, enemy.y);
                let route_target = enemy.last_decision_tile.map(|(route_x, route_y)| {
                    route_target_center(route_x, route_y, enemy.direction)
                });
                for _ in 0..steps {
                    let next_x = route_target.map_or(enemy.x + step_x, |(target_x, _)| {
                        clamp_route_step(enemy.x, enemy.x + step_x, target_x)
                    });
                    let next_y = route_target.map_or(enemy.y + step_y, |(_, target_y)| {
                        clamp_route_step(enemy.y, enemy.y + step_y, target_y)
                    });
                    if matches!(enemy.kind, EnemyKind::Ghost(_))
                        && (next_x - player.x).abs() <= 1.0
                        && (next_y - player.y).abs() <= 1.0
                        && tile_is_reachable(&connected_tiles, map, enemy.x, enemy.y)
                    {
                        // DOS MoveObj applies contact damage only when a ghost
                        // moves into the player's MINACTORDIST box, then backs
                        // the attempted movement out. A ghost stuck against a
                        // wall does not keep hurting a nearby player.
                        let elapsed_tics = (dt * 70.0).round() as u16;
                        let damage = scale_damage(elapsed_tics.saturating_mul(2), self.difficulty);
                        update.record_damage(
                            damage,
                            PlayerDamageSource::Other,
                            (next_x, next_y),
                            player_health,
                            accept_player_damage,
                        );
                        break;
                    }
                    if actor_area_connected
                        && (next_x - player.x).abs() <= 1.0
                        && (next_y - player.y).abs() <= 1.0
                    {
                        // DOS MoveObj tests the complete attempted move, then
                        // backs out both axes when it enters MINACTORDIST.
                        continue;
                    }
                    if enemy_can_move(map, doors, pushwalls, next_x, enemy.y, can_open_doors) {
                        enemy.x = next_x;
                    }
                    if enemy_can_move(map, doors, pushwalls, enemy.x, next_y, can_open_doors) {
                        enemy.y = next_y;
                    }
                }
            } else if enemy_is_patrolling(&enemy) {
                enemy.animation_frame = patrol_animation_frame(enemy.patrol_elapsed);
            } else if !matches!(enemy.mode, EnemyMode::Chasing) {
                enemy.animation_elapsed = 0.0;
                enemy.animation_frame = 0;
            }
            if matches!(enemy.mode, EnemyMode::Chasing) {
                // DOS DoActor advances the chase state clock even when the
                // thinker finds no route or movement is blocked.
                enemy.animation_frame = walk_animation_frame(enemy.kind, next_chase_elapsed);
            }
            if matches!(enemy.mode, EnemyMode::Chasing)
                && !entered_chase_this_update
                && !victory_active
            {
                if enemy.kind == EnemyKind::Boss(BossKind::MechaHitler)
                    && tile_is_reachable(&connected_tiles, map, enemy.x, enemy.y)
                {
                    let old_tics = enemy.chase_elapsed * 70.0;
                    let new_tics = (enemy.chase_elapsed + dt) * 70.0;
                    let mut sound_tic = ((old_tics - 10.0) / 24.0).floor() * 24.0 + 10.0;
                    if sound_tic <= old_tics {
                        sound_tic += 24.0;
                    }
                    while sound_tic <= new_tics + 1e-9 {
                        update.sounds.push(SoundEvent::at(70, enemy.x, enemy.y));
                        sound_tic += 24.0;
                    }
                }
                enemy.chase_elapsed = next_chase_elapsed;
            }
            self.enemies[index] = enemy;
            update_actor_collision_position(&mut other_positions, index, &self.enemies[index]);
        }
        for smoke in &mut self.smoke {
            smoke.elapsed += dt;
        }
        self.smoke.retain(|smoke| smoke.elapsed < 15.0 / 70.0);
        let mut remaining = Vec::with_capacity(self.projectiles.len());
        for mut projectile in self.projectiles.drain(..) {
            if let Some(elapsed) = &mut projectile.explosion_elapsed {
                *elapsed += dt;
                if *elapsed < 18.0 / 70.0 {
                    remaining.push(projectile);
                }
                continue;
            }
            let previous_elapsed = projectile.elapsed;
            projectile.elapsed += dt;
            if matches!(projectile.kind, ProjectileKind::Rocket) {
                // The first rocket state expires at tic 1, then every 3 tics.
                let smoke_count = |elapsed: f64| ((elapsed * 70.0 + 2.0 + 1e-9) / 3.0) as u64;
                for _ in smoke_count(previous_elapsed)..smoke_count(projectile.elapsed) {
                    self.smoke.push(ActiveSmoke {
                        x: projectile.x,
                        y: projectile.y,
                        // DOS appends the puff to the actor list and ticks it
                        // in the same pass that spawned it.
                        elapsed: dt,
                    });
                }
            }
            let movement_time = if matches!(projectile.kind, ProjectileKind::Fire) {
                // Fake Hitler's fire moves in the state action, on expiry.
                let actions = ProjectileKind::six_tic_state_count(projectile.elapsed)
                    - ProjectileKind::six_tic_state_count(previous_elapsed);
                if actions == 0.0 {
                    remaining.push(projectile);
                    continue;
                }
                actions / 70.0
            } else {
                dt
            };
            let travel = projectile.kind.speed() * movement_time;
            let steps = (travel / 0.08).ceil().max(1.0) as usize;
            let step_x = projectile.dx * travel / steps as f64;
            let step_y = projectile.dy * travel / steps as f64;
            let mut remove = false;
            for _ in 0..steps {
                let next_x = projectile.x + step_x;
                let next_y = projectile.y + step_y;
                if !projectile_can_move(map, doors, pushwalls, next_x, next_y) {
                    if matches!(projectile.kind, ProjectileKind::Rocket) {
                        projectile.x = next_x;
                        projectile.y = next_y;
                        update.sounds.push(SoundEvent::at(86, next_x, next_y));
                        // T_Projectile changes the state without resetting ticcount.
                        // The first explosion frame inherits the rocket's remaining tics.
                        let flight_tics = (projectile.elapsed * 70.0).round();
                        let remaining_tics = 3.0 - (flight_tics - 1.0).rem_euclid(3.0);
                        projectile.explosion_elapsed = Some((6.0 - remaining_tics) / 70.0);
                    } else {
                        remove = true;
                    }
                    break;
                }
                projectile.x = next_x;
                projectile.y = next_y;
                if (projectile.x - player.x).abs() < 0.75 && (projectile.y - player.y).abs() < 0.75
                {
                    let random = self.random.next();
                    let damage = projectile.kind.damage(random, self.difficulty);
                    let source = if matches!(projectile.kind, ProjectileKind::Needle) {
                        PlayerDamageSource::Needle
                    } else {
                        PlayerDamageSource::Other
                    };
                    update.record_damage(
                        damage,
                        source,
                        (projectile.x, projectile.y),
                        player_health,
                        accept_player_damage,
                    );
                    remove = true;
                    break;
                }
            }
            if !remove {
                remaining.push(projectile);
            }
        }
        self.projectiles = remaining;
        // DrawScaleds marks visible actors active after DoActor has run.
        if self.enemies.iter().any(|enemy| !enemy.active) {
            let visible = crate::world::visible_floor_tiles(map, doors, pushwalls, player);
            for enemy in &mut self.enemies {
                if enemy.active {
                    continue;
                }
                // DOS DrawScaleds checks the actor's reserved tilex/tiley;
                // TryWalk moves those to the destination before arrival.
                let (tile_x, tile_y) = enemy_route_tile(enemy);
                let x = tile_x as isize;
                let y = tile_y as isize;
                enemy.active = (-1..=1).any(|dy| {
                    (-1..=1).any(|dx| {
                        let (nx, ny) = (x + dx, y + dy);
                        if nx < 0 || ny < 0 || nx >= map.width as isize || ny >= map.height as isize
                        {
                            return false;
                        }
                        let (nx, ny) = (nx as usize, ny as usize);
                        visible[ny * map.width + nx]
                            && ((dx == 0 && dy == 0)
                                || map.tile(nx, ny) == 0
                                || map.tile(nx, ny) >= 107)
                    })
                });
            }
        }
        if let Some(source) = update.damage_source {
            self.last_damage_was_needle = source == PlayerDamageSource::Needle;
        }
        update
    }

    /// Stores the visibility result produced by the last rendered world frame,
    /// matching DOS `DrawScaleds` setting `FL_VISABLE` after `DoActor`.
    pub fn refresh_drawn_visibility(
        &mut self,
        map: &Map,
        doors: &DoorSystem,
        pushwalls: &PushWallSystem,
        player: &Player,
    ) {
        let visible_tiles = crate::world::visible_floor_tiles(map, doors, pushwalls, player);
        for enemy in &mut self.enemies {
            enemy.last_drawn_visible =
                actor_is_visible_to_player(map, &visible_tiles, enemy, player);
        }
    }

    pub fn views(&self) -> Vec<ActorView> {
        let mut views: Vec<_> = self
            .enemies
            .iter()
            .map(|enemy| ActorView {
                x: enemy.x,
                y: enemy.y,
                kind: ActorKind::Enemy(enemy.kind),
                facing_degrees: u16::from(enemy.direction) * 45,
                animation_frame: enemy.animation_frame,
                moving: enemy_is_patrolling(enemy) || matches!(enemy.mode, EnemyMode::Chasing),
                dying: matches!(enemy.mode, EnemyMode::Dying { .. } | EnemyMode::Dead),
                attacking: matches!(enemy.mode, EnemyMode::Attacking { .. }),
                pain: matches!(enemy.mode, EnemyMode::Pain { .. }),
            })
            .collect();
        views.extend(self.projectiles.iter().map(|projectile| {
            let shape = if let Some(elapsed) = projectile.explosion_elapsed {
                382 + ((elapsed * 70.0 / 6.0) as usize).min(2)
            } else {
                projectile
                    .kind
                    .sprite((ProjectileKind::six_tic_state_count(projectile.elapsed) % 4.0) as u8)
            };
            ActorView {
                x: projectile.x,
                y: projectile.y,
                kind: ActorKind::Decoration((shape - 2) as u16),
                facing_degrees: projectile_angle(projectile.dx, -projectile.dy),
                animation_frame: 0,
                moving: false,
                dying: false,
                attacking: false,
                pain: false,
            }
        }));
        views.extend(self.smoke.iter().map(|smoke| {
            let elapsed_tics = smoke.elapsed * 70.0;
            let frame = if elapsed_tics < 6.0 {
                0u16
            } else if elapsed_tics < 9.0 {
                1
            } else if elapsed_tics < 12.0 {
                2
            } else {
                3
            };
            ActorView {
                x: smoke.x,
                y: smoke.y,
                kind: ActorKind::Decoration(376 + frame),
                facing_degrees: 0,
                animation_frame: 0,
                moving: false,
                dying: false,
                attacking: false,
                pain: false,
            }
        }));
        views
    }

    /// Fires a hitscan shot along the player's view and damages its nearest
    /// visible target. Returns true when an enemy was hit.
    pub fn shoot(
        &mut self,
        map: &Map,
        doors: &DoorSystem,
        pushwalls: &PushWallSystem,
        player: &Player,
        weapon: Weapon,
        best_weapon: Weapon,
        map9_death_scream_available: bool,
    ) -> ShotOutcome {
        self.player_made_noise |= weapon.uses_ammo();
        let mut target = None;
        let mut nearest = f64::INFINITY;
        let (sin, cos) = player.angle.sin_cos();
        let focal_length = crate::world::view_focal_length();
        let aim_tolerance = (crate::world::view_width() / 10) as i32;
        for (index, enemy) in self.enemies.iter().enumerate() {
            if matches!(enemy.kind, EnemyKind::Ghost(_))
                || matches!(enemy.mode, EnemyMode::Dying { .. } | EnemyMode::Dead)
            {
                continue;
            }
            let dx = enemy.x - player.x;
            let dy = enemy.y - player.y;
            let depth = dx * cos + dy * sin + crate::world::CAMERA_FORWARD_OFFSET
                - crate::world::ACTOR_PROJECTION_OFFSET;
            if depth < crate::world::PLAYER_RADIUS || depth >= nearest {
                continue;
            }
            let lateral = -dx * sin + dy * cos;
            let screen_offset = (focal_length * lateral / depth) as i32;
            if screen_offset.abs() >= aim_tolerance
                || !has_line_of_sight(map, doors, pushwalls, enemy.x, enemy.y, player.x, player.y)
            {
                continue;
            }
            nearest = depth;
            target = Some(index);
        }
        let Some(index) = target else {
            return ShotOutcome::default();
        };
        if !weapon_range_allows(weapon, nearest) {
            return ShotOutcome::default();
        }
        if weapon == Weapon::Knife {
            // DOS `DamageActor` sets `madenoise` for a successful knife hit;
            // a missed knife swing stays quiet.
            self.player_made_noise = true;
        }
        let (enemies, random) = (&mut self.enemies, &mut self.random);
        let enemy = &mut enemies[index];
        // DOS `GunAttack` measures damage from the actor's reserved tile,
        // which `TryWalk` updates before movement reaches the tile center.
        let (enemy_tile_x, enemy_tile_y) = enemy_route_tile(enemy);
        let (player_tile_x, player_tile_y) = player.actor_tile();
        let tile_distance = (enemy_tile_x as i32 - player_tile_x as i32)
            .abs()
            .max((enemy_tile_y as i32 - player_tile_y as i32).abs());
        let roll = random.next() as i32;
        let damage = if weapon == Weapon::Knife {
            roll >> 4
        } else if tile_distance < 2 {
            roll / 4
        } else if tile_distance < 4 {
            roll / 6
        } else {
            if (roll / 12) < tile_distance {
                return ShotOutcome::default();
            }
            random.next() as i32 / 6
        };
        enemy.ambush = false;
        enemy.alerting_from_patrol = false;
        let was_inactive = !matches!(
            enemy.mode,
            EnemyMode::Chasing | EnemyMode::Attacking { .. } | EnemyMode::Pain { .. }
        );
        let damage = if was_inactive { damage * 2 } else { damage };
        enemy.hitpoints -= damage;
        if was_inactive && enemy.hitpoints > 0 {
            enemy.first_attack = true;
            enemy.initial_state_has_no_countdown = false;
        }
        let mut kill = None;
        let mut death_sound = None;
        if enemy.hitpoints <= 0 {
            enemy.death_camera_origin =
                has_death_camera(enemy.kind).then_some((player.x, player.y));
            enemy.mode = if death_animation_tics(enemy.kind).is_some() {
                EnemyMode::Dying { elapsed: 0.0 }
            } else {
                EnemyMode::Dead
            };
            enemy.animation_frame = 0;
            enemy.death_screams_played = 0;
            if death_scream_tic(enemy.kind, 0) == Some(0.0) {
                death_sound =
                    enemy_death_sound(enemy.kind, map.number, map9_death_scream_available, random);
                enemy.death_screams_played = 1;
            }
            let (points, pickup) = match enemy.kind {
                EnemyKind::Guard => (100, Some(PickupKind::Ammo(4))),
                EnemyKind::Officer => (400, Some(PickupKind::Ammo(4))),
                EnemyKind::Mutant => (700, Some(PickupKind::Ammo(4))),
                EnemyKind::Ss if best_weapon < Weapon::MachineGun => {
                    (500, Some(PickupKind::Weapon(Weapon::MachineGun)))
                }
                EnemyKind::Ss => (500, Some(PickupKind::Ammo(4))),
                EnemyKind::Dog => (200, None),
                EnemyKind::Boss(BossKind::Hans | BossKind::Gretel) => {
                    (5_000, Some(PickupKind::Key(0)))
                }
                EnemyKind::Boss(BossKind::Gift | BossKind::Fat | BossKind::Schabbs) => {
                    (5_000, None)
                }
                EnemyKind::Boss(BossKind::FakeHitler) => (2_000, None),
                EnemyKind::Boss(BossKind::MechaHitler) => (5_000, None),
                EnemyKind::Boss(BossKind::Hitler) => (5_000, None),
                EnemyKind::Ghost(_)
                | EnemyKind::Boss(
                    BossKind::GhostBlinky
                    | BossKind::GhostClyde
                    | BossKind::GhostPinky
                    | BossKind::GhostInky,
                ) => (200, None),
                EnemyKind::DeadGuard => (0, None),
            };
            kill = Some(KillReward {
                x: enemy.x.floor() as usize,
                y: enemy.y.floor() as usize,
                points,
                pickup,
            });
        } else {
            if has_pain_animation(enemy.kind) {
                let frame = if enemy.hitpoints & 1 == 0 { 1 } else { 0 };
                enemy.animation_frame = frame;
                enemy.mode = EnemyMode::Pain {
                    elapsed: 0.0,
                    frame,
                };
            } else if was_inactive {
                enemy.mode = EnemyMode::Chasing;
                enemy.chase_elapsed = 0.0;
                enemy.last_decision_tile = None;
            }
        }
        let sound = death_sound.or_else(|| {
            (was_inactive && enemy.hitpoints > 0)
                .then(|| first_sighting_sound(enemy.kind))
                .flatten()
        });
        ShotOutcome {
            hit: true,
            kill,
            sound: sound.map(|sound| {
                if enemy_death_sound_is_local(enemy.kind)
                    || (was_inactive && first_sighting_sound_is_local(enemy.kind))
                {
                    SoundEvent::at(sound, enemy.x, enemy.y)
                } else {
                    SoundEvent::global(sound)
                }
            }),
        }
    }
}

fn death_animation_tics(kind: EnemyKind) -> Option<(u8, f64)> {
    match kind {
        EnemyKind::Guard | EnemyKind::Dog | EnemyKind::Ss => Some((3, 15.0)),
        EnemyKind::Officer => Some((4, 11.0)),
        EnemyKind::Mutant => Some((4, 7.0)),
        EnemyKind::Boss(BossKind::Hans | BossKind::Gretel) => Some((3, 15.0)),
        EnemyKind::Boss(BossKind::Schabbs) => Some((5, 10.0)),
        EnemyKind::Boss(BossKind::FakeHitler) => Some((5, 10.0)),
        EnemyKind::Boss(BossKind::MechaHitler) => Some((3, 10.0)),
        EnemyKind::Boss(BossKind::Hitler) => Some((9, 10.0)),
        EnemyKind::Boss(BossKind::Gift | BossKind::Fat) => Some((5, 10.0)),
        _ => None,
    }
}

/// Tick when `A_DeathScream` runs for each death sequence. Schabbs and Hitler
/// are called once directly by `KillActor` and again by their first death
/// state, so each has a second timed scream.
fn death_scream_tic(kind: EnemyKind, scream_index: usize) -> Option<f64> {
    match kind {
        EnemyKind::Guard | EnemyKind::Dog | EnemyKind::Ss if scream_index == 0 => Some(15.0),
        EnemyKind::Officer if scream_index == 0 => Some(11.0),
        EnemyKind::Mutant if scream_index == 0 => Some(7.0),
        EnemyKind::Boss(BossKind::Hans | BossKind::Gretel) if scream_index == 0 => Some(15.0),
        EnemyKind::Boss(BossKind::Schabbs) => match scream_index {
            0 => Some(0.0),
            1 => Some(10.0),
            _ => None,
        },
        EnemyKind::Boss(BossKind::Hitler) => match scream_index {
            0 => Some(0.0),
            1 => Some(1.0),
            _ => None,
        },
        EnemyKind::Boss(BossKind::FakeHitler | BossKind::MechaHitler) if scream_index == 0 => {
            Some(10.0)
        }
        EnemyKind::Boss(BossKind::Gift | BossKind::Fat) if scream_index == 0 => Some(1.0),
        _ => None,
    }
}

fn death_camera_replay_prefix(kind: EnemyKind) -> f64 {
    match kind {
        EnemyKind::Boss(BossKind::Schabbs | BossKind::Gift | BossKind::Fat) => 1.0,
        EnemyKind::Boss(BossKind::Hitler) => 10.0,
        _ => 0.0,
    }
}

fn death_camera_replay_screams(kind: EnemyKind) -> u8 {
    match kind {
        EnemyKind::Boss(BossKind::Schabbs | BossKind::Hitler) => 1,
        _ => 0,
    }
}

fn digitized_effects_default() -> bool {
    true
}

fn boss_death_wait_tics(digitized_effects_enabled: bool) -> f64 {
    if digitized_effects_enabled {
        140.0
    } else {
        5.0
    }
}

fn has_death_camera(kind: EnemyKind) -> bool {
    matches!(
        kind,
        EnemyKind::Boss(BossKind::Schabbs | BossKind::Gift | BossKind::Fat | BossKind::Hitler)
    )
}

fn death_animation_frame(
    kind: EnemyKind,
    elapsed_seconds: f64,
    replay: bool,
    digitized_effects_enabled: bool,
) -> (u8, bool) {
    let elapsed_tics = elapsed_seconds.max(0.0) * 70.0;
    let replay_prefix = if replay {
        death_camera_replay_prefix(kind)
    } else {
        0.0
    };
    if elapsed_tics < replay_prefix {
        return (0, false);
    }
    let elapsed_tics = elapsed_tics - replay_prefix;
    let boss_wait_tics = boss_death_wait_tics(digitized_effects_enabled);
    match kind {
        EnemyKind::Boss(BossKind::Gift | BossKind::Fat) => {
            return timed_animation_frame(
                elapsed_tics,
                &[1.0, boss_wait_tics, 10.0, 10.0, 10.0, 20.0],
            );
        }
        EnemyKind::Boss(BossKind::Schabbs) => {
            return timed_animation_frame(
                elapsed_tics,
                &[10.0, boss_wait_tics, 10.0, 10.0, 10.0, 20.0],
            );
        }
        EnemyKind::Boss(BossKind::Hitler) => {
            return timed_animation_frame(
                elapsed_tics,
                &[
                    1.0,
                    boss_wait_tics,
                    10.0,
                    10.0,
                    10.0,
                    10.0,
                    10.0,
                    10.0,
                    10.0,
                    20.0,
                ],
            );
        }
        _ => {}
    }
    let Some((frames, tics_per_frame)) = death_animation_tics(kind) else {
        return (0, true);
    };
    let frame = (elapsed_tics / tics_per_frame).floor() as u8;
    if frame >= frames {
        (frames, true)
    } else {
        (frame, false)
    }
}

fn starts_victory_sequence(kind: EnemyKind) -> bool {
    has_death_camera(kind)
}

fn timed_animation_frame(elapsed_tics: f64, frame_durations: &[f64]) -> (u8, bool) {
    let mut boundary = 0.0;
    for (frame, duration) in frame_durations.iter().enumerate() {
        boundary += duration;
        if elapsed_tics < boundary {
            return (frame as u8, false);
        }
    }
    (frame_durations.len() as u8, true)
}

fn walk_animation_frame(kind: EnemyKind, elapsed_seconds: f64) -> u8 {
    let elapsed_tics = elapsed_seconds * 70.0;
    if matches!(kind, EnemyKind::Ghost(_)) {
        return ((elapsed_tics / 10.0).floor() as u8) % 2;
    }
    let durations = match kind {
        EnemyKind::Boss(BossKind::MechaHitler) => [10.0, 6.0, 8.0, 10.0, 6.0, 8.0],
        EnemyKind::Boss(BossKind::Hitler) => [6.0, 4.0, 2.0, 6.0, 4.0, 2.0],
        _ => [10.0, 3.0, 8.0, 10.0, 3.0, 8.0],
    };
    let phase = elapsed_tics.rem_euclid(durations.iter().sum());
    let mut boundary = 0.0;
    for (frame, duration) in [0u8, 0, 1, 2, 2, 3].into_iter().zip(durations) {
        boundary += duration;
        if phase < boundary {
            return frame;
        }
    }
    0
}

fn patrol_animation_frame(elapsed_seconds: f64) -> u8 {
    let phase = (elapsed_seconds * 70.0).rem_euclid(80.0);
    let mut boundary = 0.0;
    for (frame, duration) in [0u8, 0, 1, 2, 2, 3]
        .into_iter()
        .zip([20.0, 5.0, 15.0, 20.0, 5.0, 15.0])
    {
        boundary += duration;
        if phase < boundary {
            return frame;
        }
    }
    0
}

/// Matches the patrol state's thinker timings in the DOS state tables.
fn patrol_state_thinks(elapsed_seconds: f64) -> bool {
    let phase = (elapsed_seconds * 70.0).rem_euclid(80.0);
    phase < 20.0 || (25.0..60.0).contains(&phase) || phase >= 65.0
}

fn enemy_is_patrolling(enemy: &ActiveEnemy) -> bool {
    is_patrolling_mode(enemy.mode, enemy.alerting_from_patrol)
}

fn is_patrolling_mode(mode: EnemyMode, alerting_from_patrol: bool) -> bool {
    matches!(mode, EnemyMode::Patrolling)
        || (alerting_from_patrol && matches!(mode, EnemyMode::Alerting(_)))
}

fn reaction_time_after_update(remaining: f64, dt: f64, thinker_ran: bool) -> Option<f64> {
    if !thinker_ran {
        Some(remaining)
    } else if remaining <= dt {
        None
    } else {
        Some(remaining - dt)
    }
}

/// Whether the current source chase state runs its `T_Chase`/`T_DogChase`
/// thinker. The short repeated-frame states have no thinker in the DOS tables.
fn chase_state_thinks(kind: EnemyKind, elapsed_seconds: f64) -> bool {
    let tics_per_cycle = match kind {
        EnemyKind::Boss(BossKind::MechaHitler) => 48.0,
        EnemyKind::Boss(BossKind::Hitler) => 24.0,
        EnemyKind::Ghost(_) => 20.0,
        _ => 42.0,
    };
    let phase = (elapsed_seconds * 70.0).rem_euclid(tics_per_cycle);
    match kind {
        EnemyKind::Boss(BossKind::MechaHitler) => {
            phase < 10.0 || (16.0..34.0).contains(&phase) || phase >= 40.0
        }
        EnemyKind::Boss(BossKind::Hitler) => {
            phase < 6.0 || (10.0..18.0).contains(&phase) || phase >= 22.0
        }
        EnemyKind::Ghost(_) => true,
        _ => phase < 10.0 || (13.0..31.0).contains(&phase) || phase >= 34.0,
    }
}

#[derive(Clone, Copy)]
struct AttackStage {
    tics: u8,
    active: bool,
}

fn attack_stage_boundaries(
    sequence: &[AttackStage],
) -> impl Iterator<Item = (usize, f64, bool)> + '_ {
    sequence
        .iter()
        .enumerate()
        .scan(0.0, |boundary, (index, stage)| {
            *boundary += stage.tics as f64;
            Some((index, *boundary, stage.active))
        })
}

fn attack_sequence(kind: EnemyKind) -> Option<&'static [AttackStage]> {
    const GUARD: [AttackStage; 3] = [
        AttackStage {
            tics: 20,
            active: false,
        },
        AttackStage {
            tics: 20,
            active: true,
        },
        AttackStage {
            tics: 20,
            active: false,
        },
    ];
    const DOG: [AttackStage; 5] = [
        AttackStage {
            tics: 10,
            active: false,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
        AttackStage {
            tics: 10,
            active: false,
        },
        AttackStage {
            tics: 10,
            active: false,
        },
        AttackStage {
            tics: 10,
            active: false,
        },
    ];
    const OFFICER: [AttackStage; 3] = [
        AttackStage {
            tics: 6,
            active: false,
        },
        AttackStage {
            tics: 20,
            active: true,
        },
        AttackStage {
            tics: 10,
            active: false,
        },
    ];
    const MUTANT: [AttackStage; 4] = [
        AttackStage {
            tics: 6,
            active: true,
        },
        AttackStage {
            tics: 20,
            active: false,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
        AttackStage {
            tics: 20,
            active: false,
        },
    ];
    const SS: [AttackStage; 9] = [
        AttackStage {
            tics: 20,
            active: false,
        },
        AttackStage {
            tics: 20,
            active: true,
        },
        AttackStage {
            tics: 10,
            active: false,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
        AttackStage {
            tics: 10,
            active: false,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
        AttackStage {
            tics: 10,
            active: false,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
        AttackStage {
            tics: 10,
            active: false,
        },
    ];
    const BOSS: [AttackStage; 8] = [
        AttackStage {
            tics: 30,
            active: false,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
        AttackStage {
            tics: 10,
            active: false,
        },
    ];
    const SCHABBS: [AttackStage; 2] = [
        AttackStage {
            tics: 30,
            active: false,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
    ];
    const FAT: [AttackStage; 6] = [
        AttackStage {
            tics: 30,
            active: false,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
    ];
    const FAKE: [AttackStage; 9] = [
        AttackStage {
            tics: 8,
            active: true,
        },
        AttackStage {
            tics: 8,
            active: true,
        },
        AttackStage {
            tics: 8,
            active: true,
        },
        AttackStage {
            tics: 8,
            active: true,
        },
        AttackStage {
            tics: 8,
            active: true,
        },
        AttackStage {
            tics: 8,
            active: true,
        },
        AttackStage {
            tics: 8,
            active: true,
        },
        AttackStage {
            tics: 8,
            active: true,
        },
        AttackStage {
            tics: 8,
            active: false,
        },
    ];
    const HITLER: [AttackStage; 6] = [
        AttackStage {
            tics: 30,
            active: false,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
        AttackStage {
            tics: 10,
            active: true,
        },
    ];
    match kind {
        EnemyKind::Guard => Some(&GUARD),
        EnemyKind::Dog => Some(&DOG),
        EnemyKind::Officer => Some(&OFFICER),
        EnemyKind::Mutant => Some(&MUTANT),
        EnemyKind::Ss => Some(&SS),
        EnemyKind::Boss(BossKind::Hans | BossKind::Gretel) => Some(&BOSS),
        EnemyKind::Boss(BossKind::Schabbs | BossKind::Gift) => Some(&SCHABBS),
        EnemyKind::Boss(BossKind::Fat) => Some(&FAT),
        EnemyKind::Boss(BossKind::FakeHitler) => Some(&FAKE),
        EnemyKind::Boss(BossKind::MechaHitler) => Some(&HITLER),
        EnemyKind::Boss(BossKind::Hitler) => Some(&HITLER),
        _ => None,
    }
}

fn enemy_attack_sound(kind: EnemyKind) -> Option<usize> {
    match kind {
        EnemyKind::Dog => Some(68),
        EnemyKind::Ss => Some(60),
        EnemyKind::Boss(BossKind::Schabbs) => Some(8),
        EnemyKind::Boss(BossKind::Gift | BossKind::Fat) => Some(85),
        EnemyKind::Boss(BossKind::FakeHitler) => Some(69),
        EnemyKind::Boss(BossKind::MechaHitler | BossKind::Hitler | BossKind::Hans) => Some(59),
        EnemyKind::Boss(
            BossKind::GhostBlinky
            | BossKind::GhostClyde
            | BossKind::GhostPinky
            | BossKind::GhostInky,
        ) => None,
        EnemyKind::Guard
        | EnemyKind::Officer
        | EnemyKind::Mutant
        | EnemyKind::Boss(BossKind::Gretel) => Some(58),
        EnemyKind::Ghost(_) | EnemyKind::DeadGuard => None,
    }
}

fn has_pain_animation(kind: EnemyKind) -> bool {
    matches!(
        kind,
        EnemyKind::Guard | EnemyKind::Officer | EnemyKind::Mutant | EnemyKind::Ss
    )
}

fn attack_stage_index(sequence: &[AttackStage], elapsed_tics: f64) -> u8 {
    let mut boundary = 0.0;
    for (index, stage) in sequence.iter().enumerate() {
        boundary += stage.tics as f64;
        if elapsed_tics < boundary {
            return index as u8;
        }
    }
    sequence.len().saturating_sub(1) as u8
}

fn enemy_hitpoints(kind: EnemyKind, difficulty: Difficulty) -> i32 {
    let tier = match difficulty {
        Difficulty::Baby => 0,
        Difficulty::Easy => 1,
        Difficulty::Medium => 2,
        Difficulty::Hard => 3,
    };
    match kind {
        EnemyKind::Guard => 25,
        EnemyKind::Officer => 50,
        EnemyKind::Ss => 100,
        EnemyKind::Dog => 1,
        EnemyKind::Mutant => [45, 55, 55, 65][tier],
        EnemyKind::Ghost(_) => 25,
        EnemyKind::Boss(boss) => match boss {
            BossKind::Hans | BossKind::Gretel | BossKind::Gift | BossKind::Fat => {
                [850, 950, 1050, 1200][tier]
            }
            BossKind::Schabbs => [850, 950, 1550, 2400][tier],
            BossKind::FakeHitler => [200, 300, 400, 500][tier],
            BossKind::MechaHitler => [800, 950, 1050, 1200][tier],
            BossKind::Hitler => [500, 700, 800, 900][tier],
            BossKind::GhostBlinky
            | BossKind::GhostClyde
            | BossKind::GhostPinky
            | BossKind::GhostInky => 25,
        },
        EnemyKind::DeadGuard => 0,
    }
}

fn transformed_hitler_hitpoints(difficulty: Difficulty) -> i32 {
    match difficulty {
        Difficulty::Baby => 500,
        Difficulty::Easy => 700,
        Difficulty::Medium => 800,
        Difficulty::Hard => 900,
    }
}

fn attack_roll_succeeds(
    enemy: &mut ActiveEnemy,
    player: &Player,
    dt: f64,
    waiting_for_door: bool,
    random: &mut WolfRandom,
) -> bool {
    let per_tick_chance = match enemy.kind {
        EnemyKind::Boss(BossKind::Schabbs | BossKind::Gift | BossKind::Fat) => Some(8.0),
        EnemyKind::Boss(BossKind::FakeHitler) => Some(2.0),
        _ => None,
    };
    if let Some(chance_per_tick) = per_tick_chance {
        return (random.next() as f64) < (dt * 70.0 * chance_per_tick).min(256.0);
    }

    let (enemy_tile_x, enemy_tile_y) = enemy_route_tile(enemy);
    let (player_tile_x, player_tile_y) = player.actor_tile();
    let dx = (enemy_tile_x as isize - player_tile_x as isize).unsigned_abs();
    let dy = (enemy_tile_y as isize - player_tile_y as isize).unsigned_abs();
    let distance = dx.max(dy).max(1) as f64;
    let close_range = dx.max(dy) == 1 && (waiting_for_door || distance_to_next_tile(enemy) < 0.25);
    let chance = if dx.max(dy) == 0 || close_range {
        300.0
    } else {
        (dt * 70.0 * 16.0 / distance).floor()
    };
    (random.next() as f64) < chance
}

fn distance_to_next_tile(enemy: &ActiveEnemy) -> f64 {
    distance_to_next_tile_from(enemy.x, enemy.y, enemy.direction)
}

fn distance_to_next_tile_from(x: f64, y: f64, direction: u8) -> f64 {
    let (direction_x, direction_y) = direction_vector(direction);
    let next_x = if direction_x > 0.0 {
        (x + 0.5).floor() + 0.5
    } else if direction_x < 0.0 {
        (x - 0.5).ceil() - 0.5
    } else {
        x
    };
    let next_y = if direction_y > 0.0 {
        (y + 0.5).floor() + 0.5
    } else if direction_y < 0.0 {
        (y - 0.5).ceil() - 0.5
    } else {
        y
    };
    // DOS MoveObj subtracts the per-tic `move` value from `ob->distance`.
    // Diagonal movement adds that same value to both coordinates, so the
    // remaining route distance is the larger axis delta, not its Euclidean
    // length (which would scale diagonal routes by sqrt(2)).
    (next_x - x).abs().max((next_y - y).abs())
}

fn weapon_range_allows(weapon: Weapon, distance: f64) -> bool {
    // DOS GunAttack considers every visible target; only KnifeAttack has the
    // original 0x18000 (1.5 tile) distance cutoff.
    weapon != Weapon::Knife || distance <= 1.5
}

fn attack_player(
    enemy: &mut ActiveEnemy,
    player: &Player,
    difficulty: Difficulty,
    shot_clear: bool,
    player_running: bool,
    player_visible: bool,
    random: &mut WolfRandom,
) -> u16 {
    let dx = (player.x - enemy.x).abs();
    let dy = (player.y - enemy.y).abs();
    if enemy.kind == EnemyKind::Dog {
        // DOS T_Bite subtracts TILEGLOBAL (one tile) from each axis, then
        // allows MINACTORDIST (also one tile) on the remainder.
        if dx > 2.0 || dy > 2.0 || random.next() >= 180 {
            return 0;
        }
        return scale_damage((random.next() >> 4) as u16, difficulty);
    }
    if !shot_clear {
        return 0;
    }
    // DOS T_Shoot measures from ob->tilex/tiley, which TryWalk updates to the
    // reserved destination tile before the actor reaches its center.
    let (enemy_tile_x, enemy_tile_y) = enemy_route_tile(enemy);
    let (player_tile_x, player_tile_y) = player.actor_tile();
    let tile_x = enemy_tile_x.abs_diff(player_tile_x);
    let tile_y = enemy_tile_y.abs_diff(player_tile_y);
    let distance = tile_x.max(tile_y) as i32;
    let aim_distance = if matches!(enemy.kind, EnemyKind::Ss | EnemyKind::Boss(BossKind::Hans)) {
        distance * 2 / 3
    } else {
        distance
    };
    let hit_chance = if player_running {
        if player_visible {
            160 - aim_distance * 16
        } else {
            160 - aim_distance * 8
        }
    } else if player_visible {
        256 - aim_distance * 16
    } else {
        256 - aim_distance * 8
    }
    .clamp(0, 256);
    if random.next() as i32 >= hit_chance {
        return 0;
    }
    let roll = random.next();
    let damage = if aim_distance < 2 {
        roll >> 2
    } else if aim_distance < 4 {
        roll >> 3
    } else {
        roll >> 4
    };
    scale_damage(damage as u16, difficulty)
}

fn actor_is_visible_to_player(
    map: &Map,
    visible_tiles: &[bool],
    enemy: &ActiveEnemy,
    player: &Player,
) -> bool {
    let dx = enemy.x - player.x;
    let dy = enemy.y - player.y;
    let depth =
        dx * player.angle.cos() + dy * player.angle.sin() + crate::world::CAMERA_FORWARD_OFFSET
            - crate::world::ACTOR_PROJECTION_OFFSET;
    if depth < crate::world::PLAYER_RADIUS {
        return false;
    }
    let (tile_x, tile_y) = enemy_route_tile(enemy);
    let tile_x = tile_x as isize;
    let tile_y = tile_y as isize;
    (-1..=1).any(|offset_y| {
        (-1..=1).any(|offset_x| {
            let x = tile_x + offset_x;
            let y = tile_y + offset_y;
            if x < 0 || y < 0 || x >= map.width as isize || y >= map.height as isize {
                return false;
            }
            let tile_index = y as usize * map.width + x as usize;
            if !visible_tiles.get(tile_index).copied().unwrap_or(false) {
                return false;
            }
            (offset_x == 0 && offset_y == 0)
                || map.tile(x as usize, y as usize) == 0
                || map.tile(x as usize, y as usize) >= 107
        })
    })
}

fn scale_damage(damage: u16, difficulty: Difficulty) -> u16 {
    if difficulty == Difficulty::Baby {
        damage >> 2
    } else {
        damage
    }
}

fn projectile_kind(kind: EnemyKind) -> Option<ProjectileKind> {
    match kind {
        EnemyKind::Boss(BossKind::Schabbs) => Some(ProjectileKind::Needle),
        EnemyKind::Boss(BossKind::Gift | BossKind::Fat) => Some(ProjectileKind::Rocket),
        EnemyKind::Boss(BossKind::FakeHitler) => Some(ProjectileKind::Fire),
        _ => None,
    }
}

fn projectile_can_move(
    map: &Map,
    doors: &DoorSystem,
    pushwalls: &PushWallSystem,
    x: f64,
    y: f64,
) -> bool {
    const PROJECTILE_RADIUS: f64 = 0.125;
    for (check_x, check_y) in [
        (x - PROJECTILE_RADIUS, y - PROJECTILE_RADIUS),
        (x + PROJECTILE_RADIUS, y - PROJECTILE_RADIUS),
        (x - PROJECTILE_RADIUS, y + PROJECTILE_RADIUS),
        (x + PROJECTILE_RADIUS, y + PROJECTILE_RADIUS),
    ] {
        let tile_x = check_x.floor() as isize;
        let tile_y = check_y.floor() as isize;
        if tile_x < 0 || tile_y < 0 || tile_x >= map.width as isize || tile_y >= map.height as isize
        {
            return false;
        }
        let tile_x = tile_x as usize;
        let tile_y = tile_y as usize;
        let tile = map.tile(tile_x, tile_y);
        let door_blocks = doors
            .at(tile_x, tile_y)
            .is_some_and(|(_, door)| projectile_door_blocks(door.action));
        if pushwalls.blocks_reserved_tile(tile_x, tile_y)
            || pushwalls.blocks_point(check_x, check_y, 0.0)
            || static_blocks_tile(map.info_tile(tile_x, tile_y))
            || door_blocks
            || (doors.at(tile_x, tile_y).is_none() && tile != 0 && tile < 107)
        {
            return false;
        }
    }
    true
}

fn projectile_door_blocks(action: DoorAction) -> bool {
    action != DoorAction::Open
}

pub(crate) fn area_reachable_tiles(
    map: &Map,
    doors: &DoorSystem,
    pushwalls: &PushWallSystem,
    player: &Player,
) -> Vec<bool> {
    let mut reachable = vec![false; map.width.saturating_mul(map.height)];
    let (player_tile_x, player_tile_y) = player.actor_tile();
    let start_x = player_tile_x as isize;
    let start_y = player_tile_y as isize;
    if start_x < 0 || start_y < 0 || start_x >= map.width as isize || start_y >= map.height as isize
    {
        return reachable;
    }
    let start = (start_x as usize, start_y as usize);
    let mut queue = VecDeque::new();
    queue.push_back(start);
    reachable[start.1 * map.width + start.0] = true;
    while let Some((x, y)) = queue.pop_front() {
        for (dx, dy) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
            if doors
                .at(x, y)
                .is_some_and(|(_, door)| !door.connects_areas(dx, dy))
            {
                continue;
            }
            let nx = x as isize + dx;
            let ny = y as isize + dy;
            if nx < 0 || ny < 0 || nx >= map.width as isize || ny >= map.height as isize {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            let index = ny * map.width + nx;
            if reachable[index] {
                continue;
            }
            let tile = map.tile(nx, ny);
            let passable = if let Some((_, door)) = doors.at(nx, ny) {
                door.connects_areas(dx, dy)
            } else {
                let (cx, cy) = (nx as f64 + 0.5, ny as f64 + 0.5);
                (tile == 0 || tile >= 107)
                    && !pushwalls.blocks_reserved_tile(nx, ny)
                    && !pushwalls.blocks_point(cx, cy, 0.0)
            };
            if !passable {
                continue;
            }
            reachable[index] = true;
            queue.push_back((nx, ny));
        }
    }
    reachable
}

pub(crate) fn tile_is_reachable(reachable: &[bool], map: &Map, x: f64, y: f64) -> bool {
    let (tile_x, tile_y) = (x.floor() as isize, y.floor() as isize);
    tile_x >= 0
        && tile_y >= 0
        && tile_x < map.width as isize
        && tile_y < map.height as isize
        && reachable[tile_y as usize * map.width + tile_x as usize]
}

/// Mirrors the DOS `CheckSight` order: require a player-connected area, allow
/// automatic detection inside the close range, then check facing and line of
/// sight. `enemy_direction` uses the original eight-way direction numbering.
fn check_sight(
    map: &Map,
    doors: &DoorSystem,
    pushwalls: &PushWallSystem,
    connected_tiles: &[bool],
    enemy_x: f64,
    enemy_y: f64,
    enemy_direction: u8,
    player: &Player,
) -> bool {
    if !tile_is_reachable(connected_tiles, map, enemy_x, enemy_y) {
        return false;
    }
    let delta_x = player.x - enemy_x;
    let delta_y = player.y - enemy_y;
    if delta_x.abs() < 1.5 && delta_y.abs() < 1.5 {
        return true;
    }
    let facing_player = match enemy_direction {
        0 => delta_x >= 0.0,
        2 => delta_y <= 0.0,
        4 => delta_x <= 0.0,
        6 => delta_y >= 0.0,
        _ => true,
    };
    facing_player && has_line_of_sight(map, doors, pushwalls, enemy_x, enemy_y, player.x, player.y)
}

fn reaction_delay(kind: EnemyKind, random: &mut WolfRandom) -> f64 {
    let tics = match kind {
        EnemyKind::Guard => 1 + random.next() as u16 / 4,
        EnemyKind::Officer => 2,
        EnemyKind::Mutant | EnemyKind::Ss => 1 + random.next() as u16 / 6,
        EnemyKind::Dog => 1 + random.next() as u16 / 8,
        EnemyKind::Boss(_) | EnemyKind::Ghost(_) => 1,
        EnemyKind::DeadGuard => return f64::INFINITY,
    };
    tics as f64 / 70.0
}

fn first_sighting_sound(kind: EnemyKind) -> Option<usize> {
    match kind {
        EnemyKind::Guard => Some(21),
        EnemyKind::Officer => Some(66),
        EnemyKind::Ss => Some(51),
        EnemyKind::Dog => Some(41),
        EnemyKind::Boss(BossKind::Hans) => Some(55),
        EnemyKind::Boss(BossKind::Gretel) => Some(82),
        EnemyKind::Boss(BossKind::Gift) => Some(80),
        EnemyKind::Boss(BossKind::Fat) => Some(81),
        EnemyKind::Boss(BossKind::Schabbs) => Some(64),
        EnemyKind::Boss(BossKind::FakeHitler) => Some(62),
        EnemyKind::Boss(BossKind::MechaHitler) => Some(53),
        EnemyKind::Boss(BossKind::Hitler) => Some(53),
        EnemyKind::Mutant | EnemyKind::Ghost(_) | EnemyKind::DeadGuard => None,
        EnemyKind::Boss(
            BossKind::GhostBlinky
            | BossKind::GhostClyde
            | BossKind::GhostPinky
            | BossKind::GhostInky,
        ) => None,
    }
}

fn first_sighting_sound_is_local(kind: EnemyKind) -> bool {
    matches!(
        kind,
        EnemyKind::Guard | EnemyKind::Officer | EnemyKind::Ss | EnemyKind::Dog
    )
}

fn enemy_death_sound_is_local(kind: EnemyKind) -> bool {
    matches!(
        kind,
        EnemyKind::Guard | EnemyKind::Officer | EnemyKind::Mutant | EnemyKind::Ss | EnemyKind::Dog
    )
}

fn enemy_death_sound(
    kind: EnemyKind,
    map_number: usize,
    map9_death_scream_available: bool,
    random: &mut WolfRandom,
) -> Option<usize> {
    if map_number == 9
        && map9_death_scream_available
        && random.next() == 0
        && matches!(
            kind,
            EnemyKind::Mutant
                | EnemyKind::Guard
                | EnemyKind::Officer
                | EnemyKind::Ss
                | EnemyKind::Dog
        )
    {
        return Some(75);
    }
    match kind {
        EnemyKind::Guard => Some([29, 22, 25, 73, 74, 76, 77, 78][(random.next() % 8) as usize]),
        EnemyKind::Officer => Some(67),
        EnemyKind::Mutant => Some(52),
        EnemyKind::Ss => Some(56),
        EnemyKind::Dog => Some(10),
        EnemyKind::Boss(BossKind::Hans) => Some(50),
        EnemyKind::Boss(BossKind::Schabbs) => Some(63),
        EnemyKind::Boss(BossKind::FakeHitler) => Some(65),
        EnemyKind::Boss(BossKind::MechaHitler) => Some(57),
        EnemyKind::Boss(BossKind::Hitler) => Some(54),
        EnemyKind::Boss(BossKind::Gretel) => Some(83),
        EnemyKind::Boss(BossKind::Gift) => Some(79),
        EnemyKind::Boss(BossKind::Fat) => Some(84),
        EnemyKind::Boss(
            BossKind::GhostBlinky
            | BossKind::GhostClyde
            | BossKind::GhostPinky
            | BossKind::GhostInky,
        )
        | EnemyKind::Ghost(_)
        | EnemyKind::DeadGuard => None,
    }
}

fn enemy_speed(kind: EnemyKind, chasing: bool) -> f64 {
    let base = match kind {
        EnemyKind::Dog | EnemyKind::Ghost(_) => 1_500.0,
        _ => 512.0,
    } * 70.0
        / 65_536.0;
    if !chasing {
        return base;
    }
    let multiplier = match kind {
        EnemyKind::Boss(BossKind::Hitler) => 5.0,
        EnemyKind::Guard | EnemyKind::Mutant | EnemyKind::Boss(_) => 3.0,
        EnemyKind::Officer => 5.0,
        EnemyKind::Ss => 4.0,
        // DOS FirstSighting doubles the dog's SPDDOG speed before T_DogChase.
        EnemyKind::Dog => 2.0,
        // DOS SpawnGhosts assigns SPDDOG and T_Ghosts never raises it.
        EnemyKind::Ghost(_) => 1.0,
        EnemyKind::DeadGuard => 0.0,
    };
    base * multiplier
}

fn direction_vector(direction: u8) -> (f64, f64) {
    // `MoveObj` in the DOS engine applies the full movement amount to both
    // axes on diagonals, so diagonal enemy speed is √2 times cardinal speed.
    const DIRECTIONS: [(f64, f64); 8] = [
        (1.0, 0.0),
        (1.0, -1.0),
        (0.0, -1.0),
        (-1.0, -1.0),
        (-1.0, 0.0),
        (-1.0, 1.0),
        (0.0, 1.0),
        (1.0, 1.0),
    ];
    DIRECTIONS[direction as usize % DIRECTIONS.len()]
}

/// Chooses a tile direction using the original `SelectChaseDir` priority:
/// toward the player first, continue the old direction next, then search all
/// cardinal directions in one of two randomized orders before turning around.
fn select_chase_direction(
    map: &Map,
    doors: &mut DoorSystem,
    pushwalls: &PushWallSystem,
    enemy: &mut ActiveEnemy,
    player: &Player,
    random: &mut WolfRandom,
    actor_index: usize,
    other_positions: &[ActorTileClaim],
) -> Option<u8> {
    let enemy_tile_x = enemy.x.floor() as isize;
    let enemy_tile_y = enemy.y.floor() as isize;
    let (player_tile_x, player_tile_y) = player.actor_tile();
    let delta_x = player_tile_x as isize - enemy_tile_x;
    let delta_y = player_tile_y as isize - enemy_tile_y;
    let mut toward_x = match delta_x.cmp(&0) {
        std::cmp::Ordering::Greater => Some(0),
        std::cmp::Ordering::Less => Some(4),
        std::cmp::Ordering::Equal => None,
    };
    let mut toward_y = match delta_y.cmp(&0) {
        std::cmp::Ordering::Greater => Some(6),
        std::cmp::Ordering::Less => Some(2),
        std::cmp::Ordering::Equal => None,
    };
    if delta_y.unsigned_abs() > delta_x.unsigned_abs() {
        std::mem::swap(&mut toward_x, &mut toward_y);
    }

    let turnaround = if enemy.direction < 8 {
        (enemy.direction + 4) % 8
    } else {
        8
    };
    let mut candidates = Vec::with_capacity(7);
    for direction in [toward_x, toward_y].into_iter().flatten() {
        if direction != turnaround && !candidates.contains(&direction) {
            candidates.push(direction);
        }
    }
    if enemy.direction < 8
        && enemy.direction != turnaround
        && !candidates.contains(&enemy.direction)
    {
        candidates.push(enemy.direction);
    }
    if let Some(direction) = candidates.iter().copied().find(|direction| {
        chase_step_is_open(
            map,
            doors,
            pushwalls,
            enemy_tile_x,
            enemy_tile_y,
            *direction,
            enemy_can_open_doors(enemy.kind),
            actor_index,
            other_positions,
        )
    }) {
        return Some(direction);
    }

    let alternatives = if random.next() > 128 {
        [2, 3, 4]
    } else {
        [4, 3, 2]
    };
    if let Some(direction) = alternatives.into_iter().find(|direction| {
        *direction != turnaround
            && chase_step_is_open(
                map,
                doors,
                pushwalls,
                enemy_tile_x,
                enemy_tile_y,
                *direction,
                enemy_can_open_doors(enemy.kind),
                actor_index,
                other_positions,
            )
    }) {
        return Some(direction);
    }
    (turnaround < 8
        && chase_step_is_open(
            map,
            doors,
            pushwalls,
            enemy_tile_x,
            enemy_tile_y,
            turnaround,
            enemy_can_open_doors(enemy.kind),
            actor_index,
            other_positions,
        ))
    .then_some(turnaround)
}

fn chase_step_is_open(
    map: &Map,
    doors: &mut DoorSystem,
    pushwalls: &PushWallSystem,
    tile_x: isize,
    tile_y: isize,
    direction: u8,
    can_open_doors: bool,
    actor_index: usize,
    other_positions: &[ActorTileClaim],
) -> bool {
    let (dx, dy) = direction_vector(direction);
    let next_x = tile_x + dx.round() as isize;
    let next_y = tile_y + dy.round() as isize;
    let diagonal = dx != 0.0 && dy != 0.0;
    let checks = [(next_x, next_y), (next_x, tile_y), (tile_x, next_y)];
    let check_count = if diagonal { 3 } else { 1 };
    for &(x, y) in &checks[..check_count] {
        if x < 0 || y < 0 || x >= map.width as isize || y >= map.height as isize {
            return false;
        }
        let x = x as usize;
        let y = y as usize;
        if route_cell_is_occupied(x, y, actor_index, other_positions) {
            return false;
        }
        let center_x = x as f64 + 0.5;
        let center_y = y as f64 + 0.5;
        if pushwalls.blocks_reserved_tile(x, y) || pushwalls.blocks_point(center_x, center_y, 0.12)
        {
            return false;
        }
        if static_blocks_tile(map.planes[1][y * 64 + x]) {
            return false;
        }
        if let Some((_, door)) = doors.at(x, y) {
            if door.action != DoorAction::Open {
                if diagonal || !can_open_doors {
                    return false;
                }
                doors.open_for_enemy(x, y);
            }
        } else {
            let tile = map.tile(x, y);
            if tile != 0 && tile < 107 {
                return false;
            }
        }
    }
    true
}

/// Mirrors DOS `SelectDodgeDir`: favor the diagonal toward the player and the
/// two cardinal choices on that side, with randomized axis priority. Unlike
/// ordinary chase selection, it does not keep its old path direction ahead.
fn select_dodge_direction(
    map: &Map,
    doors: &mut DoorSystem,
    pushwalls: &PushWallSystem,
    enemy: &mut ActiveEnemy,
    player: &Player,
    random: &mut WolfRandom,
    actor_index: usize,
    other_positions: &[ActorTileClaim],
) -> Option<u8> {
    let enemy_tile_x = enemy.x.floor() as isize;
    let enemy_tile_y = enemy.y.floor() as isize;
    let (player_tile_x, player_tile_y) = player.actor_tile();
    let delta_x = player_tile_x as isize - enemy_tile_x;
    let delta_y = player_tile_y as isize - enemy_tile_y;
    let mut horizontal = if delta_x > 0 { 0 } else { 4 };
    let mut horizontal_opposite = if delta_x > 0 { 4 } else { 0 };
    let mut vertical = if delta_y > 0 { 6 } else { 2 };
    let mut vertical_opposite = if delta_y > 0 { 2 } else { 6 };
    if delta_x.unsigned_abs() > delta_y.unsigned_abs() {
        std::mem::swap(&mut horizontal, &mut vertical);
        std::mem::swap(&mut horizontal_opposite, &mut vertical_opposite);
    }
    if random.next() < 128 {
        std::mem::swap(&mut horizontal, &mut vertical);
        std::mem::swap(&mut horizontal_opposite, &mut vertical_opposite);
    }
    let diagonal = match (horizontal, vertical) {
        (0, 2) | (2, 0) => 1,
        (4, 2) | (2, 4) => 3,
        (4, 6) | (6, 4) => 5,
        _ => 7,
    };
    let first_attack = enemy.first_attack;
    let turnaround = if first_attack || enemy.direction >= 8 {
        8
    } else {
        (enemy.direction + 4) % 8
    };
    let excluded_direction = if first_attack {
        enemy.first_attack = false;
        None
    } else {
        Some(turnaround)
    };
    let candidates = [
        diagonal,
        horizontal,
        vertical,
        horizontal_opposite,
        vertical_opposite,
    ];
    let mut can_step = |direction| {
        chase_step_is_open(
            map,
            doors,
            pushwalls,
            enemy_tile_x,
            enemy_tile_y,
            direction,
            enemy_can_open_doors(enemy.kind),
            actor_index,
            other_positions,
        )
    };
    candidates
        .into_iter()
        .filter(|direction| Some(*direction) != excluded_direction)
        .find(|direction| can_step(*direction))
        .or_else(|| {
            (turnaround < 8 && excluded_direction != Some(turnaround) && can_step(turnaround))
                .then_some(turnaround)
        })
}

/// Mirrors `SelectRunDir` for Schabbs, Gift, and Fat when the player is within
/// four tiles: try the two cardinal directions away from the player, then the
/// original randomized north-to-west or west-to-north fallback.
fn select_run_direction(
    map: &Map,
    doors: &mut DoorSystem,
    pushwalls: &PushWallSystem,
    enemy: &ActiveEnemy,
    player: &Player,
    random: &mut WolfRandom,
    actor_index: usize,
    other_positions: &[ActorTileClaim],
) -> Option<u8> {
    let tile_x = enemy.x.floor() as isize;
    let tile_y = enemy.y.floor() as isize;
    let (player_tile_x, player_tile_y) = player.actor_tile();
    let delta_x = player_tile_x as isize - tile_x;
    let delta_y = player_tile_y as isize - tile_y;
    let mut away_x = if delta_x < 0 { 0 } else { 4 };
    let mut away_y = if delta_y < 0 { 6 } else { 2 };
    if delta_y.unsigned_abs() > delta_x.unsigned_abs() {
        std::mem::swap(&mut away_x, &mut away_y);
    }
    let can_open_doors = enemy_can_open_doors(enemy.kind);
    for direction in [away_x, away_y] {
        if chase_step_is_open(
            map,
            doors,
            pushwalls,
            tile_x,
            tile_y,
            direction,
            can_open_doors,
            actor_index,
            other_positions,
        ) {
            return Some(direction);
        }
    }
    let fallback = if random.next() > 128 {
        [2, 3, 4]
    } else {
        [4, 3, 2]
    };
    fallback.into_iter().find(|direction| {
        chase_step_is_open(
            map,
            doors,
            pushwalls,
            tile_x,
            tile_y,
            *direction,
            can_open_doors,
            actor_index,
            other_positions,
        )
    })
}

/// Checks whether an actor has reached the destination center for the route
/// selected from `(route_x, route_y)`. DOS updates `tilex/tiley` when it picks
/// a route, then selects another direction only after consuming that route's
/// distance and snapping to its destination center.
fn route_reached_tile_center(
    x: f64,
    y: f64,
    route_x: isize,
    route_y: isize,
    direction: u8,
) -> bool {
    let (target_x, target_y) = route_target_center(route_x, route_y, direction);
    let (dx, dy) = direction_vector(direction);
    let reached_x = if dx > 0.0 {
        x >= target_x
    } else if dx < 0.0 {
        x <= target_x
    } else {
        (x - target_x).abs() <= 0.08
    };
    let reached_y = if dy > 0.0 {
        y >= target_y
    } else if dy < 0.0 {
        y <= target_y
    } else {
        (y - target_y).abs() <= 0.08
    };
    reached_x && reached_y
}

fn route_target_center(route_x: isize, route_y: isize, direction: u8) -> (f64, f64) {
    let (dx, dy) = direction_vector(direction);
    (route_x as f64 + 0.5 + dx, route_y as f64 + 0.5 + dy)
}

fn clamp_route_step(current: f64, proposed: f64, target: f64) -> f64 {
    if target > current {
        proposed.min(target)
    } else if target < current {
        proposed.max(target)
    } else {
        target
    }
}

fn projectile_angle(dx: f64, north: f64) -> u16 {
    // DOS truncates launch angles to whole degrees. The tolerance preserves
    // those degrees when reconstructing them from saved sine/cosine vectors.
    (north.atan2(dx).to_degrees() + 1e-9).rem_euclid(360.0) as u16
}

fn enemy_can_move(
    map: &Map,
    doors: &mut DoorSystem,
    pushwalls: &PushWallSystem,
    x: f64,
    y: f64,
    can_open_doors: bool,
) -> bool {
    let tile_x = x.floor() as isize;
    let tile_y = y.floor() as isize;
    if tile_x < 0 || tile_y < 0 || tile_x >= map.width as isize || tile_y >= map.height as isize {
        return false;
    }
    let tile_x = tile_x as usize;
    let tile_y = tile_y as usize;
    let tile = map.tile(tile_x, tile_y);
    if pushwalls.blocks_reserved_tile(tile_x, tile_y)
        || pushwalls.blocks_point(x, y, 0.12)
        || static_blocks_tile(map.planes[1][tile_y * 64 + tile_x])
    {
        return false;
    }
    if let Some((_, door)) = doors.at(tile_x, tile_y) {
        if door.action != DoorAction::Open {
            if can_open_doors {
                doors.open_for_enemy(tile_x, tile_y);
            }
            return false;
        }
    } else if tile != 0 && tile < 107 {
        return false;
    }
    true
}

fn enemy_can_open_doors(kind: EnemyKind) -> bool {
    !matches!(kind, EnemyKind::Dog | EnemyKind::Boss(BossKind::FakeHitler))
}

fn should_use_dodge_direction(
    kind: EnemyKind,
    player_in_sight: bool,
    attack_started: bool,
) -> bool {
    matches!(kind, EnemyKind::Dog | EnemyKind::Boss(BossKind::FakeHitler))
        || (player_in_sight && !attack_started && !matches!(kind, EnemyKind::Ghost(_)))
}

fn update_actor_collision_position(
    positions: &mut Vec<ActorTileClaim>,
    index: usize,
    enemy: &ActiveEnemy,
) {
    if matches!(enemy.mode, EnemyMode::Dying { .. } | EnemyMode::Dead) {
        positions.retain(|claim| claim.index != index);
    } else if let Some(claim) = positions.iter_mut().find(|claim| claim.index == index) {
        *claim = actor_tile_claim(index, enemy);
    } else {
        positions.push(actor_tile_claim(index, enemy));
    }
}

fn actor_tile_claim(index: usize, enemy: &ActiveEnemy) -> ActorTileClaim {
    let (tile_x, tile_y) = enemy_route_tile(enemy);
    ActorTileClaim {
        index,
        tile_x,
        tile_y,
    }
}

fn enemy_route_tile(enemy: &ActiveEnemy) -> (usize, usize) {
    enemy
        .last_decision_tile
        .filter(|_| enemy.direction < 8)
        .map(|(origin_x, origin_y)| {
            let (dx, dy) = direction_vector(enemy.direction);
            (
                origin_x + dx.round() as isize,
                origin_y + dy.round() as isize,
            )
        })
        .filter(|(tile_x, tile_y)| *tile_x >= 0 && *tile_y >= 0)
        .map(|(tile_x, tile_y)| (tile_x as usize, tile_y as usize))
        .unwrap_or((enemy.x.floor() as usize, enemy.y.floor() as usize))
}

fn enemy_waiting_for_door(enemy: &ActiveEnemy, doors: &DoorSystem) -> bool {
    let Some((origin_x, origin_y)) = enemy.last_decision_tile.filter(|_| enemy.direction < 8)
    else {
        return false;
    };
    let (dx, dy) = direction_vector(enemy.direction);
    let Ok(tile_x) = usize::try_from(origin_x + dx.round() as isize) else {
        return false;
    };
    let Ok(tile_y) = usize::try_from(origin_y + dy.round() as isize) else {
        return false;
    };
    doors
        .at(tile_x, tile_y)
        .is_some_and(|(_, door)| door.action != DoorAction::Open)
}

fn route_cell_is_occupied(
    tile_x: usize,
    tile_y: usize,
    actor_index: usize,
    other_positions: &[ActorTileClaim],
) -> bool {
    other_positions
        .iter()
        .any(|claim| claim.index != actor_index && (claim.tile_x, claim.tile_y) == (tile_x, tile_y))
}

fn has_line_of_sight(
    map: &Map,
    doors: &DoorSystem,
    pushwalls: &PushWallSystem,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) -> bool {
    let dx = x1 - x0;
    let dy = y1 - y0;
    if pushwalls.blocks_point(x0, y0, 0.0)
        || pushwalls
            .ray_hit(x0, y0, dx, dy)
            .is_some_and(|hit| hit.distance < 1.0)
    {
        return false;
    }

    // CheckLine independently crosses the vertical and horizontal grid lines.
    // Its coordinates and slopes have eight fractional bits.
    let fixed = |coordinate: f64| (coordinate * 256.0).floor() as i32;
    let (x0, y0, x1, y1) = (fixed(x0), fixed(y0), fixed(x1), fixed(y1));
    for (start, across, end, across_end, horizontal) in
        [(x0, y0, x1, y1, false), (y0, x0, y1, x1, true)]
    {
        let start_tile = start >> 8;
        let end_tile = end >> 8;
        if start_tile == end_tile {
            continue;
        }
        let direction = (end_tile - start_tile).signum();
        let partial = if direction > 0 {
            256 - (start & 255)
        } else {
            start & 255
        };
        let slope = (((across_end - across) << 8) / (end - start).abs()).clamp(-0x7fff, 0x7fff);
        let mut intercept = across + ((slope * partial) >> 8);
        let mut tile = start_tile;
        while tile != end_tile {
            tile += direction;
            let other = intercept >> 8;
            let (x, y) = if horizontal {
                (other, tile)
            } else {
                (tile, other)
            };
            if x < 0 || y < 0 || x >= map.width as i32 || y >= map.height as i32 {
                return false;
            }
            let (x, y) = (x as usize, y as usize);
            let midpoint = intercept + slope - slope / 2;
            intercept += slope;
            if let Some((_, door)) = doors.at(x, y) {
                // DOS compares the unsigned fixed-point intercept to doorposition.
                if midpoint as u16 > door.position {
                    return false;
                }
            } else if (1..107).contains(&map.tile(x, y)) {
                return false;
            }
        }
    }
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PickupKind {
    Key(u8),
    Heal(u8),
    Ammo(u8),
    ExtraLife,
    Treasure(u32),
    Weapon(Weapon),
}

pub fn scan_item_pickups(map: &Map) -> Vec<ItemPickup> {
    let mut pickups = Vec::new();
    for y in 0..map.height {
        for x in 0..map.width {
            let tile = map.planes[1][y * 64 + x];
            // These statinfo entries are items in the original Wolf3D game;
            // info-plane statics start at tile 23. Values mirror GetBonus.
            let kind = match tile {
                29 => Some(PickupKind::Heal(4)), // Alpo
                43 => Some(PickupKind::Key(0)),
                44 => Some(PickupKind::Key(1)),
                47 => Some(PickupKind::Heal(10)), // Food
                48 => Some(PickupKind::Heal(25)), // First aid
                49 => Some(PickupKind::Ammo(8)),
                50 => Some(PickupKind::Weapon(Weapon::MachineGun)),
                51 => Some(PickupKind::Weapon(Weapon::Chaingun)),
                52 => Some(PickupKind::Treasure(100)),
                53 => Some(PickupKind::Treasure(500)),
                54 => Some(PickupKind::Treasure(1_000)),
                55 => Some(PickupKind::Treasure(5_000)),
                56 => Some(PickupKind::ExtraLife),
                57 | 61 => Some(PickupKind::Heal(1)), // Gibs, only useful at low health
                71 => Some(PickupKind::Ammo(4)),
                _ => None,
            };
            if let Some(kind) = kind {
                pickups.push(ItemPickup { x, y, kind });
            }
        }
    }
    pickups
}

/// Port of the Wolf3D `ScanInfoPlane` spawn rules for the original game.
pub fn scan_info_plane(map: &Map, difficulty: Difficulty) -> ScanResult {
    let mut result = ScanResult::default();
    for y in 0..map.height {
        for x in 0..map.width {
            let tile = map.planes[1][y * 64 + x];
            if tile == 0 {
                continue;
            }
            if (19..=22).contains(&tile) {
                continue;
            } // The player is initialized separately.
            if (23..=74).contains(&tile) {
                if let Some(sprite) = static_sprite_index(tile - 23) {
                    result.actors.push(spawn(
                        x,
                        y,
                        tile,
                        ActorKind::Decoration(sprite),
                        SpawnMode::Static,
                        0,
                        false,
                    ));
                }
                continue;
            }
            if tile == 98 {
                result.has_pushwall = true;
                continue;
            }
            if tile == 124 {
                result.actors.push(spawn(
                    x,
                    y,
                    tile,
                    ActorKind::Enemy(EnemyKind::DeadGuard),
                    SpawnMode::Static,
                    0,
                    false,
                ));
                continue;
            }
            if let Some(actor) = standard_enemy(tile, difficulty) {
                let (spawn_x, spawn_y) = if actor.mode == SpawnMode::Patrol {
                    match actor.direction {
                        0 => (x + 1, y),
                        1 => (x, y.saturating_sub(1)),
                        2 => (x.saturating_sub(1), y),
                        _ => (x, y + 1),
                    }
                } else {
                    (x, y)
                };
                let ambush = actor.mode == SpawnMode::Stand && map.tile(x, y) == 106;
                result.actors.push(spawn(
                    spawn_x,
                    spawn_y,
                    tile,
                    ActorKind::Enemy(actor.kind),
                    actor.mode,
                    actor.direction,
                    ambush,
                ));
                continue;
            }
            if let Some(kind) = boss(tile) {
                result.actors.push(spawn(
                    x,
                    y,
                    tile,
                    ActorKind::Enemy(kind),
                    SpawnMode::Stand,
                    boss_spawn_direction(kind),
                    true,
                ));
            }
        }
    }
    result
}

/// Replaces every DOS ambush marker with its adjacent walkable area tile after
/// spawned actors have retained their own ambush state.
pub fn clear_ambush_markers(map: &mut Map) {
    for y in 0..map.height {
        for x in 0..map.width {
            if map.tile(x, y) != 106 {
                continue;
            }
            let neighbors = [
                (x.checked_add(1), Some(y)),
                (Some(x), y.checked_sub(1)),
                (Some(x), y.checked_add(1)),
                (x.checked_sub(1), Some(y)),
            ];
            let area = neighbors
                .into_iter()
                .filter_map(|(x, y)| Some((x?, y?)))
                .filter(|(x, y)| *x < map.width && *y < map.height)
                .map(|(x, y)| map.tile(x, y))
                .filter(|tile| *tile >= 107)
                .last();
            if let Some(area) = area {
                map.planes[0][y * 64 + x] = area;
            }
        }
    }
}

fn static_sprite_index(statinfo_index: u16) -> Option<u16> {
    match statinfo_index {
        0..=47 => Some(statinfo_index),
        // In the Wolf3D (non-Spear) table, the final clip entry reuses the
        // existing clip picture instead of allocating another sprite.
        48 => Some(26),
        _ => None,
    }
}

fn spawn(
    x: usize,
    y: usize,
    source_tile: u16,
    kind: ActorKind,
    mode: SpawnMode,
    direction: u8,
    ambush: bool,
) -> ActorSpawn {
    ActorSpawn {
        x,
        y,
        source_tile,
        kind,
        mode,
        direction,
        ambush,
    }
}

struct StandardEnemy {
    kind: EnemyKind,
    mode: SpawnMode,
    direction: u8,
}

fn standard_enemy(tile: u16, difficulty: Difficulty) -> Option<StandardEnemy> {
    let groups = [
        (EnemyKind::Guard, 108, 112, 36),
        (EnemyKind::Officer, 116, 120, 36),
        (EnemyKind::Ss, 126, 130, 36),
        (EnemyKind::Dog, 134, 138, 36),
        (EnemyKind::Mutant, 216, 220, 18),
    ];
    for (kind, stand_base, patrol_base, difficulty_step) in groups {
        for mode in [SpawnMode::Stand, SpawnMode::Patrol] {
            let base = if mode == SpawnMode::Stand {
                stand_base
            } else {
                patrol_base
            };
            for (tier, minimum) in [Difficulty::Baby, Difficulty::Medium, Difficulty::Hard]
                .into_iter()
                .enumerate()
            {
                let expected = base + tier as u16 * difficulty_step;
                if (expected..expected + 4).contains(&tile) && difficulty >= minimum {
                    return Some(StandardEnemy {
                        kind,
                        mode,
                        direction: (tile - expected) as u8,
                    });
                }
            }
        }
    }
    None
}

fn boss(tile: u16) -> Option<EnemyKind> {
    let kind = match tile {
        214 => BossKind::Hans,
        197 => BossKind::Gretel,
        215 => BossKind::Gift,
        179 => BossKind::Fat,
        196 => BossKind::Schabbs,
        160 => BossKind::FakeHitler,
        178 => BossKind::MechaHitler,
        224 => BossKind::GhostBlinky,
        225 => BossKind::GhostClyde,
        226 => BossKind::GhostPinky,
        227 => BossKind::GhostInky,
        _ => return None,
    };
    Some(match kind {
        BossKind::GhostBlinky
        | BossKind::GhostClyde
        | BossKind::GhostPinky
        | BossKind::GhostInky => EnemyKind::Ghost(kind),
        _ => EnemyKind::Boss(kind),
    })
}

/// Cardinal info-plane direction used by the original Wolf3D boss spawners.
fn boss_spawn_direction(kind: EnemyKind) -> u8 {
    match kind {
        EnemyKind::Boss(BossKind::Gretel | BossKind::Gift | BossKind::FakeHitler) => 2,
        EnemyKind::Boss(_) => 6,
        EnemyKind::Ghost(_) => 0,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn death_screams_follow_original_state_timing() {
        assert_eq!(death_scream_tic(EnemyKind::Guard, 0), Some(15.0));
        assert_eq!(death_scream_tic(EnemyKind::Guard, 1), None);
        assert_eq!(death_scream_tic(EnemyKind::Officer, 0), Some(11.0));
        assert_eq!(
            death_scream_tic(EnemyKind::Boss(BossKind::Schabbs), 0),
            Some(0.0)
        );
        assert_eq!(
            death_scream_tic(EnemyKind::Boss(BossKind::Schabbs), 1),
            Some(10.0)
        );
        assert_eq!(
            death_scream_tic(EnemyKind::Boss(BossKind::Hitler), 0),
            Some(0.0)
        );
        assert_eq!(
            death_scream_tic(EnemyKind::Boss(BossKind::Hitler), 1),
            Some(1.0)
        );
    }

    #[test]
    fn mecha_hitler_morphs_after_the_third_ten_tick_death_state() {
        assert_eq!(
            death_animation_tics(EnemyKind::Boss(BossKind::MechaHitler)),
            Some((3, 10.0))
        );
        assert_eq!(MECHA_MORPH_TICS, 30.0);
    }

    #[test]
    fn enemy_shots_fire_at_the_end_of_their_original_state() {
        let mutant = attack_sequence(EnemyKind::Mutant).unwrap();
        let mutant_fires: Vec<_> = attack_stage_boundaries(mutant)
            .filter_map(|(_, boundary, fires)| fires.then_some(boundary))
            .collect();
        assert_eq!(mutant_fires.as_slice(), &[6.0, 36.0]);

        let fake = attack_sequence(EnemyKind::Boss(BossKind::FakeHitler)).unwrap();
        let fake_fires: Vec<_> = attack_stage_boundaries(fake)
            .filter_map(|(_, boundary, fires)| fires.then_some(boundary))
            .collect();
        assert_eq!(
            fake_fires.as_slice(),
            &[8.0, 16.0, 24.0, 32.0, 40.0, 48.0, 56.0, 64.0]
        );

        let fat = attack_sequence(EnemyKind::Boss(BossKind::Fat)).unwrap();
        let fat_fires: Vec<_> = attack_stage_boundaries(fat)
            .filter_map(|(_, boundary, fires)| fires.then_some(boundary))
            .collect();
        assert_eq!(fat_fires.as_slice(), &[40.0, 50.0, 60.0, 70.0, 80.0]);
    }

    fn map_with_infos(infos: &[(usize, usize, u16)]) -> Map {
        let mut map = Map::demo();
        map.planes[1].fill(0);
        for &(x, y, tile) in infos {
            map.planes[1][y * 64 + x] = tile;
        }
        map
    }

    #[test]
    fn scan_uses_wolf3d_enemy_offsets_and_direction() {
        let map = map_with_infos(&[(1, 1, 111), (2, 1, 116), (3, 1, 98), (4, 1, 23)]);
        let scan = scan_info_plane(&map, Difficulty::Medium);
        assert!(scan.has_pushwall);
        assert_eq!(scan.actors.len(), 3);
        assert!(
            scan.actors
                .iter()
                .any(|a| a.kind == ActorKind::Enemy(EnemyKind::Guard)
                    && a.mode == SpawnMode::Stand
                    && a.direction == 3)
        );
        assert!(
            scan.actors
                .iter()
                .any(|a| a.kind == ActorKind::Enemy(EnemyKind::Officer))
        );
        assert!(
            scan.actors
                .iter()
                .any(|a| a.kind == ActorKind::Decoration(0))
        );
    }

    #[test]
    fn medium_and_hard_enemy_groups_follow_selected_difficulty() {
        let map = map_with_infos(&[(1, 1, 144), (2, 1, 180), (3, 1, 234), (4, 1, 252)]);
        let easy = scan_info_plane(&map, Difficulty::Easy);
        assert!(easy.actors.is_empty());
        let medium = scan_info_plane(&map, Difficulty::Medium);
        assert_eq!(medium.actors.len(), 2);
        let hard = scan_info_plane(&map, Difficulty::Hard);
        assert_eq!(hard.actors.len(), 4);
    }

    #[test]
    fn route_checks_reject_other_enemy_tiles() {
        let positions = [
            ActorTileClaim {
                index: 2,
                tile_x: 8,
                tile_y: 9,
            },
            ActorTileClaim {
                index: 3,
                tile_x: 10,
                tile_y: 11,
            },
        ];

        assert!(route_cell_is_occupied(10, 11, 1, &positions));
        assert!(!route_cell_is_occupied(8, 9, 2, &positions));
        assert!(!route_cell_is_occupied(7, 7, 1, &positions));
    }

    #[test]
    fn patrol_alert_keeps_moving_and_counts_delay_only_on_thinker_tics() {
        assert!(is_patrolling_mode(EnemyMode::Patrolling, false));
        assert!(is_patrolling_mode(EnemyMode::Alerting(0.2), true));
        assert!(!is_patrolling_mode(EnemyMode::Alerting(0.2), false));
        assert_eq!(reaction_time_after_update(0.2, 0.1, false), Some(0.2));
        assert_eq!(reaction_time_after_update(0.2, 0.1, true), Some(0.1));
        assert_eq!(reaction_time_after_update(0.1, 0.1, true), None);
    }

    #[test]
    fn dogs_keep_dodge_routing_without_line_of_sight() {
        assert!(should_use_dodge_direction(EnemyKind::Dog, false, false));
        assert!(should_use_dodge_direction(
            EnemyKind::Boss(BossKind::FakeHitler),
            false,
            false
        ));
        assert!(!should_use_dodge_direction(EnemyKind::Guard, false, false));
        assert!(should_use_dodge_direction(EnemyKind::Guard, true, false));
        assert!(!should_use_dodge_direction(
            EnemyKind::Ghost(BossKind::GhostBlinky),
            true,
            false
        ));
    }

    #[test]
    fn diagonal_chase_distance_uses_dos_moveobj_scalar() {
        // DOS MoveObj moves both axes by `move` and subtracts only `move`
        // from ob->distance, so the remaining scalar is one axis delta.
        assert!((distance_to_next_tile_from(5.3, 4.7, 1) - 0.2).abs() < 1e-9);
        assert!((distance_to_next_tile_from(4.3, 5.3, 0) - 0.2).abs() < 1e-9);
    }

    #[test]
    fn firearms_keep_dos_unbounded_visible_target_range() {
        assert!(weapon_range_allows(Weapon::Pistol, 20.0));
        assert!(weapon_range_allows(Weapon::MachineGun, 20.0));
        assert!(weapon_range_allows(Weapon::Chaingun, 20.0));
        assert!(weapon_range_allows(Weapon::Knife, 1.5));
        assert!(!weapon_range_allows(Weapon::Knife, 1.5001));
    }

    #[test]
    fn ghosts_keep_the_dos_spddog_speed_while_chasing() {
        let ghost = EnemyKind::Ghost(BossKind::GhostBlinky);
        assert_eq!(enemy_speed(ghost, true), enemy_speed(ghost, false));
        assert!((enemy_speed(ghost, true) - 1_500.0 * 70.0 / 65_536.0).abs() < 1e-12);
    }

    #[test]
    fn projectile_door_collision_matches_dos_tile_occupancy() {
        assert!(projectile_door_blocks(DoorAction::Closed));
        assert!(projectile_door_blocks(DoorAction::Opening));
        assert!(!projectile_door_blocks(DoorAction::Open));
        assert!(projectile_door_blocks(DoorAction::Closing));
    }
}
