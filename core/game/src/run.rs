//! The data of the roguelite layer (`Meta`, `Run`, `Unit`, `Enemy`, `Offer`) and the life of a
//! run: its start, the enemy of each floor, the camp actions, and its end. This module mirrors
//! `army.ts`, `floors.ts`, `offers.ts`, `run.ts`, and `upgrades.ts` in `src/game/`.

use std::collections::BTreeMap;

use crate::chess::{Kind, Square};
use crate::content::{
    self, ARMY_MAX, DRAFT_GOLD_WEIGHT, DRAFT_RELIC_WEIGHT, FLOORS, RECRUIT_KINDS, RECRUITS, RELIC_PRICE, REROLL_COST,
    RelicId, UpgradeEffect, UpgradeId, WIN_CROWNS,
};
use crate::protocol::{Code, Fail, fail};
use crate::random::Dice;

/// The id of a unit of the army. Units have the ids from 1 to `UNIT_ID_MAX`.
pub type UnitId = u16;
pub const UNIT_ID_MAX: UnitId = 19_999;
/// The id of the pawn that Conscription adds. It is not a unit of the army.
pub const CONSCRIPT_ID: u16 = 20_000;
/// The enemy piece with index `i` in `Enemy::pieces` has the id `ENEMY_ID_BASE + i`.
pub const ENEMY_ID_BASE: u16 = 30_000;
/// The most pieces in an enemy army.
pub const ENEMY_PIECES_MAX: usize = 64;

/// A piece of the player army. `home` is its start square on the first two ranks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Unit {
    pub id: UnitId,
    pub kind: Kind,
    pub home: Square,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EnemyPiece {
    pub kind: Kind,
    pub square: Square,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Enemy {
    pub pieces: Vec<EnemyPiece>,
    pub traits: Vec<RelicId>,
}

/// One item of a reward draft or of the shop.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Offer {
    /// A recruit. The kind is never the king.
    Piece(Kind),
    Relic(RelicId),
    Gold(u64),
}

/// The reason that the run cannot take an offer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Blocked {
    ArmyFull,
    Owned,
}

impl Blocked {
    pub const fn code(self) -> &'static str {
        match self {
            Blocked::ArmyFull => "army_full",
            Blocked::Owned => "owned",
        }
    }
}

impl Offer {
    /// The name and the text of the offer (`describeOffer`). The text is None when the name tells all.
    pub fn describe(self) -> (String, Option<&'static str>) {
        match self {
            Offer::Piece(kind) => (content::piece_name(kind).to_string(), None),
            Offer::Relic(id) => (id.def().name.to_string(), Some(id.def().text)),
            Offer::Gold(amount) => (format!("{amount} gold"), None),
        }
    }

    pub fn blocked(self, run: &Run) -> Option<Blocked> {
        match self {
            Offer::Piece(_) => (!run.can_add_unit()).then_some(Blocked::ArmyFull),
            Offer::Relic(id) => run.relics.contains(&id).then_some(Blocked::Owned),
            Offer::Gold(_) => None,
        }
    }

    /// The shop price in gold before upgrades.
    pub fn base_price(self) -> u64 {
        match self {
            Offer::Piece(kind) => content::piece_price(kind),
            Offer::Relic(_) => RELIC_PRICE,
            Offer::Gold(_) => 0,
        }
    }

    /// Gives the offer to the run. Returns false and changes nothing if the offer is blocked.
    pub fn take(self, run: &mut Run) -> bool {
        if self.blocked(run).is_some() {
            return false;
        }
        match self {
            Offer::Piece(kind) => {
                run.add_unit(kind);
            }
            Offer::Relic(id) => run.relics.push(id),
            Offer::Gold(amount) => run.gold = run.gold.saturating_add(amount),
        }
        true
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Battle,
    Camp,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Run {
    /// The floor of the current or the next battle, from 1 to `FLOORS.len()`.
    pub floor: usize,
    pub gold: u64,
    pub army: Vec<Unit>,
    pub next_id: UnitId,
    pub relics: Vec<RelicId>,
    pub enemy: Enemy,
    pub phase: Phase,
    /// The reward choices. None when the player has no reward to take.
    pub draft: Option<Vec<Offer>>,
    pub shop: Vec<Offer>,
}

/// The data that stays from one run to the next run.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Meta {
    pub crowns: u64,
    pub best: u64,
    pub runs: u64,
    /// The level of each upgrade that the player has. A level is more than 0.
    pub upgrades: BTreeMap<UpgradeId, u64>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RunSummary {
    pub won: bool,
    pub cleared: u64,
    /// The crowns for the win of the run.
    pub bonus: u64,
    /// All the crowns of the run.
    pub crowns: u64,
    /// True if the run cleared more floors than each run before it.
    pub new_best: bool,
}

// ---- Upgrades ----

impl Meta {
    pub fn level(&self, id: UpgradeId) -> u64 {
        self.upgrades.get(&id).copied().unwrap_or(0)
    }

    /// The upgrades that the player has, with their levels, in the order of `UPGRADES`.
    pub fn owned(&self) -> impl Iterator<Item = (UpgradeId, u64)> + '_ {
        UpgradeId::all().map(|id| (id, self.level(id))).filter(|&(_, level)| level > 0)
    }

    pub fn can_scout(&self) -> bool {
        self.owned().any(|(id, _)| id.def().effect == UpgradeEffect::Scout)
    }

    /// The crown cost of the next level, or None at the maximum level.
    pub fn next_cost(&self, id: UpgradeId) -> Option<u64> {
        usize::try_from(self.level(id)).ok().and_then(|level| id.def().costs.get(level).copied())
    }

    pub fn buy_upgrade(&mut self, id: UpgradeId) -> Result<(), Fail> {
        let Some(cost) = self.next_cost(id) else { return fail(Code::MaxLevel, "The upgrade has its maximum level") };
        if self.crowns < cost {
            return fail(Code::NotAffordable, format!("The next level costs {cost} crowns"));
        }
        self.crowns -= cost;
        self.upgrades.insert(id, self.level(id) + 1);
        Ok(())
    }

    /// Sets the level of an upgrade at no cost. The level stays between 0 and the maximum level.
    pub fn set_level(&mut self, id: UpgradeId, level: u64) {
        let level = level.min(id.max_level());
        if level > 0 {
            self.upgrades.insert(id, level);
        } else {
            self.upgrades.remove(&id);
        }
    }

    /// The shop price of an offer (`priceOf`).
    pub fn price_of(&self, offer: Offer) -> u64 {
        let factor = self.owned().fold(1.0, |f, (id, level)| match id.def().effect {
            UpgradeEffect::PriceCut { per_level } => f * (1.0 - per_level * level as f64),
            _ => f,
        });
        // Math.max(1, Math.round(price * factor)).
        let price = (offer.base_price() as f64 * factor + 0.5).floor();
        if price.is_nan() || price < 1.0 { 1 } else { price as u64 }
    }

    /// Adds the result of a run to the permanent data (`finishRun`).
    pub fn finish_run(&mut self, run: &Run, won: bool) -> RunSummary {
        let cleared = if won { FLOORS.len() as u64 } else { run.floor as u64 - 1 };
        let bonus = if won { WIN_CROWNS } else { 0 };
        let crowns = cleared + bonus;
        let new_best = cleared > self.best;
        self.crowns = self.crowns.saturating_add(crowns);
        self.best = self.best.max(cleared);
        self.runs = self.runs.saturating_add(1);
        RunSummary { won, cleared, bonus, crowns, new_best }
    }
}

// ---- Army ----

/// Home squares on the first two ranks, from the center to the edge.
const BACK_HOMES: [Square; 8] = [3, 2, 5, 1, 6, 0, 7, 4];
const FRONT_HOMES: [Square; 8] = [12, 11, 13, 10, 14, 9, 15, 8];

pub fn base_army() -> Vec<Unit> {
    let army = [
        (Kind::King, 4),
        (Kind::Rook, 0),
        (Kind::Knight, 6),
        (Kind::Pawn, 10),
        (Kind::Pawn, 11),
        (Kind::Pawn, 12),
        (Kind::Pawn, 13),
    ];
    army.iter().enumerate().map(|(i, &(kind, home))| Unit { id: i as UnitId + 1, kind, home }).collect()
}

/// A free home square for a new piece, or None if the first two ranks are full.
pub fn free_home(army: &[Unit], kind: Kind) -> Option<Square> {
    let (first, second) = if kind == Kind::Pawn { (FRONT_HOMES, BACK_HOMES) } else { (BACK_HOMES, FRONT_HOMES) };
    first.into_iter().chain(second).find(|&s| !army.iter().any(|unit| unit.home == s))
}

impl Run {
    pub fn new(meta: &Meta, dice: &mut Dice, barred: &[RelicId]) -> Run {
        let army = base_army();
        let mut run = Run {
            floor: 1,
            gold: 0,
            next_id: army.len() as UnitId + 1,
            army,
            relics: Vec::new(),
            enemy: generate_enemy(1, dice, barred),
            phase: Phase::Battle,
            draft: None,
            shop: Vec::new(),
        };
        for (id, level) in meta.owned() {
            match id.def().effect {
                UpgradeEffect::RecruitEachLevel(kind) => {
                    // A saved level can be larger than the maximum level. The army fills first.
                    for _ in 0..level {
                        if run.add_unit(kind).is_none() {
                            break;
                        }
                    }
                }
                UpgradeEffect::Recruit(kind) => {
                    run.add_unit(kind);
                }
                UpgradeEffect::GoldEachLevel(gold) => run.gold = run.gold.saturating_add(gold.saturating_mul(level)),
                UpgradeEffect::PriceCut { .. } | UpgradeEffect::Scout => {}
            }
        }
        run
    }

    pub fn floor_def(&self) -> &'static content::FloorDef {
        content::floor_def(self.floor)
    }

    pub fn can_add_unit(&self) -> bool {
        self.army.len() < ARMY_MAX && self.next_id <= UNIT_ID_MAX
    }

    /// Adds a unit on a free home square. Returns its id, or None if the army is full.
    pub fn add_unit(&mut self, kind: Kind) -> Option<UnitId> {
        if !self.can_add_unit() {
            return None;
        }
        let home = free_home(&self.army, kind)?;
        let id = self.next_id;
        self.next_id += 1;
        self.army.push(Unit { id, kind, home });
        Some(id)
    }

    /// Moves the unit on one home square to a second home square. If the second square has a
    /// unit, the two units swap. Returns the id of the unit that swapped.
    pub fn move_unit(&mut self, from: Square, to: Square) -> Option<UnitId> {
        let unit = self.army.iter().position(|u| u.home == from)?;
        let occupant = self.army.iter().position(|u| u.home == to);
        if let Some(i) = occupant {
            self.army[i].home = from;
        }
        self.army[unit].home = to;
        occupant.filter(|&i| i != unit).map(|i| self.army[i].id)
    }

    /// Moves the run to the camp before its next floor.
    ///
    /// A draw on the last floor stays on the last floor.
    pub fn enter_camp(&mut self, with_draft: bool, dice: &mut Dice, barred: &[RelicId]) {
        self.floor = (self.floor + 1).min(FLOORS.len());
        self.enemy = generate_enemy(self.floor, dice, barred);
        self.draft = if with_draft { Some(roll_draft(self, dice, barred)) } else { None };
        self.shop = roll_shop(self, dice, barred);
        self.phase = Phase::Camp;
    }

    pub fn take_draft(&mut self, index: usize) -> Result<Offer, Fail> {
        let Some(draft) = &self.draft else { return fail(Code::RewardClosed, "The camp has no reward to take") };
        let Some(&offer) = draft.get(index) else {
            return fail(Code::BadIndex, format!("The reward has no card {index}"));
        };
        if let Some(blocked) = offer.blocked(self) {
            return fail(Code::Blocked, format!("The run cannot take this reward: {}", blocked.code()));
        }
        offer.take(self);
        self.draft = None;
        Ok(offer)
    }

    pub fn buy_offer(&mut self, meta: &Meta, index: usize) -> Result<Offer, Fail> {
        let Some(&offer) = self.shop.get(index) else {
            return fail(Code::BadIndex, format!("The shop has no item {index}"));
        };
        let cost = meta.price_of(offer);
        if let Some(blocked) = offer.blocked(self) {
            return fail(Code::Blocked, format!("The run cannot take this item: {}", blocked.code()));
        }
        if self.gold < cost {
            return fail(Code::NotAffordable, format!("The item costs {cost} gold"));
        }
        offer.take(self);
        self.gold -= cost;
        self.shop.remove(index);
        Ok(offer)
    }

    pub fn reroll_shop(&mut self, dice: &mut Dice, barred: &[RelicId]) -> Result<(), Fail> {
        if self.gold < REROLL_COST {
            return fail(Code::NotAffordable, format!("New items cost {REROLL_COST} gold"));
        }
        self.gold -= REROLL_COST;
        self.shop = roll_shop(self, dice, barred);
        Ok(())
    }
}

// ---- Enemies and offers ----

/// The relics that the game can offer as a reward or in the shop.
pub fn relic_pool(barred: &[RelicId]) -> Vec<RelicId> {
    RelicId::all().filter(|id| !barred.contains(id)).collect()
}

/// The relics that the game can give to a boss as a trait.
pub fn trait_pool(barred: &[RelicId]) -> Vec<RelicId> {
    relic_pool(barred).into_iter().filter(|id| id.is_trait()).collect()
}

pub fn generate_enemy(floor: usize, dice: &mut Dice, barred: &[RelicId]) -> Enemy {
    let spec = content::floor_def(floor);
    let mut counts = [0u32; 5];
    let mut budget = spec.budget;
    loop {
        let pool: Vec<(usize, f64)> = RECRUIT_KINDS
            .iter()
            .enumerate()
            .filter(|&(i, &kind)| counts[i] < content::enemy_cap(kind, floor) && content::gold_value(kind) <= budget)
            .map(|(i, &kind)| (i, content::enemy_weight(kind)))
            .collect();
        let Some(&i) = dice.pick_weighted(pool, 1).first() else { break };
        counts[i] += 1;
        budget -= content::gold_value(RECRUIT_KINDS[i]);
    }
    let squares = |kind: Kind, dice: &mut Dice| -> Vec<Square> {
        match kind {
            Kind::Rook => dice.shuffle(vec![56, 63]),
            Kind::Knight => dice.shuffle(vec![57, 62]),
            Kind::Bishop => dice.shuffle(vec![58, 61]),
            Kind::Queen => vec![59],
            _ => vec![52, 51, 53, 50, 54, 49, 55, 48],
        }
    };
    // The TypeScript game shuffles the squares of all kinds before it places the pieces.
    let homes: Vec<Vec<Square>> = [Kind::Rook, Kind::Knight, Kind::Bishop, Kind::Queen, Kind::Pawn]
        .into_iter()
        .map(|kind| squares(kind, dice))
        .collect();
    let homes_of = |kind: Kind| match kind {
        Kind::Rook => &homes[0],
        Kind::Knight => &homes[1],
        Kind::Bishop => &homes[2],
        Kind::Queen => &homes[3],
        _ => &homes[4],
    };
    let mut pieces = vec![EnemyPiece { kind: Kind::King, square: 60 }];
    for (i, &kind) in RECRUIT_KINDS.iter().enumerate() {
        for n in 0..counts[i] as usize {
            pieces.push(EnemyPiece { kind, square: homes_of(kind)[n] });
        }
    }
    let mut traits = dice.shuffle(trait_pool(barred));
    traits.truncate(spec.traits);
    Enemy { pieces, traits }
}

fn piece_pool(floor: usize) -> Vec<(Offer, f64)> {
    RECRUITS.iter().filter(|r| floor >= r.min_floor).map(|r| (Offer::Piece(r.kind), r.weight)).collect()
}

fn new_relics(run: &Run, n: usize, dice: &mut Dice, barred: &[RelicId]) -> Vec<Offer> {
    let pool: Vec<RelicId> = relic_pool(barred).into_iter().filter(|id| !run.relics.contains(id)).collect();
    dice.shuffle(pool).into_iter().take(n).map(Offer::Relic).collect()
}

/// The three free rewards after a win. `run.floor` is the floor that comes next.
pub fn roll_draft(run: &Run, dice: &mut Dice, barred: &[RelicId]) -> Vec<Offer> {
    let mut pool = piece_pool(run.floor);
    pool.extend(new_relics(run, 2, dice, barred).into_iter().map(|offer| (offer, DRAFT_RELIC_WEIGHT)));
    pool.push((Offer::Gold(content::draft_gold(run.floor)), DRAFT_GOLD_WEIGHT));
    dice.pick_weighted(pool, 3)
}

pub fn roll_shop(run: &Run, dice: &mut Dice, barred: &[RelicId]) -> Vec<Offer> {
    let mut shop = dice.pick_weighted(piece_pool(run.floor), 2);
    shop.extend(new_relics(run, 2, dice, barred));
    shop
}
