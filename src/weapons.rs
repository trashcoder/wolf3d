#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Weapon {
    Knife,
    Pistol,
    MachineGun,
    Chaingun,
}

impl Weapon {
    pub fn from_slot(slot: usize) -> Option<Self> {
        match slot {
            1 => Some(Self::Knife),
            2 => Some(Self::Pistol),
            3 => Some(Self::MachineGun),
            4 => Some(Self::Chaingun),
            _ => None,
        }
    }

    pub fn attack_sound(self) -> usize {
        match self {
            Self::Knife => 23,
            Self::Pistol => 24,
            Self::MachineGun => 26,
            Self::Chaingun => 11,
        }
    }

    pub fn uses_ammo(self) -> bool {
        self != Self::Knife
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Knife => "Knife",
            Self::Pistol => "Pistol",
            Self::MachineGun => "Machine gun",
            Self::Chaingun => "Chaingun",
        }
    }

    pub fn slot(self) -> usize {
        match self {
            Self::Knife => 0,
            Self::Pistol => 1,
            Self::MachineGun => 2,
            Self::Chaingun => 3,
        }
    }

    pub fn sprite_base(self) -> usize {
        match self {
            Self::Knife => 416,
            Self::Pistol => 421,
            Self::MachineGun => 426,
            Self::Chaingun => 431,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Arsenal {
    ammo: u8,
    selected: Weapon,
    chosen: Weapon,
    best: Weapon,
}

impl Default for Arsenal {
    fn default() -> Self {
        Self {
            ammo: 8,
            selected: Weapon::Pistol,
            chosen: Weapon::Pistol,
            best: Weapon::Pistol,
        }
    }
}

impl Arsenal {
    pub fn matches(&self, other: &Self) -> bool {
        self.ammo == other.ammo
            && self.selected == other.selected
            && self.chosen == other.chosen
            && self.best == other.best
    }

    pub fn is_valid(&self) -> bool {
        self.ammo <= 99 && self.selected <= self.best && self.chosen <= self.best
    }

    pub fn ammo(&self) -> u8 {
        self.ammo
    }

    pub fn selected(&self) -> Weapon {
        self.selected
    }

    pub fn best(&self) -> Weapon {
        self.best
    }

    pub fn select(&mut self, weapon: Weapon) -> bool {
        if self.ammo == 0 || weapon > self.best {
            return false;
        }
        self.selected = weapon;
        self.chosen = weapon;
        true
    }

    pub fn grant_weapon(&mut self, weapon: Weapon, attack_frame_zero: bool) {
        self.add_ammo(6, attack_frame_zero);
        if weapon > self.best {
            self.best = weapon;
            self.selected = weapon;
            self.chosen = weapon;
        }
    }

    pub fn add_ammo(&mut self, amount: u8, attack_frame_zero: bool) {
        let was_empty = self.ammo == 0;
        self.ammo = self.ammo.saturating_add(amount).min(99);
        if was_empty && self.ammo > 0 && attack_frame_zero {
            self.selected = self.chosen;
        }
    }

    /// Selects the weapon to use for a new attack, falling back to the knife.
    pub fn begin_attack(&mut self) -> Weapon {
        if self.selected.uses_ammo() && self.ammo == 0 {
            self.selected = Weapon::Knife;
        }
        self.selected
    }

    fn consume_ammo(&mut self) -> bool {
        if self.ammo == 0 {
            return false;
        }
        self.ammo -= 1;
        true
    }

    fn finish_attack(&mut self) {
        if self.selected.uses_ammo() && self.ammo == 0 {
            self.selected = Weapon::Knife;
        } else if self.ammo > 0 && self.selected != self.chosen {
            self.selected = self.chosen;
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
struct ActiveAttack {
    frame: u8,
    remaining: f64,
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct WeaponAttack {
    active: Option<ActiveAttack>,
}

impl WeaponAttack {
    pub fn is_valid(self) -> bool {
        self.active.is_none_or(|attack| {
            attack.frame <= 3
                && attack.remaining.is_finite()
                && attack.remaining > 0.0
                && attack.remaining <= 6.0 / 70.0
        })
    }

    pub fn is_idle(self) -> bool {
        self.active.is_none()
    }

    pub fn is_initial_attack_frame(self) -> bool {
        self.active.is_none_or(|attack| attack.frame == 0)
    }

    /// The original attack table displays frames 1 through 4 while active.
    pub fn display_frame(self) -> u8 {
        self.active.map_or(0, |attack| attack.frame + 1)
    }

    /// Advances the original 6-tic weapon animation and returns every shot
    /// crossed by this time step. Automatic weapons repeat only while held.
    pub fn update(
        &mut self,
        arsenal: &mut Arsenal,
        fire_held: bool,
        fire_started: bool,
        dt_seconds: f64,
    ) -> Vec<Weapon> {
        let starts_attack = self.active.is_none() && fire_started;
        let mut attack = match self.active {
            Some(attack) => attack,
            None if fire_started => {
                arsenal.begin_attack();
                ActiveAttack {
                    frame: 0,
                    remaining: 6.0 / 70.0,
                }
            }
            None => return Vec::new(),
        };
        if starts_attack {
            self.active = Some(attack);
            return Vec::new();
        }
        let frame_tics = 6.0 / 70.0;
        let mut time_left = dt_seconds.clamp(0.0, 0.25);
        let mut fired = Vec::new();
        loop {
            if time_left + 1e-12 < attack.remaining {
                attack.remaining -= time_left;
                self.active = Some(attack);
                break;
            }
            time_left = (time_left - attack.remaining).max(0.0);
            attack.remaining = frame_tics;
            let mut finished = false;
            // T_Attack reads gamestate.weapon when each action expires.
            let weapon = arsenal.selected();
            match (weapon, attack.frame) {
                (Weapon::Knife, 1) => {
                    fired.push(Weapon::Knife);
                    attack.frame = 2;
                }
                (Weapon::Pistol | Weapon::MachineGun | Weapon::Chaingun, 1) => {
                    if arsenal.consume_ammo() {
                        fired.push(weapon);
                        attack.frame = 2;
                    } else {
                        attack.frame = 3;
                    }
                }
                (Weapon::MachineGun, 2) if fire_held && arsenal.ammo() > 0 => {
                    attack.frame = 1;
                }
                (Weapon::Chaingun, 2) if arsenal.consume_ammo() => {
                    fired.push(Weapon::Chaingun);
                    attack.frame = if fire_held { 1 } else { 3 };
                }
                (Weapon::MachineGun | Weapon::Chaingun, 2) => attack.frame = 3,
                (_, 3) => {
                    arsenal.finish_attack();
                    finished = true;
                }
                _ => attack.frame += 1,
            }
            if finished {
                self.active = None;
                break;
            }
            if time_left <= 0.0 {
                self.active = Some(attack);
                break;
            }
        }
        fired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holding_fire_does_not_repeat_the_pistol() {
        let mut arsenal = Arsenal::default();
        let mut attack = WeaponAttack::default();
        let mut shots = 0;
        for tick in 0..50 {
            shots += attack
                .update(&mut arsenal, true, tick == 0, 1.0 / 70.0)
                .len();
        }
        assert_eq!(shots, 1);
        assert_eq!(arsenal.ammo(), 7);
    }

    #[test]
    fn holding_fire_repeats_automatic_weapons() {
        let mut arsenal = Arsenal::default();
        arsenal.grant_weapon(Weapon::MachineGun, true);
        let mut attack = WeaponAttack::default();
        let mut shots = 0;
        for tick in 0..50 {
            shots += attack
                .update(&mut arsenal, true, tick == 0, 1.0 / 70.0)
                .len();
        }
        assert!(shots >= 3);
    }
}
