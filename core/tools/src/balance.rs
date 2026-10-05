//! Battles of the game for `balance`: a sweep of floors, AI levels, armies, traits, and relic
//! sets, and the data of each battle.
//!
//! A battle here is a `Battle` of the crate `chrogue-game`. Thus it has the relics, the traits,
//! the formation of the enemy, and the rewards of the game.
//!
//! Battle `i` of each cell has the same run seed. Thus two cells with the same floor and the same
//! army have the same two armies in battle `i`, and their results compare battle by battle.

use chrogue_game::battle::Battle;
use chrogue_game::chess::{self, Color, Kind, Outcome, mix};
use chrogue_game::content::{self, ARMY_MAX, FLOORS, RECRUITS, RELICS, RelicId};
use chrogue_game::random::Dice;
use chrogue_game::run::{Meta, Run, SEED_MAX, Unit, generate_enemy};
use chrogue_game::tuning::Tuning;
use serde_json::{Value, json};

use crate::cli::{number, option, ranges, unknown};

const SEED_SALT: u64 = 0xBA1A;
const ARMY_SALT: u64 = 0xA7;
/// The home square of the king of an army of letters.
const KING_HOME: u8 = 4;
/// The largest number of `auto+N` and `auto-N`: the value of a full army.
const OFFSET_MAX: i64 = 39;
/// The options of `Sweep::parse`.
pub const OPTIONS: [&str; 10] =
    ["--floor", "--player", "--enemy", "--army", "--traits", "--relics", "--games", "--seed", "--max-plies", "--gold"];

/// The AI level of the enemy.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnemyLevel {
    /// The level of the floor in `FLOORS`.
    Floor,
    Level(usize),
}

/// The army of the player.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Army {
    /// The base army plus random recruits. The value of the army is the budget of the floor plus
    /// this number. The army is not smaller than the base army.
    Auto(i64),
    /// These kinds, the king first. Each kind after the king gets the next free home square.
    Kinds(Vec<Kind>),
}

/// The traits of the enemy.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Traits {
    /// The traits that the game gives to the enemy of the floor.
    Floor,
    Set(Vec<RelicId>),
}

/// The values of each axis of a sweep, and the numbers of its battles. A text is the argument
/// that gave the value.
#[derive(Clone, PartialEq, Debug)]
pub struct Sweep {
    pub floors: Vec<usize>,
    pub players: Vec<usize>,
    pub enemies: Vec<EnemyLevel>,
    pub armies: Vec<(String, Army)>,
    pub traits: Vec<(String, Traits)>,
    pub relics: Vec<Vec<RelicId>>,
    /// The battles of each cell.
    pub games: usize,
    pub seed: u64,
    /// A battle that gets to this number of half moves is a draw.
    pub max_plies: u32,
    /// The gold of the run before each battle.
    pub gold: u64,
}

/// One combination of the axes: an index for each axis of the sweep.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    pub floor: usize,
    pub player: usize,
    pub enemy: usize,
    pub army: usize,
    pub traits: usize,
    pub relics: usize,
}

/// The data of one battle.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Game {
    /// `w`: the player won. `d`: a draw. `l`: the player lost.
    pub result: char,
    /// `m`: checkmate. `s`: stalemate. `r`: rout. `b`: bare kings. `c`: the clock. `x`: `max_plies`.
    pub end: char,
    pub plies: u32,
    /// The gold reward of the battle.
    pub gold: u64,
    /// The piece value of the units that the enemy captured and that did not return.
    pub lost: u32,
    /// The units that the relics added to the army after the battle.
    pub recruits: u32,
}

fn relic(key: &str) -> Result<RelicId, String> {
    RelicId::parse(key).ok_or_else(|| format!("\"{key}\" is not a relic"))
}

/// A set of relics in the order of `RELICS`, each relic one time.
fn relic_set(mut ids: Vec<RelicId>) -> Vec<RelicId> {
    ids.sort();
    ids.dedup();
    ids
}

/// The relic sets of `--relics`: items with `,` between them. An item is `none`, `pairs` (each
/// set of two relics), or relic keys with `+` between them. `each` in the place of a key gives one
/// set for each relic. A set is in the result one time. An error if an item has a key two times.
pub fn relic_sets(text: &str) -> Result<Vec<Vec<RelicId>>, String> {
    let mut sets: Vec<Vec<RelicId>> = Vec::new();
    for item in text.split(',') {
        let item = if item == "pairs" { "each+each" } else { item };
        let mut partial: Vec<Vec<RelicId>> = vec![Vec::new()];
        let members: Vec<&str> = if item == "none" { Vec::new() } else { item.split('+').collect() };
        let twice = |(i, member): (usize, &&str)| *member != "each" && members[..i].contains(member);
        if members.iter().enumerate().any(twice) {
            return Err(format!("\"{item}\" has a relic two times"));
        }
        for member in &members {
            let choices: Vec<RelicId> = if *member == "each" { RelicId::all().collect() } else { vec![relic(member)?] };
            partial = partial
                .iter()
                .flat_map(|set| choices.iter().map(move |&id| set.iter().copied().chain([id]).collect()))
                .collect();
        }
        for set in partial.into_iter().map(relic_set) {
            // A set with a relic two times is a smaller set, and that set has its own item.
            if set.len() == members.len() && !sets.contains(&set) {
                sets.push(set);
            }
        }
    }
    Ok(sets)
}

fn enemy_levels(text: &str) -> Result<Vec<EnemyLevel>, String> {
    let mut list = Vec::new();
    for part in text.split(',') {
        let levels = match part {
            "floor" => vec![EnemyLevel::Floor],
            _ => ranges(part, 1, chess::LEVELS)?.into_iter().map(EnemyLevel::Level).collect(),
        };
        for level in levels {
            if !list.contains(&level) {
                list.push(level);
            }
        }
    }
    Ok(list)
}

fn army(text: &str) -> Result<Army, String> {
    if let Some(offset) = text.strip_prefix("auto") {
        let signed = offset.starts_with(['+', '-']);
        let offset = if offset.is_empty() { Some(0) } else { offset.parse::<i64>().ok().filter(|_| signed) };
        return offset
            .filter(|offset| offset.abs() <= OFFSET_MAX)
            .map(Army::Auto)
            .ok_or_else(|| format!("\"{text}\" must be auto, auto+N, or auto-N, with N from 0 to {OFFSET_MAX}"));
    }
    let kinds: Option<Vec<Kind>> = text.chars().map(|letter| Kind::from_letter(letter.to_ascii_lowercase())).collect();
    let Some(mut kinds) = kinds else {
        return Err(format!("\"{text}\" is not an army: use auto or the letters k, q, r, b, n, p"));
    };
    if kinds.iter().filter(|&&kind| kind == Kind::King).count() != 1 {
        return Err(format!("The army \"{text}\" must have one king"));
    }
    if kinds.len() > ARMY_MAX {
        return Err(format!("The army \"{text}\" has more than {ARMY_MAX} units"));
    }
    kinds.sort_by_key(|&kind| kind != Kind::King);
    Ok(Army::Kinds(kinds))
}

fn traits(text: &str) -> Result<Traits, String> {
    match text {
        "floor" => Ok(Traits::Floor),
        "none" => Ok(Traits::Set(Vec::new())),
        _ => {
            let ids = text.split('+').map(relic).collect::<Result<Vec<RelicId>, String>>()?;
            match ids.iter().find(|id| !id.is_trait()) {
                Some(id) => Err(format!("The relic \"{}\" cannot be a trait", id.key())),
                None => Ok(Traits::Set(relic_set(ids))),
            }
        }
    }
}

/// The values of a list with `,` between them, each one time, with the text of each value.
fn list<T: PartialEq>(text: &str, parse: fn(&str) -> Result<T, String>) -> Result<Vec<(String, T)>, String> {
    let mut values: Vec<(String, T)> = Vec::new();
    for part in text.split(',') {
        let value = parse(part)?;
        if values.iter().all(|(_, other)| *other != value) {
            values.push((part.to_string(), value));
        }
    }
    Ok(values)
}

impl Sweep {
    /// The sweep of the arguments of `balance`. `others` has the options that the caller reads.
    pub fn parse(args: &[String], others: &[&str]) -> Result<Sweep, String> {
        if let Some(arg) = unknown(args, &[&OPTIONS[..], others].concat()) {
            return Err(format!("\"{arg}\" is not an option"));
        }
        let games = number(args, "--games", 40) as usize;
        if games == 0 {
            return Err("--games must be 1 or more".to_string());
        }
        let text = |name: &str, default: &'static str| option(args, name).unwrap_or(default).to_string();
        let axis = |name: &str, error: String| format!("{name}: {error}");
        Ok(Sweep {
            floors: ranges(&text("--floor", "1-8"), 1, FLOORS.len()).map_err(|e| axis("--floor", e))?,
            players: ranges(&text("--player", "3"), 1, chess::LEVELS).map_err(|e| axis("--player", e))?,
            enemies: enemy_levels(&text("--enemy", "floor")).map_err(|e| axis("--enemy", e))?,
            armies: list(&text("--army", "auto"), army).map_err(|e| axis("--army", e))?,
            traits: list(&text("--traits", "floor"), traits).map_err(|e| axis("--traits", e))?,
            relics: relic_sets(&text("--relics", "none,each")).map_err(|e| axis("--relics", e))?,
            games,
            seed: number(args, "--seed", 1),
            max_plies: number(args, "--max-plies", 300).min(u32::MAX as u64) as u32,
            gold: number(args, "--gold", 0),
        })
    }

    /// Each combination of the axes. The relic sets change first, thus the cells that compare
    /// battle by battle are together.
    pub fn cells(&self) -> Vec<Cell> {
        let mut cells = Vec::new();
        for floor in 0..self.floors.len() {
            for army in 0..self.armies.len() {
                for traits in 0..self.traits.len() {
                    for player in 0..self.players.len() {
                        for enemy in 0..self.enemies.len() {
                            for relics in 0..self.relics.len() {
                                cells.push(Cell { floor, player, enemy, army, traits, relics });
                            }
                        }
                    }
                }
            }
        }
        cells
    }

    /// The seed of the run of battle `game` of each cell, from 0 to `SEED_MAX`.
    pub fn run_seed(&self, game: usize) -> u64 {
        mix(mix(self.seed, SEED_SALT), game as u64) % (SEED_MAX + 1)
    }

    /// The run before battle `game` of the cell: the army, the relics, the gold, and the enemy.
    pub fn run(&self, cell: Cell, game: usize) -> Run {
        let tuning = Tuning::default();
        let (seed, floor) = (self.run_seed(game), self.floors[cell.floor]);
        let mut run = Run::new(&Meta::default(), seed, &tuning);
        run.floor = floor;
        run.gold = self.gold;
        run.relics = self.relics[cell.relics].clone();
        run.enemy = generate_enemy(seed, floor, &tuning);
        if let Traits::Set(ids) = &self.traits[cell.traits].1 {
            run.enemy.traits = ids.clone();
        }
        match &self.armies[cell.army].1 {
            Army::Auto(offset) => {
                let dice = &mut Dice::new(mix(mix(seed, floor as u64), ARMY_SALT));
                let value = |run: &Run| run.army.iter().map(|unit| content::gold_value(unit.kind) as i64).sum::<i64>();
                let target = value(&run).max(run.floor_def().budget as i64) + offset;
                while run.can_add_unit() {
                    let rest = target - value(&run);
                    let pool: Vec<(Kind, f64)> = RECRUITS
                        .iter()
                        .filter(|r| floor >= r.min_floor && content::gold_value(r.kind) as i64 <= rest)
                        .map(|r| (r.kind, r.weight))
                        .collect();
                    let Some(&kind) = dice.pick_weighted(pool, 1).first() else { break };
                    run.add_unit(kind);
                }
            }
            Army::Kinds(kinds) => {
                run.army = vec![Unit { id: 1, kind: Kind::King, home: KING_HOME }];
                run.next_id = 2;
                for &kind in &kinds[1..] {
                    run.add_unit(kind);
                }
            }
        }
        run
    }

    /// The AI levels of the player and of the enemy in the cell.
    pub fn levels(&self, cell: Cell) -> (usize, usize) {
        let enemy = match self.enemies[cell.enemy] {
            EnemyLevel::Floor => content::floor_def(self.floors[cell.floor]).level,
            EnemyLevel::Level(level) => level,
        };
        (self.players[cell.player], enemy)
    }

    /// Plays battle `game` of the cell. An error if the armies cannot start a battle. A battle
    /// that gets to `max_plies` has no reward, thus its gold is 0.
    pub fn play(&self, cell: Cell, game: usize) -> Result<Game, String> {
        let run = self.run(cell, game);
        let (player, enemy) = self.levels(cell);
        let mut battle = Battle::new(&run)?;
        while battle.result.is_none() && battle.plies < self.max_plies {
            let level = if chess::turn(&battle.state) == Color::White { player } else { enemy };
            let Some(mv) = battle.ai_move(&run, level) else {
                return Err("The battle has ended before the first move".to_string());
            };
            battle.play(&run, mv);
        }
        let value = |id| run.army.iter().find(|unit| unit.id == id).map_or(0, |unit| content::gold_value(unit.kind));
        let lost = battle.lost.iter().map(|&id| value(id)).sum();
        let Some(result) = &battle.result else {
            return Ok(Game { result: 'd', end: 'x', plies: battle.plies, gold: 0, lost, recruits: 0 });
        };
        Ok(Game {
            result: match result.outcome.winner() {
                Some(Color::White) => 'w',
                Some(Color::Black) => 'l',
                None => 'd',
            },
            end: match result.outcome {
                Outcome::Checkmate { .. } => 'm',
                Outcome::Stalemate { .. } => 's',
                Outcome::Rout { .. } => 'r',
                Outcome::Bare => 'b',
                Outcome::Clock => 'c',
            },
            plies: battle.plies,
            gold: result.reward.total(),
            lost,
            recruits: result.reward.recruits.len() as u32,
        })
    }

    /// The data of the sweep (`balance.json`). `games` has the battles of each cell, in the order
    /// of `cells`.
    pub fn json(&self, command: &str, games: &[Vec<Game>]) -> Value {
        let keys = |ids: &[RelicId]| ids.iter().map(|id| id.key()).collect::<Vec<_>>();
        let relics: Vec<Value> = RELICS
            .iter()
            .map(|def| {
                let kind = if def.rules.is_empty() { "effect" } else { "rule" };
                json!({ "key": def.key, "name": def.name, "text": def.text, "kind": kind, "trait": def.foe_text.is_some() })
            })
            .collect();
        let levels: Vec<Value> =
            (1..=chess::LEVELS).map(|n| json!({ "number": n, "name": chess::level_name(n) })).collect();
        let floors: Vec<Value> = FLOORS
            .iter()
            .enumerate()
            .map(|(i, floor)| {
                json!({
                    "number": i + 1, "name": floor.name, "level": floor.level, "budget": floor.budget,
                    "traits": floor.traits, "boss": floor.boss,
                })
            })
            .collect();
        let enemies: Vec<Value> = self
            .enemies
            .iter()
            .map(|enemy| match enemy {
                EnemyLevel::Floor => json!("floor"),
                EnemyLevel::Level(level) => json!(level),
            })
            .collect();
        let cells: Vec<Value> = self
            .cells()
            .iter()
            .zip(games)
            .map(|(cell, games)| {
                json!({
                    "floor": cell.floor, "player": cell.player, "enemy": cell.enemy,
                    "army": cell.army, "traits": cell.traits, "relics": cell.relics,
                    "results": games.iter().map(|game| game.result).collect::<String>(),
                    "ends": games.iter().map(|game| game.end).collect::<String>(),
                    "plies": games.iter().map(|game| game.plies).collect::<Vec<_>>(),
                    "gold": games.iter().map(|game| game.gold).collect::<Vec<_>>(),
                    "lost": games.iter().map(|game| game.lost).collect::<Vec<_>>(),
                    "recruits": games.iter().map(|game| game.recruits).sum::<u32>(),
                })
            })
            .collect();
        json!({
            "format": "chrogue-balance",
            "version": 1,
            "command": command,
            "seed": self.seed,
            "games": self.games,
            "max_plies": self.max_plies,
            "gold": self.gold,
            "content": { "relics": relics, "levels": levels, "floors": floors },
            "axes": {
                "floors": self.floors,
                "players": self.players,
                "enemies": enemies,
                "armies": self.armies.iter().map(|(text, _)| text).collect::<Vec<_>>(),
                "traits": self.traits.iter().map(|(text, traits)| match traits {
                    Traits::Floor => text.clone(),
                    Traits::Set(ids) if ids.is_empty() => text.clone(),
                    Traits::Set(ids) => keys(ids).join("+"),
                }).collect::<Vec<_>>(),
                "relics": self.relics.iter().map(|set| keys(set)).collect::<Vec<_>>(),
            },
            "cells": cells,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sweep(args: &[&str]) -> Sweep {
        Sweep::parse(&args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>(), &[]).expect("the arguments are valid")
    }

    fn keys(sets: &[Vec<RelicId>]) -> Vec<String> {
        sets.iter().map(|set| set.iter().map(|id| id.key()).collect::<Vec<_>>().join("+")).collect()
    }

    #[test]
    fn the_relic_sets_have_each_set_one_time() {
        let sets = relic_sets("none,gallop+bounty,bounty+gallop,bounty").unwrap();
        assert_eq!(keys(&sets), ["", "bounty+gallop", "bounty"]);
        assert_eq!(relic_sets("none,each").unwrap().len(), 1 + RELICS.len());
        assert_eq!(relic_sets("pairs").unwrap().len(), RELICS.len() * (RELICS.len() - 1) / 2);
        let with_gallop = relic_sets("gallop+each").unwrap();
        assert_eq!(with_gallop.len(), RELICS.len() - 1);
        assert!(with_gallop.iter().all(|set| set.len() == 2 && set.contains(&relic("gallop").unwrap())));
        assert!(relic_sets("gallop+nothing").is_err());
    }

    #[test]
    fn the_arguments_give_the_axes() {
        let sweep = sweep(&[
            "--floor",
            "2-3,8",
            "--player",
            "1-2",
            "--enemy",
            "floor,5-6",
            "--army",
            "auto,auto+3,auto-2,KQRPP",
            "--traits",
            "floor,none,gallop+longLeap",
            "--relics",
            "none,bounty",
            "--games",
            "5",
        ]);
        assert_eq!((sweep.floors.as_slice(), sweep.players.as_slice()), (&[2, 3, 8][..], &[1, 2][..]));
        assert_eq!(sweep.enemies, [EnemyLevel::Floor, EnemyLevel::Level(5), EnemyLevel::Level(6)]);
        let armies: Vec<&Army> = sweep.armies.iter().map(|(_, army)| army).collect();
        let letters = Army::Kinds(vec![Kind::King, Kind::Queen, Kind::Rook, Kind::Pawn, Kind::Pawn]);
        assert_eq!(armies, [&Army::Auto(0), &Army::Auto(3), &Army::Auto(-2), &letters]);
        assert_eq!(sweep.traits.len(), 3);
        assert_eq!(sweep.cells().len(), 3 * 2 * 3 * 4 * 3 * 2);

        let bad =
            |args: &[&str]| Sweep::parse(&args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>(), &[]).is_err();
        assert!(bad(&["--floor", "9"]));
        assert!(bad(&["--player", "0"]));
        assert!(bad(&["--army", "QRPP"]));
        assert!(bad(&["--army", "KKP"]));
        assert!(bad(&["--army", "auto+x"]));
        assert!(bad(&["--army", "auto3"]));
        assert!(bad(&["--army", "auto+40"]));
        assert!(bad(&["--games", "0"]));
        assert!(bad(&["--floors", "3"]));
        assert!(bad(&["--relics", "gallop+gallop"]));
        assert!(bad(&["--traits", "bounty"]));
    }

    #[test]
    fn the_cells_of_one_battle_have_the_same_armies() {
        let sweep = sweep(&["--floor", "6", "--player", "1,4", "--relics", "none,gallop", "--traits", "floor,none"]);
        let cells = sweep.cells();
        let first = sweep.run(cells[0], 3);
        for &cell in &cells {
            let run = sweep.run(cell, 3);
            assert_eq!((&run.army, &run.enemy.pieces), (&first.army, &first.enemy.pieces));
            assert_eq!(run.relics, sweep.relics[cell.relics]);
        }
        assert_ne!(sweep.run(cells[0], 4).enemy.pieces, first.enemy.pieces);
    }

    #[test]
    fn an_auto_army_has_the_value_of_the_enemy_plus_the_offset() {
        let sweep = sweep(&["--floor", "1,6", "--army", "auto,auto+3,auto-39"]);
        for game in 0..20 {
            for cell in sweep.cells() {
                let army = sweep.run(cell, game).army;
                let value = army.iter().map(|unit| content::gold_value(unit.kind)).sum::<u32>();
                let base = FLOORS[sweep.floors[cell.floor] - 1].budget.max(12);
                let expected = [base, base + 3, 12][cell.army];
                // A full army can have less than the value.
                assert!(value == expected || (value < expected && army.len() == ARMY_MAX), "{value} and {expected}");
            }
        }
    }

    #[test]
    fn an_army_of_letters_has_these_kinds() {
        let sweep = sweep(&["--army", "rkqp"]);
        let run = sweep.run(sweep.cells()[0], 0);
        let kinds: Vec<Kind> = run.army.iter().map(|unit| unit.kind).collect();
        assert_eq!(kinds, [Kind::King, Kind::Rook, Kind::Queen, Kind::Pawn]);
        assert_eq!(run.army[0].home, KING_HOME);

        let king = self::sweep(&["--army", "k"]);
        assert!(king.play(king.cells()[0], 0).is_err());
    }

    #[test]
    fn the_same_arguments_give_the_same_data() {
        let sweep = sweep(&["--floor", "1-2", "--player", "2", "--relics", "none,conscription", "--games", "3"]);
        let play = || -> Vec<Vec<Game>> {
            let cells = sweep.cells();
            cells.iter().map(|&cell| (0..sweep.games).map(|game| sweep.play(cell, game).unwrap()).collect()).collect()
        };
        let games = play();
        assert_eq!(games, play());
        let data = sweep.json("balance", &games);
        assert_eq!(data["cells"].as_array().unwrap().len(), 4);
        assert_eq!(data["cells"][0]["results"].as_str().unwrap().len(), 3);
        assert_eq!(data["axes"]["relics"], json!([[], ["conscription"]]));
        assert_eq!(data["content"]["relics"].as_array().unwrap().len(), RELICS.len());
    }
}
