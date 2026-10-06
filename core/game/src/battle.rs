//! One battle of a run. This module connects the chess engine to the run and its relics.

use crate::chess::{self, Boon, Color, Kind, Move, Outcome, Piece, Placement, Special, Square, State};
use crate::content::{self, ARMY_MAX, Effect, FLOORS, RelicId};
use crate::formation;
use crate::random::{Dice, Stream};
use crate::run::{CONSCRIPT_ID, ENEMY_ID_BASE, EnemyPiece, EnemyPieces, Meta, Run, UNIT_ID_MAX, UnitId};
use crate::tuning::Tuning;

#[derive(Clone, PartialEq, Debug)]
pub struct Bonus {
    pub id: RelicId,
    pub gold: u64,
}

/// A unit that a relic adds to the army after the battle.
#[derive(Clone, PartialEq, Debug)]
pub struct Recruit {
    pub id: RelicId,
    pub kind: Kind,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct BattleReward {
    pub captures: u64,
    pub clear: u64,
    /// Extra gold from relics.
    pub bonuses: Vec<Bonus>,
    /// The units that join the army.
    pub recruits: Vec<Recruit>,
}

impl BattleReward {
    pub fn total(&self) -> u64 {
        self.captures + self.clear + self.bonuses.iter().map(|b| b.gold).sum::<u64>()
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct BattleResult {
    pub outcome: Outcome,
    pub reward: BattleReward,
}

/// Who acts next in a battle.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BattlePhase {
    Player,
    Enemy,
    Over,
}

impl BattlePhase {
    pub const fn code(self) -> &'static str {
        match self {
            BattlePhase::Player => "player",
            BattlePhase::Enemy => "enemy",
            BattlePhase::Over => "over",
        }
    }
}

/// What `settle` does with the run.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Next {
    Camp,
    Won,
    Lost,
}

impl Next {
    pub const fn code(self) -> &'static str {
        match self {
            Next::Camp => "camp",
            Next::Won => "won",
            Next::Lost => "lost",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Battle {
    pub state: State,
    /// The ids of the units that the enemy captured.
    pub lost: Vec<UnitId>,
    /// The ids of the captured units that return after the battle.
    pub rescued: Vec<UnitId>,
    /// The gold from captures. It can have a fraction until the battle ends.
    pub gold: f64,
    /// The kinds that each side captured: `[White, Black]`.
    pub taken: [Vec<Kind>; 2],
    pub result: Option<BattleResult>,
    pub last: Option<Move>,
    /// The number of moves that the battle played.
    pub plies: u32,
}

/// The effects of one move.
#[derive(Clone, PartialEq, Debug)]
pub struct MoveReport {
    pub mover: Piece,
    pub mv: Move,
    /// The captured piece, its square, and the gold that the capture gave.
    pub capture: Option<(Piece, Square, f64)>,
    /// The relics of the player that had an effect, in the order of the effects.
    pub relics: Vec<RelicId>,
    /// The unit that the enemy captured, and true if it returns after the battle.
    pub unit_lost: Option<(UnitId, bool)>,
}

/// An aura of a relic of one side on the board now. The player has the relics of the run, and
/// the enemy has its traits.
#[derive(Clone, PartialEq, Debug)]
pub struct RelicAura {
    pub relic: RelicId,
    pub color: Color,
    pub boon: Boon,
    /// The ids of the pieces that give the boon, from the smallest id.
    pub sources: Vec<u16>,
    /// The squares of the pieces that have the boon.
    pub holders: Vec<Square>,
    /// The squares where the aura shows (`chess::AuraNow::zone`).
    pub zone: Vec<Square>,
}

/// The boons of the piece of a color on a square: the shield first. Each boon has the relics that
/// give it, in the order of `auras`.
pub fn boons_at(auras: &[RelicAura], color: Color, square: Square) -> Vec<(Boon, Vec<RelicId>)> {
    let mut boons = Vec::new();
    for boon in [Boon::Shield, Boon::Moves] {
        let mut relics: Vec<RelicId> = Vec::new();
        for aura in auras.iter().filter(|a| (a.color, a.boon) == (color, boon) && a.holders.contains(&square)) {
            if !relics.contains(&aura.relic) {
                relics.push(aura.relic);
            }
        }
        if !relics.is_empty() {
            boons.push((boon, relics));
        }
    }
    boons
}

/// The relics of a list that have an effect, in the order of the list.
fn effects(ids: &[RelicId]) -> impl Iterator<Item = (RelicId, Effect)> + '_ {
    ids.iter().filter_map(|&id| id.def().effect.map(|effect| (id, effect)))
}

fn piece(id: u16, kind: Kind, color: Color) -> Piece {
    Piece { id, kind, color, moved: false }
}

/// The pieces of the enemy on the board. The piece with index `i` has the id `ENEMY_ID_BASE + i`.
pub(crate) fn enemy_placements(pieces: &[EnemyPiece]) -> impl Iterator<Item = Placement> + '_ {
    pieces.iter().enumerate().map(|(i, enemy)| Placement {
        piece: piece(ENEMY_ID_BASE + i as u16, enemy.kind, Color::Black),
        square: enemy.square,
    })
}

impl Battle {
    /// The pieces at the start of a battle: the army, the pieces of the relics, and the enemy.
    /// An enemy with no set squares gets its formation here (`formation::place`).
    pub fn placements(run: &Run) -> Result<Vec<Placement>, String> {
        let mut pieces: Vec<Placement> = run
            .army
            .iter()
            .map(|unit| Placement { piece: piece(unit.id, unit.kind, Color::White), square: unit.home })
            .collect();
        let (set, kinds) = match &run.enemy.pieces {
            EnemyPieces::Placed(set) => (set.as_slice(), None),
            EnemyPieces::Kinds(kinds) => (&[][..], Some(kinds)),
        };
        for (_, effect) in effects(&run.relics) {
            if effect == Effect::ExtraPawn {
                // The pawn goes to the first free square of rank 2, or of rank 3 if rank 2 is full.
                // A square of an enemy piece is not free, thus a debug enemy on rank 2 or 3 does not
                // share a square.
                let taken = |s: Square| pieces.iter().any(|p| p.square == s) || set.iter().any(|e| e.square == s);
                if let Some(square) = (8..24).find(|&s| !taken(s)) {
                    pieces.push(Placement { piece: piece(CONSCRIPT_ID, Kind::Pawn, Color::White), square });
                }
            }
        }
        let enemy = match kinds {
            Some(kinds) => formation::place(run, kinds, &pieces)?,
            None => set.to_vec(),
        };
        pieces.extend(enemy_placements(&enemy));
        Ok(pieces)
    }

    /// The battle of the run. An error if two pieces have one square or a side does not have
    /// exactly one king.
    pub fn new(run: &Run) -> Result<Battle, String> {
        Battle::from_placements(run, &Battle::placements(run)?)
    }

    /// The engine keeps the last piece of a square and accepts a side with no king or with two
    /// kings, thus the game checks the pieces before it makes a battle.
    pub fn from_placements(run: &Run, pieces: &[Placement]) -> Result<Battle, String> {
        let mut used = 0u64;
        for placement in pieces {
            if placement.square >= 64 {
                return Err(format!("Square {} is not on the board", placement.square));
            }
            let bit = 1u64 << placement.square;
            if used & bit != 0 {
                return Err(format!("Two pieces are on square {}", placement.square));
            }
            used |= bit;
        }
        for (color, side) in [(Color::White, "The army"), (Color::Black, "The enemy")] {
            let kings = pieces.iter().filter(|p| p.piece.color == color && p.piece.kind == Kind::King).count();
            if kings != 1 {
                return Err(format!("{side} must have one king, not {kings}"));
            }
        }
        let tables = chess::tables(content::rules_for(&run.relics), content::rules_for(&run.enemy.traits))?;
        let state = chess::new_state(pieces, &tables)?;
        Ok(Battle {
            state,
            lost: Vec::new(),
            rescued: Vec::new(),
            gold: 0.0,
            taken: [Vec::new(), Vec::new()],
            result: None,
            last: None,
            plies: 0,
        })
    }

    pub fn phase(&self) -> BattlePhase {
        match (&self.result, chess::turn(&self.state)) {
            (Some(_), _) => BattlePhase::Over,
            (None, Color::White) => BattlePhase::Player,
            (None, Color::Black) => BattlePhase::Enemy,
        }
    }

    /// The auras on the board now: of each relic of the player, and then of each trait of the
    /// enemy, in the order of their lists. An aura with no source piece on the board is not in
    /// the list. The battle does not keep this list: it comes from the state and the rules data.
    ///
    /// The rules of the state are the rules of the battle. A debug command can change the relics
    /// of the run after the battle has its rules, thus an aura that the rules of the state do not
    /// have is not in the list.
    pub fn auras(&self, run: &Run) -> Vec<RelicAura> {
        let mut list = Vec::new();
        for (color, ids) in [(Color::White, &run.relics), (Color::Black, &run.enemy.traits)] {
            let in_battle = chess::side_rules(&self.state, color).auras();
            let in_battle = |aura: &chess::Aura| {
                let same = |a: &&chess::Aura| (a.boon, a.source, a.range) == (aura.boon, aura.source, aura.range);
                in_battle.iter().find(same).is_some_and(|a| a.targets & aura.targets == aura.targets)
            };
            for (i, &relic) in ids.iter().enumerate() {
                // A relic counts one time (`content::rules_for`).
                if ids[..i].contains(&relic) {
                    continue;
                }
                for aura in relic.auras().into_iter().filter(in_battle) {
                    let now = chess::aura_now(&self.state, color, &aura);
                    if !now.sources.is_empty() {
                        let chess::AuraNow { sources, holders, zone } = now;
                        list.push(RelicAura { relic, color, boon: aura.boon, sources, holders, zone });
                    }
                }
            }
        }
        list
    }

    /// The legal move of the side to move that has these squares and this promotion.
    pub fn find_move(&mut self, from: Square, to: Square, promo: Option<Kind>) -> Result<Move, FindError> {
        let candidates: Vec<Move> =
            chess::legal_moves(&mut self.state).into_iter().filter(|m| m.from == from && m.to == to).collect();
        match (candidates.as_slice(), promo) {
            ([], _) => Err(FindError::Illegal),
            ([only], None) if only.promo.is_none() => Ok(*only),
            (_, None) => Err(FindError::PromoRequired),
            (moves, Some(kind)) => moves.iter().copied().find(|m| m.promo == Some(kind)).ok_or(FindError::Illegal),
        }
    }

    /// Plays a legal move for the side to move and records its effect on the run (`playMove`).
    pub fn play(&mut self, run: &Run, mv: Move) -> MoveReport {
        let mover_color = chess::turn(&self.state);
        let mover = chess::piece_at(&self.state, mv.from).unwrap_or(piece(0, Kind::Pawn, mover_color));
        let played = chess::play(&mut self.state, mv);
        self.last = Some(mv);
        self.plies = self.plies.saturating_add(1);
        let mut report = MoveReport { mover, mv, capture: None, relics: Vec::new(), unit_lost: None };
        if let Some((captured, square)) = played.captured {
            self.taken[mover_color.index()].push(captured.kind);
            let mut gold = 0.0;
            if mover_color == Color::White {
                gold = content::gold_value(captured.kind) as f64;
                for (id, effect) in effects(&run.relics) {
                    if let Effect::CaptureGold { factor } = effect {
                        let next = gold * factor;
                        if next != gold {
                            report.relics.push(id);
                        }
                        gold = next;
                    }
                }
                self.gold += gold;
            } else if run.army.iter().any(|unit| unit.id == captured.id) {
                let rescuer = effects(&run.relics)
                    .find(|&(_, effect)| effect == Effect::RescueFirst && self.rescued.is_empty())
                    .map(|(id, _)| id);
                if let Some(id) = rescuer {
                    report.relics.push(id);
                    self.rescued.push(captured.id);
                } else {
                    self.lost.push(captured.id);
                }
                report.unit_lost = Some((captured.id, rescuer.is_some()));
            }
            report.capture = Some((captured, square, gold));
        }
        if let Some(outcome) = chess::outcome(&mut self.state) {
            let reward = self.reward_for(run, outcome);
            let used = |id| reward.bonuses.iter().any(|b| b.id == id) || reward.recruits.iter().any(|r| r.id == id);
            report.relics.extend(run.relics.iter().copied().filter(|&id| used(id)));
            self.result = Some(BattleResult { outcome, reward });
        }
        report
    }

    fn reward_for(&self, run: &Run, outcome: Outcome) -> BattleReward {
        let mut reward = BattleReward::default();
        if outcome.winner() == Some(Color::Black) {
            return reward;
        }
        // Math.round of a number that is 0 or more.
        let round = |gold: f64| (gold + 0.5).floor() as u64;
        reward.captures = round(self.gold);
        let won = outcome.winner() == Some(Color::White);
        if won {
            reward.clear = 3 + run.floor as u64;
        }
        for (id, effect) in effects(&run.relics) {
            let gold = match effect {
                Effect::VictoryGold { per, max } if won => (run.gold.saturating_add(reward.total()) / per).min(max),
                Effect::CheckmateGold if outcome == (Outcome::Checkmate { winner: Color::White }) => {
                    let enemy = chess::pieces(&self.state).into_iter().filter(|(_, p)| p.color == Color::Black);
                    enemy.map(|(_, p)| content::gold_value(p.kind) as u64).sum()
                }
                Effect::LossGold => {
                    let prices: u64 =
                        self.taken[Color::Black.index()].iter().map(|&kind| content::piece_price(kind)).sum();
                    round(prices as f64 / 2.0)
                }
                Effect::PromotionRecruit if won && self.has_promoted_unit(run) && self.army_has_space(run) => {
                    reward.recruits.push(Recruit { id, kind: Kind::Pawn });
                    0
                }
                _ => 0,
            };
            if gold > 0 {
                reward.bonuses.push(Bonus { id, gold });
            }
        }
        reward
    }

    /// True if a unit that was a pawn at the start of the battle is on the board with another
    /// kind. A captured unit is not on the board, and the pawn of Conscription is not a unit.
    fn has_promoted_unit(&self, run: &Run) -> bool {
        let pieces = chess::pieces(&self.state);
        let mut pawns = run.army.iter().filter(|unit| unit.kind == Kind::Pawn);
        pawns.any(|unit| pieces.iter().any(|(_, p)| p.id == unit.id && p.kind != Kind::Pawn))
    }

    /// True if the army after the battle can take one more unit.
    fn army_has_space(&self, run: &Run) -> bool {
        run.army.len() - self.lost.len() < ARMY_MAX && run.next_id <= UNIT_ID_MAX
    }

    /// The move of an AI level. The seed of the AI comes from the seed of the run, the floor, and
    /// the number of moves that the battle played.
    pub fn ai_move(&mut self, run: &Run, level: usize) -> Option<Move> {
        let seed = Dice::stream(run.seed, Stream::Ai, run.floor as u64, self.plies as u64).seed();
        chess::ai_move(&mut self.state, level, seed)
    }

    /// What `settle` will do.
    pub fn next(&self, run: &Run) -> Option<Next> {
        let result = self.result.as_ref()?;
        Some(match result.outcome.winner() {
            Some(Color::Black) => Next::Lost,
            Some(Color::White) if run.floor == FLOORS.len() => Next::Won,
            _ => Next::Camp,
        })
    }

    /// Applies a completed battle to the run (`settleBattle`). Returns None if the battle has no result.
    pub fn settle(&self, run: &mut Run, meta: &Meta, tuning: &Tuning) -> Option<Next> {
        let next = self.next(run)?;
        let result = self.result.as_ref()?;
        if next == Next::Lost {
            return Some(next);
        }
        run.gold = run.gold.saturating_add(result.reward.total());
        run.army.retain(|unit| !self.lost.contains(&unit.id));
        // A unit keeps the kind that it has on the board, thus a promoted pawn stays promoted.
        for (_, p) in chess::pieces(&self.state) {
            if let Some(unit) = run.army.iter_mut().find(|u| u.id == p.id) {
                unit.kind = p.kind;
            }
        }
        for recruit in &result.reward.recruits {
            run.add_unit(recruit.kind);
        }
        if next == Next::Camp {
            // A draw on the last floor stays on that floor, and each camp before a floor has the
            // same reward. Thus such a draw gives no reward.
            let won = result.outcome.winner() == Some(Color::White);
            let with_draft = won || (meta.draw_gives_reward() && run.floor < FLOORS.len());
            run.enter_camp(with_draft, meta, tuning);
        }
        Some(next)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FindError {
    Illegal,
    PromoRequired,
}

/// True for a move that captures: a piece on the target square, or en passant.
pub fn is_capture(state: &State, m: Move) -> bool {
    m.special == Special::EnPassant || chess::piece_at(state, m.to).is_some()
}
