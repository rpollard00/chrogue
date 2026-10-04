//! The data of the roguelite layer (`Meta`, `Run`, `Unit`, `Enemy`, `Offer`) and the life of a
//! run: its start, the enemy of each floor, the camp actions, and its end.

use std::collections::BTreeMap;

use crate::chess::{Kind, Square};
use crate::content::{
    self, ARMY_MAX, DRAFT_GOLD_WEIGHT, DRAFT_RELIC_WEIGHT, FLOORS, RECRUIT_KINDS, RECRUITS, RELIC_PRICE, REROLL_COST,
    RelicId, UpgradeEffect, UpgradeId, WIN_CROWNS,
};
use crate::protocol::{Code, Fail, fail};
use crate::random::{Dice, Stream};
use crate::tuning::Tuning;

/// The id of a unit of the army. Units have the ids from 1 to `UNIT_ID_MAX`.
pub type UnitId = u16;
pub const UNIT_ID_MAX: UnitId = 19_999;
/// The id of the pawn that Conscription adds. It is not a unit of the army.
pub const CONSCRIPT_ID: u16 = 20_000;
/// The enemy piece with index `i` in the army has the id `ENEMY_ID_BASE + i`.
pub const ENEMY_ID_BASE: u16 = 30_000;
/// The most pieces in an enemy army.
pub const ENEMY_PIECES_MAX: usize = 64;
/// The largest seed of a run. A person can type such a number, and Lua keeps it exactly.
pub const SEED_MAX: u64 = 999_999_999;

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

/// The pieces of an enemy army.
#[derive(Clone, PartialEq, Debug)]
pub enum EnemyPieces {
    /// The kinds of the army of a floor, the king first. The pieces have no squares: the battle
    /// gives each piece its square against the army of the player (`formation`).
    Kinds(Vec<Kind>),
    /// Pieces on set squares: the army of `debug_set_enemy`, and the army of saved data that has
    /// squares.
    Placed(Vec<EnemyPiece>),
}

impl EnemyPieces {
    /// The kinds of the army, in the order of its pieces.
    pub fn kinds(&self) -> Vec<Kind> {
        match self {
            EnemyPieces::Kinds(kinds) => kinds.clone(),
            EnemyPieces::Placed(pieces) => pieces.iter().map(|piece| piece.kind).collect(),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Enemy {
    pub pieces: EnemyPieces,
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
    /// Each relic slot of the run has a relic.
    RelicsFull,
}

impl Blocked {
    pub const fn code(self) -> &'static str {
        match self {
            Blocked::ArmyFull => "army_full",
            Blocked::Owned => "owned",
            Blocked::RelicsFull => "relics_full",
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
            Offer::Relic(id) if run.relics.contains(&id) => Some(Blocked::Owned),
            Offer::Relic(_) => (!run.can_add_relic()).then_some(Blocked::RelicsFull),
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
    /// The seed of the random numbers of the run, from 0 to `SEED_MAX`.
    pub seed: u64,
    /// The floor of the current or the next battle, from 1 to `FLOORS.len()`.
    pub floor: usize,
    pub gold: u64,
    pub army: Vec<Unit>,
    pub next_id: UnitId,
    pub relics: Vec<RelicId>,
    /// The relic slots of the run, from 0 to `RELICS_MAX`. A debug session can give the run
    /// more relics than slots.
    pub slots: usize,
    pub enemy: Enemy,
    pub phase: Phase,
    /// The reward choices. None when the player has no reward to take.
    pub draft: Option<Vec<Offer>>,
    pub shop: Vec<Offer>,
    /// The number of shop rerolls in this camp visit.
    pub rolls: u32,
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

    fn has(&self, effect: UpgradeEffect) -> bool {
        self.owned().any(|(id, _)| id.def().effect == effect)
    }

    pub fn can_scout(&self) -> bool {
        self.has(UpgradeEffect::Scout)
    }

    /// True if a draw gives a reward.
    pub fn draw_gives_reward(&self) -> bool {
        self.has(UpgradeEffect::DrawReward)
    }

    /// True if a unit of a reward comes with a pawn.
    pub fn reward_gives_pawn(&self) -> bool {
        self.has(UpgradeEffect::RewardPawn)
    }

    /// True if each reward has one relic or more.
    pub fn draft_has_relic(&self) -> bool {
        self.has(UpgradeEffect::DraftRelic)
    }

    /// The gold cost of new shop items.
    pub fn reroll_cost(&self) -> u64 {
        self.owned().fold(REROLL_COST, |cost, (id, level)| match id.def().effect {
            UpgradeEffect::RerollCut { per_level } => cost.saturating_sub(per_level.saturating_mul(level)),
            _ => cost,
        })
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

    /// The shop price of an offer (`priceOf`). A relic first loses the gold of `RelicPriceCut`,
    /// down to 1 gold. Then the price gets the factor of `PriceCut`.
    pub fn price_of(&self, offer: Offer) -> u64 {
        let (mut base, mut factor) = (offer.base_price(), 1.0);
        for (id, level) in self.owned() {
            match id.def().effect {
                UpgradeEffect::PriceCut { per_level } => factor *= 1.0 - per_level * level as f64,
                UpgradeEffect::RelicPriceCut { per_level } if matches!(offer, Offer::Relic(_)) => {
                    base = base.saturating_sub(per_level.saturating_mul(level)).max(1);
                }
                _ => {}
            }
        }
        // Math.max(1, Math.round(price * factor)).
        let price = (base as f64 * factor + 0.5).floor();
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
    pub fn new(meta: &Meta, seed: u64, tuning: &Tuning) -> Run {
        let army = base_army();
        let mut run = Run {
            seed,
            floor: 1,
            gold: 0,
            next_id: army.len() as UnitId + 1,
            army,
            relics: Vec::new(),
            slots: tuning.relic_slots,
            enemy: generate_enemy(seed, 1, tuning),
            phase: Phase::Battle,
            draft: None,
            shop: Vec::new(),
            rolls: 0,
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
                UpgradeEffect::StartRelic => {
                    let pool = relic_pool(tuning);
                    let dice = &mut Dice::stream(seed, Stream::Start, 0, 0);
                    if run.can_add_relic()
                        && let Some(&id) = pool.get(dice.below(pool.len()))
                    {
                        run.relics.push(id);
                    }
                }
                UpgradeEffect::PriceCut { .. }
                | UpgradeEffect::Scout
                | UpgradeEffect::RelicPriceCut { .. }
                | UpgradeEffect::RerollCut { .. }
                | UpgradeEffect::DrawReward
                | UpgradeEffect::RewardPawn
                | UpgradeEffect::DraftRelic => {}
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

    /// True if the run has a relic slot with no relic.
    pub fn can_add_relic(&self) -> bool {
        self.relics.len() < self.slots
    }

    /// Removes a relic from the run. The run gets no gold for it, and the game can offer the
    /// relic again.
    pub fn discard_relic(&mut self, id: RelicId) -> Result<(), Fail> {
        let Some(at) = self.relics.iter().position(|&other| other == id) else {
            return fail(Code::BadArgs, format!("The run does not have the relic \"{}\"", id.key()));
        };
        self.relics.remove(at);
        Ok(())
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
    pub fn enter_camp(&mut self, with_draft: bool, meta: &Meta, tuning: &Tuning) {
        self.floor = (self.floor + 1).min(FLOORS.len());
        self.enemy = generate_enemy(self.seed, self.floor, tuning);
        self.draft = if with_draft { Some(roll_draft(self, meta, tuning)) } else { None };
        self.rolls = 0;
        self.shop = roll_shop(self, tuning);
        self.phase = Phase::Camp;
    }

    /// Takes a card of the reward. With `RewardPawn`, a unit comes with a pawn if the army has
    /// space after the unit.
    pub fn take_draft(&mut self, meta: &Meta, index: usize) -> Result<Offer, Fail> {
        let Some(draft) = &self.draft else { return fail(Code::RewardClosed, "The camp has no reward to take") };
        let Some(&offer) = draft.get(index) else {
            return fail(Code::BadIndex, format!("The reward has no card {index}"));
        };
        if let Some(blocked) = offer.blocked(self) {
            return fail(Code::Blocked, format!("The run cannot take this reward: {}", blocked.code()));
        }
        offer.take(self);
        if matches!(offer, Offer::Piece(_)) && meta.reward_gives_pawn() {
            self.add_unit(Kind::Pawn);
        }
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

    pub fn reroll_shop(&mut self, meta: &Meta, tuning: &Tuning) -> Result<(), Fail> {
        let cost = meta.reroll_cost();
        if self.gold < cost {
            return fail(Code::NotAffordable, format!("New items cost {cost} gold"));
        }
        self.gold -= cost;
        self.rolls = self.rolls.saturating_add(1);
        self.shop = roll_shop(self, tuning);
        Ok(())
    }
}

// ---- Enemies and offers ----

/// The relics that the game can offer as a reward or in the shop.
pub fn relic_pool(tuning: &Tuning) -> Vec<RelicId> {
    RelicId::all().filter(|id| !tuning.barred.contains(id)).collect()
}

/// The relics that the game can give to a boss as a trait.
pub fn trait_pool(tuning: &Tuning) -> Vec<RelicId> {
    relic_pool(tuning).into_iter().filter(|id| id.is_trait()).collect()
}

/// The enemy of a floor of the run with this seed: its kinds and its traits. The budget and the
/// traits of the floor, and the cap, the weight, and the first floor of each kind come from the
/// tuning. A kind with a weight of 0 is not in the army.
pub fn generate_enemy(seed: u64, floor: usize, tuning: &Tuning) -> Enemy {
    let dice = &mut Dice::stream(seed, Stream::Enemy, floor as u64, 0);
    let spec = tuning.floor(floor);
    let mut counts = [0u32; 5];
    let mut budget = spec.budget;
    loop {
        let pool: Vec<(usize, f64)> = RECRUIT_KINDS
            .iter()
            .zip(&tuning.kinds)
            .enumerate()
            .filter(|&(i, (&kind, k))| {
                floor >= k.min_floor && counts[i] < k.cap && k.weight > 0.0 && content::gold_value(kind) <= budget
            })
            .map(|(i, (_, k))| (i, k.weight))
            .collect();
        let Some(&i) = dice.pick_weighted(pool, 1).first() else { break };
        counts[i] += 1;
        budget -= content::gold_value(RECRUIT_KINDS[i]);
    }
    let mut kinds = vec![Kind::King];
    for (i, &kind) in RECRUIT_KINDS.iter().enumerate() {
        kinds.extend(std::iter::repeat_n(kind, counts[i] as usize));
    }
    let mut traits = dice.shuffle(trait_pool(tuning));
    traits.truncate(spec.traits);
    Enemy { pieces: EnemyPieces::Kinds(kinds), traits }
}

fn piece_pool(floor: usize) -> Vec<(Offer, f64)> {
    RECRUITS.iter().filter(|r| floor >= r.min_floor).map(|r| (Offer::Piece(r.kind), r.weight)).collect()
}

fn new_relics(run: &Run, n: usize, dice: &mut Dice, tuning: &Tuning) -> Vec<Offer> {
    let pool: Vec<RelicId> = relic_pool(tuning).into_iter().filter(|id| !run.relics.contains(id)).collect();
    dice.shuffle(pool).into_iter().take(n).map(Offer::Relic).collect()
}

/// The three free rewards. `run.floor` is the floor that comes next. With `DraftRelic`, one of
/// the two relics of the reward is the first card, also when each relic slot of the run has a
/// relic: the player can discard a relic and then take the card.
pub fn roll_draft(run: &Run, meta: &Meta, tuning: &Tuning) -> Vec<Offer> {
    let dice = &mut Dice::stream(run.seed, Stream::Draft, run.floor as u64, 0);
    let mut pool = piece_pool(run.floor);
    pool.extend(new_relics(run, 2, dice, tuning).into_iter().map(|offer| (offer, DRAFT_RELIC_WEIGHT)));
    pool.push((Offer::Gold(content::draft_gold(run.floor)), DRAFT_GOLD_WEIGHT));
    let mut draft = Vec::new();
    if meta.draft_has_relic() {
        let relics = pool.iter().enumerate().filter(|(_, (offer, _))| matches!(offer, Offer::Relic(_)));
        let relics: Vec<(usize, f64)> = relics.map(|(i, &(_, weight))| (i, weight)).collect();
        if let Some(&i) = dice.pick_weighted(relics, 1).first() {
            draft.push(pool.remove(i).0);
        }
    }
    let rest = 3 - draft.len();
    draft.extend(dice.pick_weighted(pool, rest));
    draft
}

/// The shop items of the camp before `run.floor`, after `run.rolls` rerolls.
pub fn roll_shop(run: &Run, tuning: &Tuning) -> Vec<Offer> {
    let dice = &mut Dice::stream(run.seed, Stream::Shop, run.floor as u64, run.rolls as u64);
    let mut shop = dice.pick_weighted(piece_pool(run.floor), 2);
    shop.extend(new_relics(run, 2, dice, tuning));
    shop
}
