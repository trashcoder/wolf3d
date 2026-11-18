use crate::{map::Map, world::Player};

const FIRST_VERTICAL_TILE: u16 = 90;
const FIRST_HORIZONTAL_TILE: u16 = 91;
const LAST_DOOR_TILE: u16 = 101;
const MAX_DOORS: usize = 64;
const ELEVATOR_TILE: u16 = 21;
// WL_ACT1.C uses OPENTICS=300 in the original 70 Hz simulation.
const OPEN_TICS: f64 = 300.0;
const ACTOR_RADIUS: f64 = 0x5800 as f64 / 65_536.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DoorAction {
    Closed,
    Opening,
    Open,
    Closing,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Door {
    pub x: usize,
    pub y: usize,
    pub vertical: bool,
    /// 0 is normal, 1..=4 require a matching key, 5 is the special lock type.
    pub lock: u8,
    pub action: DoorAction,
    pub position: u16,
    open_tics: f64,
    close_requested: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct DoorSoundEvent {
    pub sound: usize,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct DoorSystem {
    pub doors: Vec<Door>,
    elevators: Vec<(usize, usize)>,
}

impl DoorSystem {
    pub fn is_valid_for_map(&self, map: &Map, source: &Self) -> bool {
        self.doors.len() <= MAX_DOORS
            && self.elevators == source.elevators
            && self.doors.len() == source.doors.len()
            && self
                .doors
                .iter()
                .zip(&source.doors)
                .all(|(door, original)| {
                    door.x == original.x
                        && door.y == original.y
                        && door.vertical == original.vertical
                        && door.lock == original.lock
                        && door.x < map.width
                        && door.y < map.height
                        && door.open_tics.is_finite()
                        && door.open_tics >= 0.0
                })
    }

    pub fn from_map(map: &Map) -> Result<Self, String> {
        let mut doors = Vec::new();
        let mut elevators = Vec::new();
        for y in 0..map.height {
            for x in 0..map.width {
                let tile = map.tile(x, y);
                if tile == ELEVATOR_TILE {
                    elevators.push((x, y));
                    continue;
                }
                if !(FIRST_VERTICAL_TILE..=LAST_DOOR_TILE).contains(&tile) {
                    continue;
                }
                if doors.len() == MAX_DOORS {
                    return Err("map has more than 64 doors".into());
                }
                let vertical = tile % 2 == 0;
                let first_tile = if vertical {
                    FIRST_VERTICAL_TILE
                } else {
                    FIRST_HORIZONTAL_TILE
                };
                doors.push(Door {
                    x,
                    y,
                    vertical,
                    lock: ((tile - first_tile) / 2) as u8,
                    action: DoorAction::Closed,
                    position: 0,
                    open_tics: 0.0,
                    close_requested: false,
                });
            }
        }
        Ok(Self { doors, elevators })
    }

    pub fn interact(&mut self, map: &Map, player: &Player, keys: u8) -> Option<DoorAction> {
        let (target_x, target_y) = door_in_front(map, player)?;
        let Some(index) = self
            .doors
            .iter()
            .position(|door| door.x == target_x && door.y == target_y)
        else {
            return None;
        };
        let door = &mut self.doors[index];
        if (1..=4).contains(&door.lock) && keys & (1 << (door.lock - 1)) == 0 {
            return None;
        }
        match door.action {
            DoorAction::Closed | DoorAction::Closing => {
                door.action = DoorAction::Opening;
                door.close_requested = false;
            }
            DoorAction::Open => door.close_requested = true,
            // CloseDoor refuses while actorat still contains the door marker.
            DoorAction::Opening => {}
        }
        Some(door.action)
    }

    pub fn is_locked_in_front(&self, map: &Map, player: &Player, keys: u8) -> bool {
        let Some((x, y)) = door_in_front(map, player) else {
            return false;
        };
        self.doors
            .iter()
            .find(|door| door.x == x && door.y == y)
            .is_some_and(|door| (1..=4).contains(&door.lock) && keys & (1 << (door.lock - 1)) == 0)
    }

    pub fn open_for_enemy(&mut self, x: usize, y: usize) -> bool {
        let Some(door) = self
            .doors
            .iter_mut()
            .find(|door| door.x == x && door.y == y)
        else {
            return false;
        };
        match door.action {
            DoorAction::Closed | DoorAction::Closing => {
                door.action = DoorAction::Opening;
                door.close_requested = false;
            }
            DoorAction::Open => door.open_tics = 0.0,
            DoorAction::Opening => {}
        }
        true
    }

    pub fn update(&mut self, dt_seconds: f64, player: &Player, actors: &[(f64, f64)]) {
        let _ = self.update_with_sounds(dt_seconds, player, actors);
    }

    pub fn update_with_sounds(
        &mut self,
        dt_seconds: f64,
        player: &Player,
        actors: &[(f64, f64)],
    ) -> Vec<DoorSoundEvent> {
        let tics = dt_seconds.clamp(0.0, 0.25) * 70.0;
        let mut sounds = Vec::new();
        for door in &mut self.doors {
            match door.action {
                DoorAction::Closed => {}
                DoorAction::Opening => {
                    door.close_requested = false;
                    if door.position == 0 && tics > 0.0 {
                        sounds.push(DoorSoundEvent {
                            sound: 18,
                            x: door.x as f64 + 0.5,
                            y: door.y as f64 + 0.5,
                        });
                    }
                    door.position =
                        (door.position as f64 + tics * 1024.0).min(u16::MAX as f64) as u16;
                    if door.position == u16::MAX {
                        door.action = DoorAction::Open;
                        door.open_tics = 0.0;
                    }
                }
                DoorAction::Open => {
                    door.open_tics += tics;
                    if (door.close_requested || door.open_tics >= OPEN_TICS)
                        && !door_is_occupied(*door, player, actors)
                    {
                        door.action = DoorAction::Closing;
                        door.close_requested = false;
                        sounds.push(DoorSoundEvent {
                            sound: 19,
                            x: door.x as f64 + 0.5,
                            y: door.y as f64 + 0.5,
                        });
                    }
                    door.close_requested = false;
                }
                DoorAction::Closing => {
                    if door_is_occupied(*door, player, actors) {
                        door.action = DoorAction::Opening;
                        continue;
                    }
                    door.position = (door.position as f64 - tics * 1024.0).max(0.0) as u16;
                    if door.position == 0 {
                        door.action = DoorAction::Closed;
                    }
                }
            }
        }
        sounds
    }

    pub fn point_passable(&self, x: f64, y: f64) -> bool {
        let (tile_x, tile_y) = (x.floor() as usize, y.floor() as usize);
        self.point_passable_at(tile_x, tile_y, x, y, 0.12)
    }

    pub fn point_passable_at(
        &self,
        tile_x: usize,
        tile_y: usize,
        x: f64,
        y: f64,
        clearance: f64,
    ) -> bool {
        let Some((_, door)) = self.at(tile_x, tile_y) else {
            return false;
        };
        if door.action == DoorAction::Open {
            return true;
        }
        let progress = door.position as f64 / u16::MAX as f64;
        let offset = if door.vertical {
            y - tile_y as f64
        } else {
            x - tile_x as f64
        };
        offset < progress - clearance
    }

    pub fn ray_passes(&self, index: usize, x: f64, y: f64) -> bool {
        let Some(door) = self.doors.get(index) else {
            return false;
        };
        door.gap_contains(if door.vertical { y } else { x }, 0.0)
    }

    pub fn at(&self, x: usize, y: usize) -> Option<(usize, &Door)> {
        self.doors
            .iter()
            .enumerate()
            .find(|(_, door)| door.x == x && door.y == y)
    }

    /// Returns whether the elevator in front of the player leads to the secret level.
    /// Original Cmd_Use only accepts east/west-facing elevator use and selects
    /// the secret route when the floor under the player is the alternate area tile.
    pub fn use_elevator(&self, map: &mut Map, player: &Player) -> Option<bool> {
        if player.use_direction().0 == 0 {
            return None;
        }
        let (target_x, target_y) = door_in_front(map, player)?;
        if !self.elevators.contains(&(target_x, target_y))
            || map.tile(target_x, target_y) != ELEVATOR_TILE
        {
            return None;
        }
        // DOS increments the wall tile to display the activated switch.
        map.set_tile(target_x, target_y, ELEVATOR_TILE + 1);
        Some(map.tile(player.x.floor() as usize, player.y.floor() as usize) == 107)
    }

    pub fn texture_page(door: &Door, sprite_start: usize, side: usize) -> usize {
        let offset = match door.lock {
            1..=4 => 6,
            5 => 4,
            _ => 0,
        };
        sprite_start.saturating_sub(8) + offset + usize::from(side == 0)
    }
}

fn door_in_front(map: &Map, player: &Player) -> Option<(usize, usize)> {
    let (dx, dy) = player.use_direction();
    let target_x = player.x.floor() as isize + isize::from(dx);
    let target_y = player.y.floor() as isize + isize::from(dy);
    (target_x >= 0
        && target_y >= 0
        && target_x < map.width as isize
        && target_y < map.height as isize)
        .then_some((target_x as usize, target_y as usize))
}

fn door_is_occupied(door: Door, player: &Player, actors: &[(f64, f64)]) -> bool {
    let overlaps = |x: f64, y: f64| {
        x + ACTOR_RADIUS > door.x as f64
            && x - ACTOR_RADIUS < door.x as f64 + 1.0
            && y + ACTOR_RADIUS > door.y as f64
            && y - ACTOR_RADIUS < door.y as f64 + 1.0
    };
    overlaps(player.x, player.y) || actors.iter().any(|&(x, y)| overlaps(x, y))
}

impl Door {
    pub(crate) fn connects_areas(&self, dx: isize, dy: isize) -> bool {
        // DoorOpening links the areas on the first movement tic. DoorClosing
        // keeps them linked until the door reaches its closed position.
        self.position > 0 && if self.vertical { dx != 0 } else { dy != 0 }
    }

    fn gap_contains(&self, along: f64, clearance: f64) -> bool {
        if self.action == DoorAction::Open {
            return true;
        }
        let progress = self.position as f64 / u16::MAX as f64;
        let offset = along - along.floor();
        offset < progress - clearance
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_orientation_and_lock_type_from_original_tile_codes() {
        let mut map = Map::demo();
        map.planes[0][2 * 64 + 3] = 92;
        map.planes[0][3 * 64 + 3] = 95;
        let system = DoorSystem::from_map(&map).unwrap();
        assert_eq!(system.doors.len(), 2);
        assert_eq!((system.doors[0].vertical, system.doors[0].lock), (true, 1));
        assert_eq!((system.doors[1].vertical, system.doors[1].lock), (false, 2));
    }

    #[test]
    fn locked_doors_need_matching_key_and_open_on_interaction() {
        let mut map = Map::demo();
        map.planes[0][2 * 64 + 3] = 92;
        let mut system = DoorSystem::from_map(&map).unwrap();
        let player = Player {
            x: 2.5,
            y: 2.5,
            angle: 0.0,
            turn_remainder: 0,
            actor_tile_override: None,
        };
        assert_eq!(system.interact(&map, &player, 0), None);
        assert_eq!(system.doors[0].action, DoorAction::Closed);
        assert_eq!(system.interact(&map, &player, 1), Some(DoorAction::Opening));
        for _ in 0..20 {
            system.update(0.05, &player, &[]);
        }
        assert!(system.point_passable(3.5, 2.5));
    }

    #[test]
    fn an_open_door_closes_after_three_game_seconds() {
        let door = Door {
            x: 3,
            y: 2,
            vertical: true,
            lock: 0,
            action: DoorAction::Open,
            position: u16::MAX,
            open_tics: 0.0,
            close_requested: false,
        };
        let mut system = DoorSystem {
            doors: vec![door],
            elevators: Vec::new(),
        };
        let player = Player {
            x: 2.5,
            y: 2.5,
            angle: 0.0,
            turn_remainder: 0,
            actor_tile_override: None,
        };
        for _ in 0..63 {
            system.update(0.05, &player, &[]);
        }
        assert_eq!(system.doors[0].action, DoorAction::Closing);
        assert_eq!(system.doors[0].position, u16::MAX);
    }

    #[test]
    fn a_door_reopens_instead_of_closing_on_an_enemy() {
        let door = Door {
            x: 3,
            y: 2,
            vertical: true,
            lock: 0,
            action: DoorAction::Closing,
            position: u16::MAX,
            open_tics: 0.0,
            close_requested: false,
        };
        let mut system = DoorSystem {
            doors: vec![door],
            elevators: Vec::new(),
        };
        let player = Player {
            x: 2.5,
            y: 2.5,
            angle: 0.0,
            turn_remainder: 0,
            actor_tile_override: None,
        };
        system.update(0.05, &player, &[(2.8, 2.5)]);
        assert_eq!(system.doors[0].action, DoorAction::Opening);
        assert_eq!(system.doors[0].position, u16::MAX);
    }

    #[test]
    fn using_an_opening_door_keeps_it_opening_even_when_unoccupied() {
        let mut map = Map::demo();
        map.planes[0][2 * 64 + 3] = 90;
        let mut system = DoorSystem {
            doors: vec![Door {
                x: 3,
                y: 2,
                vertical: true,
                lock: 0,
                action: DoorAction::Opening,
                position: u16::MAX / 2,
                open_tics: 0.0,
                close_requested: false,
            }],
            elevators: Vec::new(),
        };
        let player = Player {
            x: 2.5,
            y: 2.5,
            angle: 0.0,
            turn_remainder: 0,
            actor_tile_override: None,
        };

        system.interact(&map, &player, 0);
        let sounds = system.update_with_sounds(0.05, &player, &[]);

        assert_eq!(system.doors[0].action, DoorAction::Opening);
        assert_eq!(system.doors[0].position, u16::MAX / 2 + 3_584);
        assert!(sounds.is_empty());
    }

    #[test]
    fn using_an_opening_door_does_not_reverse_it_while_occupied() {
        let mut map = Map::demo();
        map.planes[0][2 * 64 + 3] = 90;
        let mut system = DoorSystem {
            doors: vec![Door {
                x: 3,
                y: 2,
                vertical: true,
                lock: 0,
                action: DoorAction::Opening,
                position: u16::MAX / 2,
                open_tics: 0.0,
                close_requested: false,
            }],
            elevators: Vec::new(),
        };
        let player = Player {
            x: 2.5,
            y: 2.5,
            angle: 0.0,
            turn_remainder: 0,
            actor_tile_override: None,
        };

        system.interact(&map, &player, 0);
        system.update(0.05, &player, &[(3.5, 2.5)]);

        assert_eq!(system.doors[0].action, DoorAction::Opening);
        assert_eq!(system.doors[0].position, u16::MAX / 2 + 3_584);
    }

    #[test]
    fn a_partly_open_door_has_a_passable_gap_on_its_sliding_axis() {
        let mut map = Map::demo();
        map.planes[0][2 * 64 + 3] = 90;
        let system = DoorSystem {
            doors: vec![Door {
                x: 3,
                y: 2,
                vertical: true,
                lock: 0,
                action: DoorAction::Opening,
                position: u16::MAX / 2,
                open_tics: 0.0,
                close_requested: false,
            }],
            elevators: Vec::new(),
        };
        assert!(system.point_passable(3.5, 2.2));
        assert!(!system.point_passable(3.5, 2.8));
    }
}
