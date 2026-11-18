use crate::{
    actors::{ActorKind, ActorSpawn, static_blocks_tile},
    map::Map,
    world::Player,
};

const PUSHWALL_INFO: u16 = 98;
const AREA_TILE: u16 = 107;
const TICS_PER_TILE: f64 = 128.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushWallInteraction {
    NotPushwall,
    Started,
    Blocked,
    Ignored,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
struct MovingWall {
    origin_x: usize,
    origin_y: usize,
    tile: u16,
    area_tile: u16,
    direction_x: i8,
    direction_y: i8,
    area_written: bool,
    elapsed_tics: f64,
    distance: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct PushWallHit {
    pub distance: f64,
    pub side: usize,
    pub texture_x: usize,
    pub tile: u16,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PushWallSystem {
    active: Option<MovingWall>,
    pub secrets_found: u16,
}

impl PushWallSystem {
    pub fn is_valid_for_map(&self, map: &Map) -> bool {
        if self.secrets_found as usize > 64 * 64 {
            return false;
        }
        let Some(wall) = self.active else {
            return true;
        };
        let cardinal_direction =
            wall.direction_x.unsigned_abs() + wall.direction_y.unsigned_abs() == 1;
        let source_inside = inside(map, wall.origin_x as isize, wall.origin_y as isize);
        let first_x = wall.origin_x as isize + wall.direction_x as isize;
        let first_y = wall.origin_y as isize + wall.direction_y as isize;
        let elapsed_valid = wall.elapsed_tics.is_finite()
            && (0.0..2.0 * TICS_PER_TILE).contains(&wall.elapsed_tics);
        let distance_valid = wall.distance.is_finite()
            && (0.0..=2.0).contains(&wall.distance)
            && (wall.distance - (wall.elapsed_tics / TICS_PER_TILE).min(2.0)).abs() < 0.001;
        let area_state_valid = wall.area_written == (wall.elapsed_tics >= TICS_PER_TILE)
            && (107..=127).contains(&wall.area_tile);
        let next_step_valid = wall.elapsed_tics < TICS_PER_TILE
            || inside(
                map,
                first_x + wall.direction_x as isize,
                first_y + wall.direction_y as isize,
            );
        source_inside
            && cardinal_direction
            && (1..107).contains(&wall.tile)
            && elapsed_valid
            && distance_valid
            && area_state_valid
            && next_step_valid
    }

    pub fn interact(
        &mut self,
        map: &mut Map,
        player: &Player,
        statics: &[ActorSpawn],
        enemy_tiles: &[(usize, usize)],
    ) -> PushWallInteraction {
        if self.active.is_some() {
            return PushWallInteraction::NotPushwall;
        }
        let (dx, dy) = player.use_direction();
        let source_x = player.x.floor() as isize + dx as isize;
        let source_y = player.y.floor() as isize + dy as isize;
        if !inside(map, source_x, source_y) {
            return PushWallInteraction::NotPushwall;
        }
        let source_x = source_x as usize;
        let source_y = source_y as usize;
        if map.planes[1][source_y * 64 + source_x] != PUSHWALL_INFO {
            return PushWallInteraction::NotPushwall;
        }
        let tile = map.tile(source_x, source_y);
        if tile == 0 || tile >= 107 {
            return PushWallInteraction::Ignored;
        }
        let target_x = source_x as isize + dx as isize;
        let target_y = source_y as isize + dy as isize;
        if !inside(map, target_x, target_y)
            || !is_floor(map.tile(target_x as usize, target_y as usize))
            || occupies(statics, enemy_tiles, target_x as usize, target_y as usize)
        {
            return PushWallInteraction::Blocked;
        }
        let player_tile_x = player.x.floor() as usize;
        let player_tile_y = player.y.floor() as usize;
        let area_tile = map.tile(player_tile_x, player_tile_y).max(AREA_TILE);
        map.planes[1][source_y * 64 + source_x] = 0;
        self.secrets_found = self.secrets_found.saturating_add(1);
        self.active = Some(MovingWall {
            origin_x: source_x,
            origin_y: source_y,
            tile,
            area_tile,
            direction_x: dx,
            direction_y: dy,
            area_written: false,
            elapsed_tics: 0.0,
            distance: 0.0,
        });
        PushWallInteraction::Started
    }

    pub fn update(
        &mut self,
        map: &mut Map,
        dt_seconds: f64,
        statics: &[ActorSpawn],
        enemy_tiles: &[(usize, usize)],
    ) {
        let Some(mut wall) = self.active else {
            return;
        };
        wall.elapsed_tics += dt_seconds.clamp(0.0, 0.25) * 70.0;
        wall.distance = (wall.elapsed_tics / TICS_PER_TILE).min(2.0);
        if wall.elapsed_tics >= TICS_PER_TILE && !wall.area_written {
            map.planes[0][wall.origin_y * 64 + wall.origin_x] = wall.area_tile;
            wall.area_written = true;
        }
        if wall.elapsed_tics < TICS_PER_TILE {
            self.active = Some(wall);
            return;
        }
        let first_x = wall.origin_x as isize + wall.direction_x as isize;
        let first_y = wall.origin_y as isize + wall.direction_y as isize;
        if !inside(map, first_x, first_y) {
            self.finish_at(map, wall, wall.origin_x, wall.origin_y);
            return;
        }
        if wall.elapsed_tics < 2.0 * TICS_PER_TILE {
            let second_x = first_x + wall.direction_x as isize;
            let second_y = first_y + wall.direction_y as isize;
            let second_is_free = inside(map, second_x, second_y)
                && is_floor(map.tile(second_x as usize, second_y as usize))
                && !occupies(statics, enemy_tiles, second_x as usize, second_y as usize);
            if second_is_free {
                self.active = Some(wall);
            } else {
                self.finish_at(map, wall, first_x as usize, first_y as usize);
            }
            return;
        }
        self.finish_at(
            map,
            wall,
            (first_x + wall.direction_x as isize) as usize,
            (first_y + wall.direction_y as isize) as usize,
        );
    }

    fn finish_at(&mut self, map: &mut Map, wall: MovingWall, x: usize, y: usize) {
        map.planes[0][y * 64 + x] = wall.tile;
        // Moving a wall leaves the player's current connected area behind,
        // matching `player->areanumber + AREATILE` in the DOS implementation.
        map.planes[0][wall.origin_y * 64 + wall.origin_x] = wall.area_tile;
        self.active = None;
    }

    pub fn blocks_point(&self, x: f64, y: f64, radius: f64) -> bool {
        let Some(wall) = self.active else {
            return false;
        };
        let center_x = wall.origin_x as f64 + 0.5 + wall.direction_x as f64 * wall.distance;
        let center_y = wall.origin_y as f64 + 0.5 + wall.direction_y as f64 * wall.distance;
        (x - center_x).abs() <= 0.5 + radius && (y - center_y).abs() <= 0.5 + radius
    }

    /// Returns the whole map cell marked occupied in the DOS `tilemap` while
    /// the pushwall moves. Between tiles this follows the wall's new cell.
    pub fn tilemap_blocking_cell(&self) -> Option<(usize, usize)> {
        let wall = self.active?;
        let (x, y) = if wall.elapsed_tics >= TICS_PER_TILE {
            (
                wall.origin_x as isize + wall.direction_x as isize,
                wall.origin_y as isize + wall.direction_y as isize,
            )
        } else {
            (wall.origin_x as isize, wall.origin_y as isize)
        };
        Some((x as usize, y as usize))
    }

    /// DOS `PushWall` reserves both the moving wall's current cell and its
    /// complete next cell in `actorat`, even while the wall graphic is still
    /// crossing the current tile.
    pub fn blocks_reserved_tile(&self, x: usize, y: usize) -> bool {
        let Some(wall) = self.active else {
            return false;
        };
        let step = if wall.elapsed_tics >= TICS_PER_TILE {
            1isize
        } else {
            0isize
        };
        let current_x = wall.origin_x as isize + wall.direction_x as isize * step;
        let current_y = wall.origin_y as isize + wall.direction_y as isize * step;
        let next_x = current_x + wall.direction_x as isize;
        let next_y = current_y + wall.direction_y as isize;
        let (x, y) = (x as isize, y as isize);
        (x == current_x && y == current_y) || (x == next_x && y == next_y)
    }

    /// The source map cell still contains the wall tile until its first tile
    /// boundary, but rendering must use the wall's continuously shifted box.
    pub fn masks_origin_tile(&self, x: usize, y: usize) -> bool {
        self.active
            .is_some_and(|wall| wall.origin_x == x && wall.origin_y == y)
    }

    pub fn ray_hit(
        &self,
        x: f64,
        y: f64,
        direction_x: f64,
        direction_y: f64,
    ) -> Option<PushWallHit> {
        let wall = self.active?;
        let center_x = wall.origin_x as f64 + 0.5 + wall.direction_x as f64 * wall.distance;
        let center_y = wall.origin_y as f64 + 0.5 + wall.direction_y as f64 * wall.distance;
        let min_x = center_x - 0.5;
        let max_x = center_x + 0.5;
        let min_y = center_y - 0.5;
        let max_y = center_y + 0.5;
        let (near_x, far_x) = slab(x, direction_x, min_x, max_x)?;
        let (near_y, far_y) = slab(y, direction_y, min_y, max_y)?;
        let near = near_x.max(near_y);
        let far = far_x.min(far_y);
        if far < near || far <= 0.0 {
            return None;
        }
        let distance = if near > 0.0 { near } else { far };
        let (side, coordinate) = if near_x > near_y {
            (0, y + distance * direction_y)
        } else {
            (1, x + distance * direction_x)
        };
        let mut texture_x = ((coordinate - coordinate.floor()) * 64.0) as usize;
        if (side == 0 && direction_x < 0.0) || (side == 1 && direction_y > 0.0) {
            texture_x = 63 - texture_x.min(63);
        } else {
            texture_x = texture_x.min(63);
        }
        Some(PushWallHit {
            distance,
            side,
            texture_x,
            tile: wall.tile,
        })
    }
}

fn inside(map: &Map, x: isize, y: isize) -> bool {
    x >= 0 && y >= 0 && x < map.width as isize && y < map.height as isize
}

fn is_floor(tile: u16) -> bool {
    tile == 0 || tile >= AREA_TILE
}

fn occupies(statics: &[ActorSpawn], enemy_tiles: &[(usize, usize)], x: usize, y: usize) -> bool {
    statics.iter().any(|actor| {
        actor.x == x
            && actor.y == y
            && match actor.kind {
                ActorKind::Decoration(_) => static_blocks_tile(actor.source_tile),
                ActorKind::Enemy(_) => true,
            }
    }) || enemy_tiles.contains(&(x, y))
}

fn slab(origin: f64, direction: f64, min: f64, max: f64) -> Option<(f64, f64)> {
    if direction.abs() < f64::EPSILON {
        return (origin >= min && origin <= max).then_some((f64::NEG_INFINITY, f64::INFINITY));
    }
    let first = (min - origin) / direction;
    let second = (max - origin) / direction;
    Some((first.min(second), first.max(second)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_pushwall_is_distinguished_from_no_pushwall() {
        let mut map = Map::demo();
        map.planes[1][2 * 64 + 3] = PUSHWALL_INFO;
        let player = Player {
            x: 2.5,
            y: 2.5,
            angle: 0.0,
            turn_remainder: 0,
            actor_tile_override: None,
        };
        let mut pushwalls = PushWallSystem::default();
        assert_eq!(
            pushwalls.interact(&mut map, &player, &[], &[]),
            PushWallInteraction::Blocked
        );

        map.planes[0][2 * 64 + 4] = 0;
        assert_eq!(
            pushwalls.interact(&mut map, &player, &[], &[]),
            PushWallInteraction::Started
        );
    }

    #[test]
    fn pushwall_target_checks_use_reserved_live_actor_tiles() {
        assert!(!occupies(&[], &[], 4, 2));
        assert!(occupies(&[], &[(4, 2)], 4, 2));
    }

    #[test]
    fn pushwall_preserves_the_players_connected_area() {
        let mut map = Map::demo();
        map.planes[0][2 * 64 + 2] = 112;
        map.planes[0][2 * 64 + 3] = 4;
        map.planes[1][2 * 64 + 3] = PUSHWALL_INFO;
        map.planes[0][2 * 64 + 4] = 0;
        let player = Player {
            x: 2.5,
            y: 2.5,
            angle: 0.0,
            turn_remainder: 0,
            actor_tile_override: None,
        };
        let mut pushwalls = PushWallSystem::default();
        assert_eq!(
            pushwalls.interact(&mut map, &player, &[], &[]),
            PushWallInteraction::Started
        );
        assert_eq!(map.tile(3, 2), 4);
        for _ in 0..7 {
            pushwalls.update(&mut map, 0.25, &[], &[]);
        }
        assert_eq!(map.tile(3, 2), 4);
        pushwalls.update(&mut map, 0.1, &[], &[]);
        assert_eq!(map.tile(3, 2), 112);
        for _ in 0..15 {
            pushwalls.update(&mut map, 0.25, &[], &[]);
        }
        assert_eq!(map.tile(3, 2), 112);
        assert_eq!(map.tile(5, 2), 4);
    }
}
