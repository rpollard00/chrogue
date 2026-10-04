//! The views: the complete data to draw each screen, and the content tables of `hello`.

use serde_json::{Value, json};

use crate::battle::{Battle, BattlePhase, is_capture};
use crate::chess::{self, Color, Kind, Move};
use crate::content::{
    self, ARMY_MAX, FLOORS, RECRUIT_KINDS, RECRUITS, RELIC_PRICE, RELIC_SLOTS, RELICS, RELICS_MAX, REROLL_COST,
    RelicId, TRAITS_MAX, UPGRADE_NAME_MAX, UPGRADE_SLOTS, UPGRADES, UpgradeId, WIN_CROWNS,
};
use crate::protocol::{Code, Command, EventKind, PROTOCOL_VERSION, gold_number};
use crate::run::{Meta, Offer, Phase, Run, RunSummary, SEED_MAX};
use crate::save::run_json;
use crate::session::{Reward, Screen};
use crate::tuning::{self, BUDGET_MAX, Tuning, WEIGHT_MAX};

pub fn letter(kind: Kind) -> String {
    chess::kind_letter(kind).to_string()
}

fn meta_view(meta: &Meta) -> Value {
    json!({ "crowns": meta.crowns, "best": meta.best, "runs": meta.runs })
}

fn floor_view(floor: usize) -> Value {
    let def = content::floor_def(floor);
    json!({ "number": floor, "name": def.name, "boss": def.boss, "total": FLOORS.len() })
}

pub fn relic_view(id: RelicId) -> Value {
    json!({ "id": id.key(), "name": id.def().name, "text": id.def().text })
}

/// An enemy trait shows the text for the enemy (`relicList` with the side "enemy").
pub fn trait_view(id: RelicId) -> Value {
    json!({ "id": id.key(), "name": id.def().name, "text": id.def().foe_text.unwrap_or(id.def().text) })
}

pub fn offer_view(offer: Offer, run: &Run) -> Value {
    let (name, text) = offer.describe();
    let mut view = match offer {
        Offer::Piece(kind) => json!({ "kind": "piece", "piece": letter(kind) }),
        Offer::Relic(id) => json!({ "kind": "relic", "id": id.key() }),
        Offer::Gold(amount) => json!({ "kind": "gold", "amount": amount }),
    };
    view["name"] = Value::from(name);
    view["text"] = text.map_or(Value::Null, Value::from);
    view["blocked"] = offer.blocked(run).map_or(Value::Null, |b| Value::from(b.code()));
    view
}

fn move_view(battle: &Battle, m: Move, preview: bool) -> Value {
    let mut view = json!({
        "id": chess::piece_at(&battle.state, m.from).map(|p| p.id),
        "from": m.from,
        "to": m.to,
        "capture": is_capture(&battle.state, m),
        "special": chess::special_name(m.special),
    });
    if let Some(kind) = m.promo {
        view["promo"] = Value::from(letter(kind));
    }
    if preview {
        view["preview"] = Value::from(true);
    }
    view
}

fn result_view(battle: &Battle, run: &Run) -> Value {
    let Some(result) = &battle.result else { return Value::Null };
    let winner = result.outcome.winner();
    let reward = &result.reward;
    let bonuses: Vec<Value> =
        reward.bonuses.iter().map(|b| json!({ "id": b.id.key(), "label": b.id.def().name, "gold": b.gold })).collect();
    let mut rows = Vec::new();
    let mut rescued = 0;
    if winner != Some(Color::Black) {
        rows.push(json!({ "row": "captures", "gold": reward.captures }));
        if reward.clear > 0 {
            rows.push(json!({ "row": "clear", "gold": reward.clear }));
        }
        for b in &reward.bonuses {
            rows.push(json!({ "row": "bonus", "id": b.id.key(), "label": b.id.def().name, "gold": b.gold }));
        }
        rescued = battle.rescued.len();
    }
    json!({
        "winner": winner.map(chess::color_letter),
        "reason": result.outcome.reason(),
        "outcome": match winner { Some(Color::White) => "victory", Some(Color::Black) => "defeat", None => "draw" },
        "next": battle.next(run).map(|n| n.code()),
        "reward": { "captures": reward.captures, "clear": reward.clear, "bonuses": bonuses, "total": reward.total() },
        "rows": rows,
        "total": reward.total(),
        "rescued": rescued,
        "recruits": reward.recruits.iter().map(|r| letter(r.kind)).collect::<Vec<_>>(),
    })
}

fn battle_view(run: &Run, battle: &Battle, meta: &Meta) -> Value {
    let phase = battle.phase();
    let mut state = battle.state.clone();
    let pieces: Vec<Value> = chess::pieces(&state)
        .into_iter()
        .map(|(s, p)| json!({ "id": p.id, "kind": letter(p.kind), "color": chess::color_letter(p.color), "square": s }))
        .collect();
    let scout = meta.can_scout();
    let (moves, enemy_moves) = if phase == BattlePhase::Player {
        let moves: Vec<Value> =
            chess::legal_moves(&mut state).into_iter().map(|m| move_view(battle, m, false)).collect();
        let enemy: Vec<Value> = if scout {
            chess::pieces(&state)
                .into_iter()
                .filter(|(_, p)| p.color == Color::Black)
                .flat_map(|(s, _)| chess::moves_from(&state, s))
                .map(|m| move_view(battle, m, true))
                .collect()
        } else {
            Vec::new()
        };
        (moves, enemy)
    } else {
        (Vec::new(), Vec::new())
    };
    let kinds = |list: &[Kind]| list.iter().map(|&k| letter(k)).collect::<Vec<_>>();
    json!({
        "screen": "battle",
        "floor": floor_view(run.floor),
        "phase": phase.code(),
        "turn": chess::color_letter(chess::turn(&state)),
        "pieces": pieces,
        "check": chess::check_square(&state),
        "last": battle.last.map(|m| json!({ "from": m.from, "to": m.to })),
        "moves": moves,
        "scout": scout,
        "enemy_moves": enemy_moves,
        "taken": { "w": kinds(&battle.taken[0]), "b": kinds(&battle.taken[1]) },
        "gold": run.gold,
        "capture_gold": (battle.gold + 0.5).floor() as u64,
        "capture_gold_exact": gold_number(battle.gold),
        "lost": battle.lost,
        "rescued": battle.rescued,
        "relics": run.relics.iter().map(|&id| relic_view(id)).collect::<Vec<_>>(),
        "relic_slots": run.slots,
        "traits": run.enemy.traits.iter().map(|&id| trait_view(id)).collect::<Vec<_>>(),
        "clock": chess::clock(&state),
        "result": result_view(battle, run),
    })
}

/// The kinds of the enemy army as the camp shows them: the king first, then by value.
fn enemy_kinds(run: &Run) -> Vec<String> {
    let rank = |kind: Kind| if kind == Kind::King { u32::MAX } else { content::gold_value(kind) };
    let mut kinds = run.enemy.pieces.kinds();
    kinds.sort_by_key(|&k| std::cmp::Reverse(rank(k)));
    kinds.into_iter().map(letter).collect()
}

fn camp_view(run: &Run, reward: &Option<Reward>, meta: &Meta) -> Value {
    let reward = reward.as_ref().map(|r| {
        json!({
            "offers": r.offers.iter().map(|&o| offer_view(o, run)).collect::<Vec<_>>(),
            "taken": r.taken,
            "open": run.draft.is_some(),
            "state": if run.draft.is_some() { "open" } else if r.taken.is_some() { "taken" } else { "skipped" },
        })
    });
    let shop: Vec<Value> = run
        .shop
        .iter()
        .map(|&offer| {
            let price = meta.price_of(offer);
            let mut view = offer_view(offer, run);
            view["price"] = Value::from(price);
            view["affordable"] = Value::from(run.gold >= price);
            view
        })
        .collect();
    let mut army: Vec<_> = run.army.clone();
    army.sort_by_key(|u| u.home);
    json!({
        "screen": "camp",
        "floor": floor_view(run.floor),
        "enemy": {
            "name": run.floor_def().name,
            "traits": run.enemy.traits.iter().map(|&id| trait_view(id)).collect::<Vec<_>>(),
            "kinds": enemy_kinds(run),
        },
        "reward": reward,
        "shop": { "offers": shop, "reroll_cost": meta.reroll_cost(), "can_reroll": run.gold >= meta.reroll_cost() },
        "army": army.iter().map(|u| json!({ "id": u.id, "kind": letter(u.kind), "home": u.home })).collect::<Vec<_>>(),
        "army_max": ARMY_MAX,
        "relics": run.relics.iter().map(|&id| relic_view(id)).collect::<Vec<_>>(),
        "relic_slots": run.slots,
        "gold": run.gold,
        "can_start": run.draft.is_none(),
    })
}

fn saved_run_view(run: &Option<Run>) -> Value {
    run.as_ref().map_or(Value::Null, |run| {
        json!({ "floor": run.floor, "phase": match run.phase { Phase::Battle => "battle", Phase::Camp => "camp" } })
    })
}

fn upgrades_view(meta: &Meta) -> Value {
    let mut slots: Vec<Value> = UpgradeId::all()
        .map(|id| {
            let def = id.def();
            let next = meta.next_cost(id);
            json!({
                "id": id.key(),
                "name": def.name,
                "text": def.text,
                "level": meta.level(id),
                "max_level": id.max_level(),
                "costs": def.costs,
                "next_cost": next,
                "affordable": next.is_some_and(|cost| cost <= meta.crowns),
            })
        })
        .collect();
    slots.resize(UPGRADE_SLOTS.max(slots.len()), Value::Null);
    json!({ "screen": "upgrades", "meta": meta_view(meta), "slots": slots })
}

fn over_view(summary: &RunSummary, meta: &Meta) -> Value {
    let mut rows = vec![json!({ "row": "floors", "crowns": summary.cleared })];
    if summary.won {
        rows.push(json!({ "row": "win", "crowns": summary.bonus }));
    }
    json!({
        "screen": "over",
        "meta": meta_view(meta),
        "summary": {
            "won": summary.won,
            "cleared": summary.cleared,
            "floors": FLOORS.len(),
            "bonus": summary.bonus,
            "crowns": summary.crowns,
            "new_best": summary.new_best,
        },
        "rows": rows,
    })
}

pub fn view(screen: &Screen, meta: &Meta) -> Value {
    match screen {
        Screen::Title { run } => json!({
            "screen": "title",
            "meta": meta_view(meta),
            "floors": FLOORS.len(),
            "run": saved_run_view(run),
            "can_continue": run.is_some(),
        }),
        Screen::Upgrades { .. } => upgrades_view(meta),
        Screen::Battle { run, battle, .. } => battle_view(run, battle, meta),
        Screen::Camp { run, reward } => camp_view(run, reward, meta),
        Screen::Over { summary } => over_view(summary, meta),
    }
}

/// The run of the screen in the shape of the saved data. The debug command `view` with
/// `"run": true` adds it, thus a test can read the run.
pub fn run_data(screen: &Screen) -> Value {
    screen.run().map_or(Value::Null, run_json)
}

/// The debug state: the tuning with its limits, and the numbers of the run and of the meta that
/// a debug menu shows. `debug_state` and each debug command give it in `data.debug`.
pub fn debug_data(screen: &Screen, meta: &Meta, tuning: &Tuning) -> Value {
    let keys = |ids: &[RelicId]| ids.iter().map(|id| id.key()).collect::<Vec<_>>();
    let run = screen.run().map(|run| {
        json!({
            "seed": run.seed,
            "floor": run.floor,
            "gold": run.gold,
            "relics": keys(&run.relics),
            "relic_slots": run.slots,
            "traits": keys(&run.enemy.traits),
        })
    });
    let upgrades: serde_json::Map<String, Value> =
        meta.upgrades.iter().map(|(id, &level)| (id.key().to_string(), Value::from(level))).collect();
    let floors: Vec<Value> = FLOORS
        .iter()
        .zip(&tuning.floors)
        .enumerate()
        .map(|(i, (def, floor))| {
            json!({
                "number": i + 1,
                "name": def.name,
                "level": floor.level,
                "level_name": chess::level_name(floor.level),
                "budget": floor.budget,
                "traits": floor.traits,
            })
        })
        .collect();
    let kinds: Vec<Value> = RECRUIT_KINDS
        .iter()
        .zip(&tuning.kinds)
        .map(|(&kind, k)| {
            json!({
                "kind": letter(kind),
                "cap": k.cap,
                "cap_max": tuning::cap_max(kind),
                "weight": k.weight,
                "min_floor": k.min_floor,
            })
        })
        .collect();
    json!({
        "seed": tuning.seed,
        "relic_slots": tuning.relic_slots,
        "run": run,
        "meta": { "crowns": meta.crowns, "upgrades": upgrades },
        "barred": keys(&tuning.barred),
        "floors": floors,
        "kinds": kinds,
        "limits": {
            "seed": SEED_MAX,
            "level": chess::LEVELS,
            "budget": BUDGET_MAX,
            "traits": TRAITS_MAX,
            "weight": WEIGHT_MAX,
            "relics": RELICS_MAX,
        },
        "tuned": tuning.tuned(),
    })
}

/// The data of `hello`: the version, the names of the protocol, and the content tables.
pub fn hello(debug: bool) -> Value {
    let relics: Vec<Value> = RelicId::all()
        .map(|id| {
            let def = id.def();
            json!({
                "id": def.key,
                "name": def.name,
                "text": def.text,
                "foe_text": def.foe_text,
                "trait": id.is_trait(),
            })
        })
        .collect();
    let upgrades: Vec<Value> = UPGRADES
        .iter()
        .map(|def| json!({ "id": def.key, "name": def.name, "text": def.text, "costs": def.costs }))
        .collect();
    let floors: Vec<Value> = FLOORS
        .iter()
        .enumerate()
        .map(|(i, f)| {
            json!({
                "number": i + 1,
                "name": f.name,
                "budget": f.budget,
                "traits": f.traits,
                "boss": f.boss,
                "level": chess::level_name(i + 1),
                "draft_gold": content::draft_gold(i + 1),
            })
        })
        .collect();
    let pieces: Vec<Value> = [Kind::King, Kind::Queen, Kind::Rook, Kind::Bishop, Kind::Knight, Kind::Pawn]
        .into_iter()
        .map(|k| {
            json!({
                "kind": letter(k),
                "name": content::piece_name(k),
                "value": content::gold_value(k),
                "price": (k != Kind::King).then(|| content::piece_price(k)),
            })
        })
        .collect();
    let recruits: Vec<Value> = RECRUITS
        .iter()
        .map(|r| json!({ "kind": letter(r.kind), "weight": r.weight, "min_floor": r.min_floor }))
        .collect();
    json!({
        "protocol": PROTOCOL_VERSION,
        "debug": debug,
        "commands": Command::ALL.iter().map(|c| c.name()).collect::<Vec<_>>(),
        "events": EventKind::ALL.iter().map(|e| e.name()).collect::<Vec<_>>(),
        "errors": Code::ALL.iter().map(|c| c.name()).collect::<Vec<_>>(),
        "content": {
            "relics": relics,
            "upgrades": upgrades,
            "floors": floors,
            "pieces": pieces,
            "recruits": recruits,
            "relic_price": RELIC_PRICE,
            "relic_slots": RELIC_SLOTS,
            "relics_max": RELICS_MAX,
            "reroll_cost": REROLL_COST,
            "win_crowns": WIN_CROWNS,
            "army_max": ARMY_MAX,
            "upgrade_slots": UPGRADE_SLOTS,
            "upgrade_name_max": UPGRADE_NAME_MAX,
            "traits_max": content::traits_max(),
            "relic_count": RELICS.len(),
        },
    })
}
