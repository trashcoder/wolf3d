use std::{collections::HashMap, error::Error, fmt};

use crate::{
    actors::{ActorKind, ActorSpawn, BossKind, EnemyKind},
    vswap::Vswap,
};

#[derive(Clone, Debug)]
pub struct Post {
    pub start: usize,
    pub end: usize,
    pub pixels: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct Sprite {
    pub left: usize,
    pub right: usize,
    pub columns: Vec<Vec<Post>>,
}

#[derive(Debug)]
struct SpriteError(String);

impl fmt::Display for SpriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for SpriteError {}

#[derive(Default)]
pub struct SpriteAtlas {
    shapes: HashMap<usize, Sprite>,
}

impl SpriteAtlas {
    pub fn load(vswap: &Vswap, actors: &[ActorSpawn]) -> Result<Self, Box<dyn Error>> {
        Self::load_with_victory(vswap, actors, false)
    }

    pub fn load_with_victory(
        vswap: &Vswap,
        actors: &[ActorSpawn],
        include_bj_victory: bool,
    ) -> Result<Self, Box<dyn Error>> {
        let mut atlas = Self::default();
        for actor in actors {
            for shape in actor_shape_pages(actor.kind) {
                if atlas.shapes.contains_key(&shape) {
                    continue;
                }
                let page = vswap
                    .sprite_page(shape)
                    .ok_or_else(|| SpriteError(format!("missing VSWAP sprite page {shape}")))?;
                atlas.shapes.insert(shape, Sprite::decode(page)?);
            }
        }
        for shape in 416..436 {
            if atlas.shapes.contains_key(&shape) {
                continue;
            }
            let page = vswap.sprite_page(shape).ok_or_else(|| {
                SpriteError(format!("missing VSWAP player weapon sprite page {shape}"))
            })?;
            atlas.shapes.insert(shape, Sprite::decode(page)?);
        }
        if actors.iter().any(|actor| {
            matches!(
                actor.kind,
                ActorKind::Enemy(EnemyKind::Boss(BossKind::Gift | BossKind::Fat))
            )
        }) {
            for shape in 370..378 {
                if atlas.shapes.contains_key(&shape) {
                    continue;
                }
                let page = vswap.sprite_page(shape).ok_or_else(|| {
                    SpriteError(format!("missing VSWAP projectile sprite page {shape}"))
                })?;
                atlas.shapes.insert(shape, Sprite::decode(page)?);
            }
            for shape in 382..385 {
                if atlas.shapes.contains_key(&shape) {
                    continue;
                }
                let page = vswap.sprite_page(shape).ok_or_else(|| {
                    SpriteError(format!("missing VSWAP explosion sprite page {shape}"))
                })?;
                atlas.shapes.insert(shape, Sprite::decode(page)?);
            }
        }
        if actors.iter().any(
            |actor| matches!(actor.kind, ActorKind::Enemy(enemy) if enemy != EnemyKind::DeadGuard),
        ) && !atlas.shapes.contains_key(&95)
        {
            let page = vswap
                .sprite_page(95)
                .ok_or_else(|| SpriteError("missing VSWAP sprite page 95".into()))?;
            atlas.shapes.insert(95, Sprite::decode(page)?);
        }
        if include_bj_victory {
            for shape in 408..416 {
                let page = vswap.sprite_page(shape).ok_or_else(|| {
                    SpriteError(format!("missing BJ victory sprite page {shape}"))
                })?;
                atlas.shapes.insert(shape, Sprite::decode(page)?);
            }
        }
        Ok(atlas)
    }

    pub fn sprite(&self, shape: usize) -> Option<&Sprite> {
        self.shapes.get(&shape)
    }
}

pub fn shape_for_actor(
    kind: ActorKind,
    facing_degrees: u16,
    player_angle: f64,
    screen_offset: i32,
    animation_frame: u8,
    moving: bool,
    dying: bool,
    attacking: bool,
    pain: bool,
) -> Option<usize> {
    if dying {
        return death_sprite(kind, animation_frame);
    }
    if attacking {
        return attack_sprite(kind, animation_frame);
    }
    if pain {
        let base = pain_sprite(kind, animation_frame)?;
        let angle = sprite_view_angle(facing_degrees, player_angle, screen_offset);
        // DOS rotate == 2 adds four pages when viewed from the opposite half.
        return Some(base + 4 * (angle / 180) as usize);
    }
    let (base, rotates) = match kind {
        // Wolf3D's rocket has eight view-angle sprite pages, like a rotating actor.
        ActorKind::Decoration(368) => (370, true),
        ActorKind::Decoration(index) => (index as usize + 2, false),
        ActorKind::Enemy(enemy) => match enemy {
            EnemyKind::Guard => (
                if moving {
                    58 + animation_frame as usize * 8
                } else {
                    50
                },
                true,
            ),
            EnemyKind::Dog => (99 + animation_frame as usize * 8, true),
            EnemyKind::Ss => (
                if moving {
                    146 + animation_frame as usize * 8
                } else {
                    138
                },
                true,
            ),
            EnemyKind::Mutant => (
                if moving {
                    195 + animation_frame as usize * 8
                } else {
                    187
                },
                true,
            ),
            EnemyKind::Officer => (
                if moving {
                    246 + animation_frame as usize * 8
                } else {
                    238
                },
                true,
            ),
            EnemyKind::DeadGuard => (95, false),
            EnemyKind::Ghost(kind) => (
                match kind {
                    BossKind::GhostBlinky => 288,
                    BossKind::GhostPinky => 290,
                    BossKind::GhostClyde => 292,
                    BossKind::GhostInky => 294,
                    _ => return None,
                },
                false,
            ),
            EnemyKind::Boss(kind) => (
                match kind {
                    BossKind::Hans => 296,
                    BossKind::Schabbs => 307,
                    BossKind::FakeHitler => 321,
                    BossKind::MechaHitler => 334,
                    BossKind::Hitler => 345,
                    BossKind::Gift => 360,
                    BossKind::Gretel => 385,
                    BossKind::Fat => 396,
                    _ => return None,
                },
                false,
            ),
        },
    };
    if !rotates {
        let frame = if moving {
            match kind {
                ActorKind::Enemy(EnemyKind::Ghost(_)) => (animation_frame % 2) as usize,
                _ => animation_frame as usize,
            }
        } else {
            0
        };
        return Some(base + frame);
    }
    let angle = sprite_view_angle(facing_degrees, player_angle, screen_offset);
    Some(base + (angle / 45) as usize)
}

fn sprite_view_angle(facing_degrees: u16, player_angle: f64, screen_offset: i32) -> i32 {
    // DOS angles increase toward north, and CalcRotate uses integer division.
    let player_degrees = (-player_angle.to_degrees().round()).rem_euclid(360.0) as i32;
    let view_degrees = player_degrees - screen_offset / 8;
    (view_degrees - 180 - i32::from(facing_degrees) + 360 / 16).rem_euclid(360)
}

fn pain_sprite(kind: ActorKind, frame: u8) -> Option<usize> {
    let ActorKind::Enemy(enemy) = kind else {
        return None;
    };
    let sequence = match enemy {
        EnemyKind::Guard => [90, 94],
        EnemyKind::Officer => [278, 282],
        EnemyKind::Mutant => [227, 231],
        EnemyKind::Ss => [178, 182],
        _ => return None,
    };
    sequence.get(frame as usize).copied()
}

fn attack_sprite(kind: ActorKind, frame: u8) -> Option<usize> {
    let ActorKind::Enemy(enemy) = kind else {
        return None;
    };
    let sequence: &[usize] = match enemy {
        EnemyKind::Guard => &[96, 97, 98],
        EnemyKind::Dog => &[135, 136, 137, 135, 99],
        EnemyKind::Officer => &[285, 286, 287],
        EnemyKind::Mutant => &[234, 235, 236, 237],
        EnemyKind::Ss => &[184, 185, 186, 185, 186, 185, 186, 185, 186],
        EnemyKind::Boss(BossKind::Hans) => &[300, 301, 302, 301, 302, 301, 302, 300],
        EnemyKind::Boss(BossKind::Gretel) => &[389, 390, 391, 390, 391, 390, 391, 389],
        EnemyKind::Boss(BossKind::Schabbs) => &[311, 312],
        EnemyKind::Boss(BossKind::Gift) => &[364, 365],
        EnemyKind::Boss(BossKind::Fat) => &[400, 401, 402, 403, 402, 403],
        EnemyKind::Boss(BossKind::FakeHitler) => &[325, 325, 325, 325, 325, 325, 325, 325, 325],
        EnemyKind::Boss(BossKind::MechaHitler) => &[338, 339, 340, 339, 340, 339],
        EnemyKind::Boss(BossKind::Hitler) => &[349, 350, 351, 350, 351, 350],
        _ => return None,
    };
    sequence.get(frame as usize).copied()
}

fn death_sprite(kind: ActorKind, frame: u8) -> Option<usize> {
    let ActorKind::Enemy(enemy) = kind else {
        return None;
    };
    let (sequence, corpse) = match enemy {
        EnemyKind::Guard => (&[91, 92, 93][..], 95),
        EnemyKind::Dog => (&[131, 132, 133][..], 134),
        EnemyKind::Ss => (&[179, 180, 181][..], 183),
        EnemyKind::Mutant => (&[228, 229, 230, 232][..], 233),
        EnemyKind::Officer => (&[279, 280, 281, 283][..], 284),
        EnemyKind::Boss(BossKind::Hans) => (&[304, 305, 306][..], 303),
        EnemyKind::Boss(BossKind::Schabbs) => (&[307, 307, 313, 314, 315][..], 316),
        EnemyKind::Boss(BossKind::FakeHitler) => (&[328, 329, 330, 331, 332][..], 333),
        EnemyKind::Boss(BossKind::MechaHitler) => (&[342, 343, 344][..], 341),
        EnemyKind::Boss(BossKind::Hitler) => {
            (&[345, 345, 353, 354, 355, 356, 357, 358, 359][..], 352)
        }
        EnemyKind::Boss(BossKind::Gift) => (&[360, 360, 366, 367, 368][..], 369),
        EnemyKind::Boss(BossKind::Gretel) => (&[393, 394, 395][..], 392),
        EnemyKind::Boss(BossKind::Fat) => (&[396, 396, 404, 405, 406][..], 407),
        _ => return None,
    };
    Some(sequence.get(frame as usize).copied().unwrap_or(corpse))
}

fn actor_shape_pages(kind: ActorKind) -> Vec<usize> {
    match kind {
        ActorKind::Decoration(index) => vec![index as usize + 2],
        ActorKind::Enemy(enemy) => {
            let range = match enemy {
                EnemyKind::Guard => 50..99,
                EnemyKind::Dog => 99..138,
                EnemyKind::Ss => 138..187,
                EnemyKind::Mutant => 187..238,
                EnemyKind::Officer => 238..288,
                EnemyKind::DeadGuard => 95..96,
                EnemyKind::Ghost(kind) => {
                    let base = match kind {
                        BossKind::GhostBlinky => 288,
                        BossKind::GhostPinky => 290,
                        BossKind::GhostClyde => 292,
                        BossKind::GhostInky => 294,
                        _ => return Vec::new(),
                    };
                    base..base + 2
                }
                EnemyKind::Boss(kind) => match kind {
                    BossKind::Hans => 296..307,
                    BossKind::Schabbs => 307..321,
                    BossKind::FakeHitler => 321..334,
                    BossKind::MechaHitler => 334..345,
                    BossKind::Hitler => 345..360,
                    BossKind::Gift => 360..374,
                    BossKind::Gretel => 385..396,
                    BossKind::Fat => 396..408,
                    _ => return Vec::new(),
                },
            };
            range.collect()
        }
    }
}

impl Sprite {
    /// Decodes the original 64-column compiled sprite page and its transparent posts.
    pub fn decode(page: &[u8]) -> Result<Self, SpriteError> {
        const HEADER_BYTES: usize = 4;
        if page.len() < HEADER_BYTES {
            return Err(SpriteError(
                "sprite page is shorter than its column directory".into(),
            ));
        }
        let left = word(page, 0)? as usize;
        let right = word(page, 2)? as usize;
        if left > right || right >= 64 {
            return Err(SpriteError(
                "sprite page has invalid visible column bounds".into(),
            ));
        }
        let directory_end = HEADER_BYTES + (right - left + 1) * 2;
        let mut columns = vec![Vec::new(); 64];
        for (column, posts) in columns.iter_mut().enumerate().take(right + 1).skip(left) {
            let mut cursor = word(page, HEADER_BYTES + (column - left) * 2)? as usize;
            if cursor == 0 {
                continue;
            }
            if cursor < directory_end || cursor >= page.len() {
                return Err(SpriteError(format!(
                    "sprite column {column} has an invalid offset"
                )));
            }
            loop {
                let encoded_end = word(page, cursor)? as usize;
                cursor += 2;
                if encoded_end == 0 {
                    break;
                }
                let end = encoded_end >> 1;
                let pixel_offset = word(page, cursor)? as i16 as isize;
                cursor += 2;
                let start = (word(page, cursor)? as usize) >> 1;
                cursor += 2;
                if start >= end || end > 64 {
                    return Err(SpriteError(format!(
                        "sprite column {column} contains an invalid post"
                    )));
                }
                let mut pixels = Vec::with_capacity(end - start);
                for y in start..end {
                    let source = (pixel_offset + y as isize) as usize;
                    pixels.push(*page.get(source).ok_or_else(|| {
                        SpriteError(format!(
                            "sprite post in column {column} extends past the page"
                        ))
                    })?);
                }
                posts.push(Post { start, end, pixels });
                if cursor + 2 > page.len() {
                    return Err(SpriteError(format!(
                        "sprite column {column} has no post terminator"
                    )));
                }
            }
        }
        Ok(Self {
            left,
            right,
            columns,
        })
    }
}

fn word(bytes: &[u8], offset: usize) -> Result<u16, SpriteError> {
    let pair = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| SpriteError("truncated sprite page".into()))?;
    Ok(u16::from_le_bytes([pair[0], pair[1]]))
}
