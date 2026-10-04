//! The tuning: the debug settings of a session. It has the fixed seed of new runs, the barred
//! relics, and the numbers that make the enemy army and the AI level of each floor. The defaults
//! come from the content (`FLOORS` and `ENEMY_KINDS`). The tuning is not saved.

use crate::chess::{self, Kind};
use crate::content::{ENEMY_KINDS, FLOORS, RECRUIT_KINDS, RELIC_SLOTS, RelicId, TRAITS_MAX};

/// The largest budget of a floor: the value of a full army (8 pawns, 2 knights, 2 bishops,
/// 2 rooks, and 1 queen).
pub const BUDGET_MAX: u32 = 39;
/// The largest weight of a kind.
pub const WEIGHT_MAX: f64 = 9.0;

/// The largest cap of a kind: its default cap, the number of home squares of the kind.
pub fn cap_max(kind: Kind) -> u32 {
    ENEMY_KINDS.iter().find(|k| k.kind == kind).map_or(0, |k| k.cap)
}

/// The numbers of one floor: the level of the AI (1 to `chess::LEVELS`), the total piece value
/// of the enemy army, and the number of boss traits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FloorTuning {
    pub level: usize,
    pub budget: u32,
    pub traits: usize,
}

/// The numbers of one kind in an enemy army: the most pieces of the kind, its weight, and the
/// first floor that has it. A kind with a cap of 0 or a weight of 0 is not in an army.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct KindTuning {
    pub cap: u32,
    pub weight: f64,
    pub min_floor: usize,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Tuning {
    /// The seed of each new run. None: the dice of the session make the seed.
    pub seed: Option<u64>,
    /// The relics that the game does not offer and does not give to a boss, sorted.
    pub barred: Vec<RelicId>,
    /// The relic slots of each new run, from 0 to `RELICS_MAX`.
    pub relic_slots: usize,
    pub floors: [FloorTuning; FLOORS.len()],
    /// In the order of `RECRUIT_KINDS`.
    pub kinds: [KindTuning; ENEMY_KINDS.len()],
}

/// A number of the tuning that `debug_tune` changes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    Level,
    Budget,
    Traits,
    Cap,
    Weight,
    MinFloor,
}

/// The owner of the fields of one `debug_tune`: a floor (1 to 8) or a kind of `RECRUIT_KINDS`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    Floor(usize),
    Kind(Kind),
}

impl Target {
    /// The name in the protocol.
    pub const fn name(self) -> &'static str {
        match self {
            Target::Floor(_) => "floor",
            Target::Kind(_) => "kind",
        }
    }

    pub const fn fields(self) -> &'static [Field] {
        match self {
            Target::Floor(_) => &[Field::Level, Field::Budget, Field::Traits],
            Target::Kind(_) => &[Field::Cap, Field::Weight, Field::MinFloor],
        }
    }
}

impl Field {
    /// The name in the protocol.
    pub const fn name(self) -> &'static str {
        match self {
            Field::Level => "level",
            Field::Budget => "budget",
            Field::Traits => "traits",
            Field::Cap => "cap",
            Field::Weight => "weight",
            Field::MinFloor => "min_floor",
        }
    }

    /// The smallest and the largest value. `index` is the index of the kind in `RECRUIT_KINDS`:
    /// only the limit of `cap` reads it.
    fn limits(self, index: usize) -> (f64, f64) {
        match self {
            Field::Level => (1.0, chess::LEVELS as f64),
            Field::Budget => (0.0, BUDGET_MAX as f64),
            Field::Traits => (0.0, TRAITS_MAX as f64),
            Field::Cap => (0.0, ENEMY_KINDS[index].cap as f64),
            Field::Weight => (0.0, WEIGHT_MAX),
            Field::MinFloor => (1.0, FLOORS.len() as f64),
        }
    }

    /// True if the value is a whole number.
    const fn whole(self) -> bool {
        !matches!(self, Field::Weight)
    }
}

impl Default for Tuning {
    fn default() -> Tuning {
        Tuning {
            seed: None,
            barred: Vec::new(),
            relic_slots: RELIC_SLOTS,
            floors: std::array::from_fn(|i| FloorTuning {
                level: i + 1,
                budget: FLOORS[i].budget,
                traits: FLOORS[i].traits,
            }),
            kinds: ENEMY_KINDS.map(|k| KindTuning { cap: k.cap, weight: k.weight, min_floor: k.min_floor }),
        }
    }
}

impl Tuning {
    /// The numbers of a floor from 1 to `FLOORS.len()`.
    pub fn floor(&self, floor: usize) -> &FloorTuning {
        &self.floors[floor.clamp(1, self.floors.len()) - 1]
    }

    /// True if a floor or a kind differs from the default.
    pub fn tuned(&self) -> bool {
        let default = Tuning::default();
        self.floors != default.floors || self.kinds != default.kinds
    }

    /// True if the two tunings give the same enemy armies: the floors differ only by the level.
    pub fn same_armies(&self, other: &Tuning) -> bool {
        let army = |floor: &FloorTuning| (floor.budget, floor.traits);
        self.kinds == other.kinds && self.floors.iter().map(army).eq(other.floors.iter().map(army))
    }

    /// Sets each floor and each kind to the default. The seed and the barred relics stay.
    pub fn reset(&mut self) {
        let Tuning { floors, kinds, .. } = Tuning::default();
        (self.floors, self.kinds) = (floors, kinds);
    }

    /// Adds a relic to the barred relics, or removes it.
    pub fn bar(&mut self, id: RelicId, barred: bool) {
        self.barred.retain(|&other| other != id);
        if barred {
            self.barred.push(id);
            self.barred.sort();
        }
    }

    /// Sets fields of one floor or of one kind. The function checks the target and each value
    /// before it sets one, thus an error changes nothing.
    pub fn tune(&mut self, target: Target, values: &[(Field, f64)]) -> Result<(), String> {
        let index = match target {
            Target::Floor(floor) => (1..=self.floors.len()).contains(&floor).then(|| floor - 1),
            Target::Kind(kind) => RECRUIT_KINDS.iter().position(|&k| k == kind),
        };
        let Some(index) = index else { return Err(format!("The tuning has no such {}", target.name())) };
        if values.is_empty() {
            let names: Vec<String> = target.fields().iter().map(|f| format!("\"{}\"", f.name())).collect();
            return Err(format!("Give one or more of {}", names.join(", ")));
        }
        for &(field, value) in values {
            if !target.fields().contains(&field) {
                return Err(format!("\"{}\" is not a number of a {}", field.name(), target.name()));
            }
            let (min, max) = field.limits(index);
            if !(min..=max).contains(&value) || (field.whole() && value.fract() != 0.0) {
                let number = if field.whole() { "a whole number" } else { "a number" };
                return Err(format!("\"{}\" must be {number} from {min} to {max}", field.name()));
            }
        }
        for &(field, value) in values {
            match field {
                Field::Level => self.floors[index].level = value as usize,
                Field::Budget => self.floors[index].budget = value as u32,
                Field::Traits => self.floors[index].traits = value as usize,
                Field::Cap => self.kinds[index].cap = value as u32,
                Field::Weight => self.kinds[index].weight = value,
                Field::MinFloor => self.kinds[index].min_floor = value as usize,
            }
        }
        Ok(())
    }
}
