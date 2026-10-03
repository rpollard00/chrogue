//! A session: the state of the game, its storage, and the one entry point `Session::command`.
//!
//! A command runs on a copy of the state. The copy replaces the state only if the command
//! succeeds, thus a refused command (or a panic in the engine) changes nothing.

use std::panic::{AssertUnwindSafe, catch_unwind};

use serde_json::{Map, Value, json};

use crate::battle::{Battle, BattlePhase, FindError, MoveReport, Next};
use crate::chess::{self, Kind, Move, Special, Square};
use crate::content::{self, FLOORS, RECRUIT_KINDS, RelicId, UpgradeId};
use crate::protocol::{Code, Command, EventKind, Fail, MAX_REQUEST_BYTES, event, fail, gold_number};
use crate::random::Dice;
use crate::run::{
    Blocked, ENEMY_PIECES_MAX, EnemyPiece, Meta, Offer, Phase, Run, RunSummary, SEED_MAX, UNIT_ID_MAX, Unit, UnitId,
    generate_enemy,
};
use crate::save::{self, Doc, Storage};
use crate::tuning::{Target, Tuning};
use crate::view;

/// The reward cards of one camp visit and the card that the player took. It is None when the
/// visit has no reward.
#[derive(Clone, Debug)]
pub struct Reward {
    pub offers: Vec<Offer>,
    pub taken: Option<usize>,
}

/// What a completed battle did to the run. The session saves it when the battle ends, thus a
/// client that starts again continues after the battle and cannot play the battle a second time.
#[derive(Clone, Debug)]
pub enum Settled {
    /// The run in the camp before its next floor.
    Camp(Run),
    /// The run ended. `run` is the run after the battle.
    Over { summary: RunSummary, run: Run },
}

/// The current screen and the data that only this screen has.
/// The title and the upgrades screen keep the run in progress, if one is saved.
#[derive(Clone, Debug)]
pub enum Screen {
    Title {
        run: Option<Run>,
    },
    Upgrades {
        run: Option<Run>,
    },
    /// `run` is the run at the start of the battle: the view of the battle shows it until
    /// `continue`. `settled` is None until the battle has a result.
    Battle {
        run: Run,
        battle: Box<Battle>,
        settled: Option<Box<Settled>>,
    },
    Camp {
        run: Run,
        reward: Option<Reward>,
    },
    Over {
        summary: RunSummary,
    },
}

impl Screen {
    pub const fn name(&self) -> &'static str {
        match self {
            Screen::Title { .. } => "title",
            Screen::Upgrades { .. } => "upgrades",
            Screen::Battle { .. } => "battle",
            Screen::Camp { .. } => "camp",
            Screen::Over { .. } => "over",
        }
    }

    /// The run in progress: the run that the session saves. After a battle has a result, it is
    /// the run after the battle, or None if the run ended.
    pub fn run(&self) -> Option<&Run> {
        match self {
            Screen::Title { run } | Screen::Upgrades { run } => run.as_ref(),
            Screen::Battle { settled: Some(settled), .. } => match settled.as_ref() {
                Settled::Camp(run) => Some(run),
                Settled::Over { .. } => None,
            },
            Screen::Battle { run, .. } | Screen::Camp { run, .. } => Some(run),
            Screen::Over { .. } => None,
        }
    }

    fn run_mut(&mut self) -> Option<&mut Run> {
        match self {
            Screen::Title { run } | Screen::Upgrades { run } => run.as_mut(),
            Screen::Battle { settled: Some(settled), .. } => match settled.as_mut() {
                Settled::Camp(run) => Some(run),
                Settled::Over { .. } => None,
            },
            Screen::Battle { run, .. } | Screen::Camp { run, .. } => Some(run),
            Screen::Over { .. } => None,
        }
    }

    fn take_run(&mut self) -> Option<Run> {
        match std::mem::replace(self, Screen::Title { run: None }) {
            Screen::Title { run } | Screen::Upgrades { run } => run,
            Screen::Battle { settled: Some(settled), .. } => match *settled {
                Settled::Camp(run) => Some(run),
                Settled::Over { .. } => None,
            },
            Screen::Battle { run, .. } | Screen::Camp { run, .. } => Some(run),
            Screen::Over { .. } => None,
        }
    }
}

/// The state that a command changes. A command works on a clone of it.
#[derive(Clone, Debug)]
struct Game {
    meta: Meta,
    screen: Screen,
    /// The dice of the session. They make only the seed of each new run.
    dice: Dice,
    /// The debug settings of the session.
    tuning: Tuning,
    quit: bool,
}

/// What a successful command did.
#[derive(Default)]
struct Done {
    events: Vec<Value>,
    data: Option<Value>,
    save_meta: bool,
    /// Save the run of the screen, or clear the saved run if the screen has none.
    save_run: bool,
}

impl Done {
    fn push(&mut self, kind: EventKind, fields: Value) {
        self.events.push(event(kind, fields));
    }
}

pub struct Session {
    game: Game,
    storage: Box<dyn Storage>,
    debug: bool,
    /// The `save_problem` events of the load. The first successful response has them.
    pending: Vec<Value>,
    /// The documents that the core could not use and could not set aside. The session does not
    /// write or remove them.
    frozen: Vec<Doc>,
}

/// The message of `save_failed` for a frozen document.
const FROZEN: &str = "The core does not write over a saved file that it could not read or set aside";

impl Session {
    /// A session with the saved data of the storage, on the title screen.
    pub fn new(storage: Box<dyn Storage>, seed: u64) -> Session {
        Session::with_debug(storage, seed, false)
    }

    /// A session that also accepts the debug commands if `debug` is true. A saved document that
    /// the core cannot use is set aside before the session writes anything, and the first
    /// successful response tells it with a `save_problem` event.
    pub fn with_debug(mut storage: Box<dyn Storage>, seed: u64, debug: bool) -> Session {
        let loaded = save::load(storage.as_mut());
        let mut pending = Vec::new();
        let mut frozen = Vec::new();
        for problem in loaded.problems {
            let (kept, message) = match storage.set_aside(problem.doc) {
                Ok(kept) => (kept, problem.message),
                Err(error) => {
                    frozen.push(problem.doc);
                    let what = problem.doc.name();
                    (
                        None,
                        format!(
                            "{}. The core could not set the file aside ({error}) and does not save the {what}",
                            problem.message
                        ),
                    )
                }
            };
            pending.push(event(
                EventKind::SaveProblem,
                json!({ "what": problem.doc.name(), "reason": problem.reason.code(), "message": message, "kept": kept }),
            ));
        }
        let screen = Screen::Title { run: loaded.run };
        let game = Game { meta: loaded.meta, screen, dice: Dice::new(seed), tuning: Tuning::default(), quit: false };
        Session { game, storage, debug, pending, frozen }
    }

    pub fn screen(&self) -> &Screen {
        &self.game.screen
    }

    pub fn meta(&self) -> &Meta {
        &self.game.meta
    }

    /// True after the command `quit`.
    pub fn wants_quit(&self) -> bool {
        self.game.quit
    }

    pub fn view(&self) -> Value {
        view::view(&self.game.screen, &self.game.meta)
    }

    /// Runs one request and returns the response. See `PROTOCOL.md`. This function never panics.
    pub fn command(&mut self, request: &str) -> String {
        if request.len() > MAX_REQUEST_BYTES {
            return self.too_long();
        }
        let parsed: Value = match serde_json::from_str(request) {
            Ok(value) => value,
            Err(e) => return self.reply_error(&Value::Null, Fail::new(Code::BadJson, e.to_string())),
        };
        let Value::Object(args) = parsed else {
            return self.reply_error(&Value::Null, Fail::new(Code::BadRequest, "The request must be a JSON object"));
        };
        let id = args.get("id").cloned().unwrap_or(Value::Null);
        let Some(name) = args.get("cmd").and_then(Value::as_str) else {
            return self.reply_error(&id, Fail::new(Code::BadRequest, "The request has no \"cmd\" string"));
        };
        let Some(command) = Command::parse(name) else {
            return self.reply_error(&id, Fail::new(Code::UnknownCommand, format!("Unknown command \"{name}\"")));
        };
        if command.is_debug() && !self.debug {
            return self.reply_error(&id, Fail::new(Code::DebugDisabled, "The session has no debug commands"));
        }
        let mut next = self.game.clone();
        let debug = self.debug;
        let result = catch_unwind(AssertUnwindSafe(|| apply(&mut next, command, &args, debug)));
        match result {
            Ok(Ok(mut done)) => {
                self.game = next;
                self.persist(&mut done);
                let mut events = std::mem::take(&mut self.pending);
                events.append(&mut done.events);
                let mut reply =
                    format!("{{\"ok\":true,\"id\":{},\"events\":{},\"view\":{}", id, Value::Array(events), self.view());
                if let Some(data) = done.data {
                    reply.push_str(&format!(",\"data\":{data}"));
                }
                reply.push('}');
                reply
            }
            Ok(Err(fail)) => self.reply_error(&id, fail),
            Err(_) => self.reply_error(&id, Fail::new(Code::Internal, "The command failed inside the core")),
        }
    }

    /// The response to a request that is longer than `MAX_REQUEST_BYTES`.
    pub fn too_long(&self) -> String {
        let message = format!("The request is longer than {MAX_REQUEST_BYTES} bytes");
        self.reply_error(&Value::Null, Fail::new(Code::TooLong, message))
    }

    fn reply_error(&self, id: &Value, fail: Fail) -> String {
        let error = json!({ "code": fail.code.name(), "message": fail.message });
        format!("{{\"ok\":false,\"id\":{},\"error\":{},\"view\":{}}}", id, error, self.view())
    }

    fn persist(&mut self, done: &mut Done) {
        // The text of each document to save, or None to remove the saved run.
        let meta = done.save_meta.then(|| Some(save::meta_document(&self.game.meta)));
        let run = done.save_run.then(|| self.game.screen.run().map(save::run_document));
        for (doc, text) in [(Doc::Meta, meta), (Doc::Run, run)] {
            let Some(text) = text else { continue };
            let result = if self.frozen.contains(&doc) {
                Err(FROZEN.to_string())
            } else {
                match text {
                    Some(text) => self.storage.save(doc, &text),
                    None => self.storage.remove(doc),
                }
            };
            if let Err(message) = result {
                done.push(EventKind::SaveFailed, json!({ "what": doc.name(), "message": message }));
            }
        }
    }
}

// ---- Arguments ----

type Args = Map<String, Value>;

fn bad<T>(message: impl Into<String>) -> Result<T, Fail> {
    fail(Code::BadArgs, message)
}

fn uint(args: &Args, name: &str, max: u64) -> Result<u64, Fail> {
    match args.get(name).and_then(Value::as_u64) {
        Some(n) if n <= max => Ok(n),
        _ => bad(format!("\"{name}\" must be a whole number from 0 to {max}")),
    }
}

fn int(args: &Args, name: &str) -> Result<i64, Fail> {
    args.get(name).and_then(Value::as_i64).map_or_else(|| bad(format!("\"{name}\" must be a whole number")), Ok)
}

fn square(args: &Args, name: &str) -> Result<Square, Fail> {
    uint(args, name, 63).map(|s| s as Square)
}

fn string<'a>(args: &'a Args, name: &str) -> Result<&'a str, Fail> {
    args.get(name).and_then(Value::as_str).map_or_else(|| bad(format!("\"{name}\" must be a string")), Ok)
}

fn boolean(args: &Args, name: &str) -> Result<bool, Fail> {
    args.get(name).and_then(Value::as_bool).map_or_else(|| bad(format!("\"{name}\" must be true or false")), Ok)
}

fn kind_of(value: &Value) -> Option<Kind> {
    let text = value.as_str()?;
    if text.len() != 1 {
        return None;
    }
    chess::kind_from_letter(text.chars().next()?)
}

fn recruit(value: Option<&Value>, name: &str) -> Result<Kind, Fail> {
    match value.and_then(kind_of) {
        Some(kind) if RECRUIT_KINDS.contains(&kind) => Ok(kind),
        _ => bad(format!("\"{name}\" must be one of \"p\", \"n\", \"b\", \"r\", \"q\"")),
    }
}

fn promo(args: &Args) -> Result<Option<Kind>, Fail> {
    match args.get("promo") {
        None | Some(Value::Null) => Ok(None),
        Some(value) => match kind_of(value) {
            Some(kind @ (Kind::Queen | Kind::Rook | Kind::Bishop | Kind::Knight)) => Ok(Some(kind)),
            _ => bad("\"promo\" must be one of \"q\", \"r\", \"b\", \"n\""),
        },
    }
}

fn relic(args: &Args, name: &str) -> Result<RelicId, Fail> {
    let key = string(args, name)?;
    RelicId::parse(key).map_or_else(|| bad(format!("Unknown relic \"{key}\"")), Ok)
}

fn index(args: &Args) -> Result<usize, Fail> {
    uint(args, "index", 1 << 20).map(|i| i as usize)
}

fn array<'a>(args: &'a Args, name: &str) -> Result<&'a Vec<Value>, Fail> {
    args.get(name).and_then(Value::as_array).map_or_else(|| bad(format!("\"{name}\" must be an array")), Ok)
}

fn offers(value: &[Value]) -> Result<Vec<Offer>, Fail> {
    if value.len() > 16 {
        return bad("A list of offers has at most 16 items");
    }
    value.iter().map(|o| save::parse_offer(o).map_or_else(|| bad(format!("Not an offer: {o}")), Ok)).collect()
}

fn traits(value: &[Value]) -> Result<Vec<RelicId>, Fail> {
    let mut ids = Vec::new();
    for v in value {
        let id = v.as_str().and_then(RelicId::parse).filter(|id| id.is_trait());
        match id {
            Some(id) if !ids.contains(&id) => ids.push(id),
            _ => return bad(format!("Not a trait, or a trait two times: {v}")),
        }
    }
    if ids.len() > content::traits_max() {
        return fail(Code::Blocked, format!("The enemy has at most {} traits", content::traits_max()));
    }
    Ok(ids)
}

// ---- Commands ----

fn wrong_screen<T>(command: Command, screen: &Screen) -> Result<T, Fail> {
    fail(Code::WrongScreen, format!("\"{}\" is not a command of the {} screen", command.name(), screen.name()))
}

fn screen_event(done: &mut Done, screen: &Screen) {
    done.push(EventKind::Screen, json!({ "name": screen.name() }));
}

fn start_battle(game: &mut Game, mut run: Run, done: &mut Done) -> Result<(), Fail> {
    run.phase = Phase::Battle;
    // Saved data and the debug commands check the board, thus only the camp can get here with
    // a board that is not valid: a unit on the square of a debug enemy piece.
    let battle = Battle::new(&run).map_err(|e| Fail::new(Code::Blocked, format!("The battle cannot start: {e}")))?;
    let def = run.floor_def();
    let start = json!({ "floor": run.floor, "name": def.name, "boss": def.boss });
    game.screen = Screen::Battle { run, battle: Box::new(battle), settled: None };
    screen_event(done, &game.screen);
    done.push(EventKind::BattleStart, start);
    done.save_run = true;
    Ok(())
}

fn open_camp(game: &mut Game, run: Run, done: &mut Done) {
    let reward = run.draft.clone().map(|offers| Reward { offers, taken: None });
    let enter = json!({ "floor": run.floor, "reward": reward.is_some() });
    game.screen = Screen::Camp { run, reward };
    screen_event(done, &game.screen);
    done.push(EventKind::CampEnter, enter);
    done.save_run = true;
}

fn end_run(game: &mut Game, run: Run, won: bool, debug: bool, done: &mut Done) {
    let summary = game.meta.finish_run(&run, won);
    done.save_meta = true;
    show_over(game, summary, &run, debug, done);
}

/// Shows the summary of a run that ended. The permanent data has the run already.
fn show_over(game: &mut Game, summary: RunSummary, run: &Run, debug: bool, done: &mut Done) {
    let mut fields = json!({
        "won": summary.won, "cleared": summary.cleared, "bonus": summary.bonus,
        "crowns": summary.crowns, "new_best": summary.new_best,
    });
    if debug {
        fields["run"] = save::run_json(run);
    }
    game.screen = Screen::Over { summary };
    screen_event(done, &game.screen);
    done.push(EventKind::RunEnd, fields);
    done.save_run = true;
}

/// Applies a battle to the saved data when it gets its result: the run goes to the camp before
/// its next floor, or the run ends. The screen does not change until `continue`.
fn settle(game: &mut Game, done: &mut Done) {
    let Game { meta, screen, tuning, .. } = game;
    let Screen::Battle { run, battle, settled: settled @ None } = screen else { return };
    let mut after = run.clone();
    let Some(next) = battle.settle(&mut after, meta, tuning) else { return };
    *settled = Some(Box::new(match next {
        Next::Camp => Settled::Camp(after),
        next => {
            done.save_meta = true;
            Settled::Over { summary: meta.finish_run(&after, next == Next::Won), run: after }
        }
    }));
    done.save_run = true;
}

fn move_events(report: &MoveReport, battle: &Battle, done: &mut Done) {
    let MoveReport { mover, mv, .. } = *report;
    done.push(
        EventKind::Move,
        json!({
            "id": mover.id, "color": chess::color_letter(mover.color), "kind": view::letter(mover.kind),
            "from": mv.from, "to": mv.to,
        }),
    );
    if let Some((from, to)) = chess::castle_partner(&battle.state, mover.color, mv) {
        let rook = chess::piece_at(&battle.state, to).map(|p| p.id);
        done.push(EventKind::Castle, json!({ "id": rook, "from": from, "to": to }));
    }
    if let Some((piece, square, gold)) = report.capture {
        if mv.special == Special::EnPassant {
            done.push(EventKind::EnPassant, json!({ "square": square }));
        }
        done.push(
            EventKind::Capture,
            json!({
                "id": piece.id, "color": chess::color_letter(piece.color), "kind": view::letter(piece.kind),
                "square": square, "gold": gold_number(gold),
            }),
        );
    }
    if let Some((id, rescued)) = report.unit_lost {
        done.push(if rescued { EventKind::UnitRescued } else { EventKind::UnitLost }, json!({ "id": id }));
    }
    if let Some(kind) = mv.promo {
        done.push(EventKind::Promote, json!({ "id": mover.id, "square": mv.to, "kind": view::letter(kind) }));
    }
    if !report.relics.is_empty() {
        done.push(EventKind::Relic, json!({ "ids": report.relics.iter().map(|id| id.key()).collect::<Vec<_>>() }));
    }
    if let Some(square) = chess::check_square(&battle.state) {
        done.push(
            EventKind::Check,
            json!({ "square": square, "color": chess::color_letter(chess::turn(&battle.state)) }),
        );
    }
    if let Some(result) = &battle.result {
        let winner = result.outcome.winner().map(chess::color_letter);
        done.push(EventKind::Result, json!({ "winner": winner, "reason": result.outcome.reason() }));
    }
}

/// The battle of the screen, with a check of its phase.
fn battle_in<'a>(
    game: &'a mut Game,
    command: Command,
    phases: &[BattlePhase],
) -> Result<(&'a Run, &'a mut Battle), Fail> {
    let screen = &mut game.screen;
    let Screen::Battle { run, battle, .. } = screen else { return wrong_screen(command, screen) };
    let phase = battle.phase();
    if !phases.contains(&phase) {
        return fail(
            Code::WrongPhase,
            format!("\"{}\" is not a command of the phase \"{}\"", command.name(), phase.code()),
        );
    }
    Ok((run, battle))
}

fn play(game: &mut Game, command: Command, phase: BattlePhase, mv: Move, done: &mut Done) -> Result<(), Fail> {
    let (run, battle) = battle_in(game, command, &[phase])?;
    let report = battle.play(run, mv);
    move_events(&report, battle, done);
    settle(game, done);
    Ok(())
}

fn find_move(game: &mut Game, command: Command, phase: BattlePhase, args: &Args) -> Result<Move, Fail> {
    let (from, to, promo) = (square(args, "from")?, square(args, "to")?, promo(args)?);
    let (_, battle) = battle_in(game, command, &[phase])?;
    battle.find_move(from, to, promo).map_err(|e| match e {
        FindError::Illegal => Fail::new(Code::IllegalMove, format!("No legal move from {from} to {to}")),
        FindError::PromoRequired => Fail::new(Code::PromoRequired, "The move promotes: give \"promo\""),
    })
}

fn camp(game: &mut Game, command: Command) -> Result<(&mut Run, &mut Option<Reward>), Fail> {
    match &mut game.screen {
        Screen::Camp { run, reward } => Ok((run, reward)),
        screen => wrong_screen(command, screen),
    }
}

/// Runs a camp action and adds the event that tells what it changed.
fn camp_action(
    game: &mut Game,
    command: Command,
    done: &mut Done,
    action: impl FnOnce(&mut Run, &mut Option<Reward>, &Meta, &Tuning) -> Result<(), Fail>,
) -> Result<(), Fail> {
    let Game { meta, screen, tuning, .. } = game;
    let Screen::Camp { run, reward } = screen else { return wrong_screen(command, screen) };
    let (gold_before, units, relics) = (run.gold, run.army.clone(), run.relics.clone());
    action(run, reward, meta, tuning)?;
    let added: Vec<Value> = run
        .army
        .iter()
        .filter(|u| !units.iter().any(|old| old.id == u.id))
        .map(|u| json!({ "id": u.id, "kind": view::letter(u.kind), "home": u.home }))
        .collect();
    let new_relics: Vec<&str> = run.relics.iter().filter(|id| !relics.contains(id)).map(|id| id.key()).collect();
    done.push(
        EventKind::CampAction,
        json!({
            "action": command.name(), "gold_before": gold_before, "gold": run.gold,
            "units": added, "relics": new_relics, "rolled": command == Command::Reroll,
        }),
    );
    done.save_run = true;
    Ok(())
}

fn apply(game: &mut Game, command: Command, args: &Args, debug: bool) -> Result<Done, Fail> {
    let mut done = Done::default();
    let d = &mut done;
    match command {
        Command::Hello => d.data = Some(view::hello(debug)),
        Command::View => {
            if debug && args.get("run") == Some(&Value::Bool(true)) {
                d.data = Some(json!({ "run": view::run_data(&game.screen) }));
            }
        }
        Command::Quit => {
            game.quit = true;
            d.push(EventKind::Quit, json!({}));
        }
        Command::NewRun => {
            if !matches!(game.screen, Screen::Title { .. } | Screen::Over { .. }) {
                return wrong_screen(command, &game.screen);
            }
            let seed = match game.tuning.seed {
                Some(seed) => seed,
                None => game.dice.below(SEED_MAX as usize + 1) as u64,
            };
            let run = Run::new(&game.meta, seed, &game.tuning);
            d.push(EventKind::RunStart, json!({}));
            start_battle(game, run, d)?;
        }
        Command::ContinueRun => {
            let Screen::Title { run } = &mut game.screen else { return wrong_screen(command, &game.screen) };
            let Some(run) = run.take() else { return fail(Code::NoRun, "No run is saved") };
            match run.phase {
                Phase::Camp => open_camp(game, run, d),
                Phase::Battle => start_battle(game, run, d)?,
            }
        }
        Command::OpenUpgrades => {
            if !matches!(game.screen, Screen::Title { .. } | Screen::Over { .. }) {
                return wrong_screen(command, &game.screen);
            }
            game.screen = Screen::Upgrades { run: game.screen.take_run() };
            screen_event(d, &game.screen);
        }
        Command::Back => {
            let Screen::Upgrades { run } = &mut game.screen else { return wrong_screen(command, &game.screen) };
            game.screen = Screen::Title { run: run.take() };
            screen_event(d, &game.screen);
        }
        Command::BuyUpgrade => {
            let key = string(args, "upgrade")?;
            let Some(id) = UpgradeId::parse(key) else { return bad(format!("Unknown upgrade \"{key}\"")) };
            if !matches!(game.screen, Screen::Upgrades { .. }) {
                return wrong_screen(command, &game.screen);
            }
            let before = game.meta.crowns;
            game.meta.buy_upgrade(id)?;
            d.push(
                EventKind::UpgradeBought,
                json!({ "id": id.key(), "level": game.meta.level(id), "crowns_before": before, "crowns": game.meta.crowns }),
            );
            d.save_meta = true;
        }
        Command::Move => {
            let mv = find_move(game, command, BattlePhase::Player, args)?;
            play(game, command, BattlePhase::Player, mv, d)?;
        }
        Command::DebugEnemyMove => {
            let mv = find_move(game, command, BattlePhase::Enemy, args)?;
            play(game, command, BattlePhase::Enemy, mv, d)?;
        }
        Command::EnemyMove => {
            let Game { screen, tuning, .. } = game;
            let Screen::Battle { run, battle, .. } = screen else { return wrong_screen(command, screen) };
            if battle.phase() != BattlePhase::Enemy {
                return fail(Code::WrongPhase, "The enemy does not have the move");
            }
            let Some(mv) = battle.ai_move(run, tuning.floor(run.floor).level) else {
                return fail(Code::Internal, "The enemy has no legal move");
            };
            play(game, command, BattlePhase::Enemy, mv, d)?;
        }
        Command::DebugAiMove => {
            let level = match args.get("level") {
                None | Some(Value::Null) => None,
                Some(_) => Some(uint(args, "level", FLOORS.len() as u64)?.max(1) as usize),
            };
            let Game { screen, tuning, .. } = game;
            let Screen::Battle { run, battle, .. } = screen else { return wrong_screen(command, screen) };
            let phase = battle.phase();
            if phase == BattlePhase::Over {
                return fail(Code::WrongPhase, "The battle is over");
            }
            // A battle can start with no legal move for the player (see PROTOCOL.md).
            let Some(mv) = battle.ai_move(run, level.unwrap_or(tuning.floor(run.floor).level)) else {
                return fail(Code::IllegalMove, "The side to move has no legal move");
            };
            play(game, command, phase, mv, d)?;
        }
        Command::GiveUp => {
            battle_in(game, command, &[BattlePhase::Player, BattlePhase::Enemy])?;
            let Some(run) = game.screen.take_run() else { return fail(Code::Internal, "The battle has no run") };
            end_run(game, run, false, debug, d);
        }
        Command::Continue => {
            battle_in(game, command, &[BattlePhase::Over])?;
            let Screen::Battle { settled: Some(settled), .. } =
                std::mem::replace(&mut game.screen, Screen::Title { run: None })
            else {
                return fail(Code::Internal, "The battle has no result");
            };
            match *settled {
                Settled::Camp(run) => open_camp(game, run, d),
                Settled::Over { summary, run } => show_over(game, summary, &run, debug, d),
            }
        }
        Command::ToTitle => {
            if !matches!(game.screen, Screen::Battle { .. } | Screen::Camp { .. } | Screen::Over { .. }) {
                return wrong_screen(command, &game.screen);
            }
            game.screen = Screen::Title { run: game.screen.take_run() };
            screen_event(d, &game.screen);
        }
        Command::TakeReward => {
            let i = index(args)?;
            camp_action(game, command, d, |run, reward, meta, _| {
                run.take_draft(meta, i)?;
                if let Some(reward) = reward {
                    reward.taken = Some(i);
                }
                Ok(())
            })?;
        }
        Command::SkipReward => camp_action(game, command, d, |run, _, _, _| {
            if run.draft.take().is_none() {
                return fail(Code::RewardClosed, "The camp has no reward to skip");
            }
            Ok(())
        })?,
        Command::Buy => {
            let i = index(args)?;
            camp_action(game, command, d, |run, _, meta, _| run.buy_offer(meta, i).map(|_| ()))?;
        }
        Command::Reroll => camp_action(game, command, d, |run, _, meta, tuning| run.reroll_shop(meta, tuning))?,
        Command::Place => {
            let unit = uint(args, "unit", u16::MAX as u64)? as UnitId;
            let to = uint(args, "square", 15)? as Square;
            let (run, _) = camp(game, command)?;
            let Some(from) = run.army.iter().find(|u| u.id == unit).map(|u| u.home) else {
                return bad(format!("The army has no unit {unit}"));
            };
            let swapped = run.move_unit(from, to);
            d.push(EventKind::UnitPlaced, json!({ "id": unit, "from": from, "to": to, "swapped": swapped }));
            d.save_run = true;
        }
        Command::StartBattle => {
            let (run, _) = camp(game, command)?;
            if run.draft.is_some() {
                return fail(Code::RewardPending, "Select or skip the reward before the battle");
            }
            let Some(run) = game.screen.take_run() else { return fail(Code::Internal, "The camp has no run") };
            start_battle(game, run, d)?;
        }
        _ => debug_command(game, command, args, d)?,
    }
    if command.is_debug() {
        done.data = Some(json!({ "debug": view::debug_data(&game.screen, &game.meta, &game.tuning) }));
    }
    Ok(done)
}

// ---- Debug commands ----

/// A debug change of the saved data or of the tuning. After a change of the meta, of the run, or
/// of the enemy armies of the tuning, a battle with no result starts again.
fn debug_command(game: &mut Game, command: Command, args: &Args, d: &mut Done) -> Result<(), Fail> {
    let mut what = command.name().trim_start_matches("debug_set_").trim_start_matches("debug_");
    let mut restart = true;
    match command {
        Command::DebugState => return Ok(()),
        Command::DebugSetCrowns => {
            game.meta.crowns = uint(args, "crowns", 1_000_000_000)?;
            d.save_meta = true;
        }
        Command::DebugSetUpgrade => {
            let key = string(args, "upgrade")?;
            let Some(id) = UpgradeId::parse(key) else { return bad(format!("Unknown upgrade \"{key}\"")) };
            let level = int(args, "level")?;
            game.meta.set_level(id, level.max(0) as u64);
            d.save_meta = true;
        }
        Command::DebugBarRelic => {
            let id = relic(args, "relic")?;
            game.tuning.bar(id, boolean(args, "barred")?);
            (what, restart) = ("barred", false);
        }
        Command::DebugSetSeed => {
            game.tuning.seed = match args.get("seed") {
                Some(Value::Null) => None,
                _ => Some(uint(args, "seed", SEED_MAX)?),
            };
            restart = false;
        }
        Command::DebugTune => {
            let Game { screen, tuning, .. } = game;
            let before = tuning.clone();
            tune(tuning, args)?;
            // A change of only the levels of the AI keeps the enemy of the run and its battle.
            restart = !tuning.same_armies(&before);
            if restart && let Some(run) = screen.run_mut() {
                run.enemy = generate_enemy(run.seed, run.floor, tuning);
                Battle::new(run).map_err(|e| Fail::new(Code::BadArgs, format!("The board is not valid: {e}")))?;
                d.save_run = true;
            }
            what = "tuning";
        }
        _ => {
            let Game { screen, tuning, .. } = game;
            let Some(run) = screen.run_mut() else { return fail(Code::NoRun, "No run is in progress") };
            debug_run(run, command, args, tuning)?;
            // A command that changes the pieces or their rules must leave a board that the engine
            // takes as it is: no two pieces on one square, one king on each side. A check of the
            // enemy king at the start is permitted here, as in the camp.
            let board = matches!(
                command,
                Command::DebugSetArmy
                    | Command::DebugSetEnemy
                    | Command::DebugAddUnit
                    | Command::DebugRemoveUnit
                    | Command::DebugSetFloor
                    | Command::DebugSetRelic
                    | Command::DebugSetTrait
            );
            if board {
                Battle::new(run).map_err(|e| Fail::new(Code::BadArgs, format!("The board is not valid: {e}")))?;
            }
            if command == Command::DebugSetDraft
                && let Screen::Camp { run, reward } = screen
            {
                *reward = run.draft.clone().map(|offers| Reward { offers, taken: None });
            }
            d.save_run = true;
        }
    }
    d.push(EventKind::DebugChanged, json!({ "what": what }));
    // A battle with a result is in the saved data already, thus its screen stays.
    if restart && matches!(game.screen, Screen::Battle { settled: None, .. }) {
        let Some(run) = game.screen.take_run() else { return fail(Code::Internal, "The battle has no run") };
        start_battle(game, run, d)?;
    }
    Ok(())
}

/// Applies the arguments of `debug_tune`: a reset, or numbers of one floor or of one kind.
fn tune(tuning: &mut Tuning, args: &Args) -> Result<(), Fail> {
    let given = |name: &str| args.get(name).filter(|value| !value.is_null());
    let reset = given("reset").is_some() && boolean(args, "reset")?;
    let target = match (reset, given("floor"), given("kind")) {
        (true, None, None) => {
            tuning.reset();
            return Ok(());
        }
        (false, Some(_), None) => Target::Floor(uint(args, "floor", FLOORS.len() as u64)? as usize),
        (false, None, Some(kind)) => Target::Kind(recruit(Some(kind), "kind")?),
        _ => return bad("Give one of \"reset\" (true), \"floor\", or \"kind\""),
    };
    let mut values = Vec::new();
    for &field in target.fields() {
        if let Some(value) = given(field.name()) {
            let Some(number) = value.as_f64() else { return bad(format!("\"{}\" must be a number", field.name())) };
            values.push((field, number));
        }
    }
    tuning.tune(target, &values).map_err(|message| Fail::new(Code::BadArgs, message))
}

fn debug_run(run: &mut Run, command: Command, args: &Args, tuning: &Tuning) -> Result<(), Fail> {
    match command {
        Command::DebugSetFloor => {
            let floor = uint(args, "floor", FLOORS.len() as u64)? as usize;
            if floor < 1 {
                return bad(format!("\"floor\" must be from 1 to {}", FLOORS.len()));
            }
            run.floor = floor;
            run.enemy = generate_enemy(run.seed, floor, tuning);
        }
        Command::DebugSetGold => run.gold = uint(args, "gold", 1_000_000_000)?,
        Command::DebugAddUnit => {
            let kind = recruit(args.get("kind"), "kind")?;
            if run.add_unit(kind).is_none() {
                return fail(Code::Blocked, "The army is full");
            }
        }
        Command::DebugRemoveUnit => {
            let id = uint(args, "unit", u16::MAX as u64)? as UnitId;
            match run.army.iter().position(|u| u.id == id) {
                Some(i) if run.army[i].kind != Kind::King => {
                    run.army.remove(i);
                }
                Some(_) => return fail(Code::Blocked, "The king cannot leave the army"),
                None => return bad(format!("The army has no unit {id}")),
            }
        }
        Command::DebugSetArmy => {
            let list = array(args, "units")?;
            if list.is_empty() || list.len() > content::ARMY_MAX {
                return bad(format!("\"units\" must have 1 to {} units", content::ARMY_MAX));
            }
            let mut army: Vec<Unit> = Vec::new();
            for (i, unit) in list.iter().enumerate() {
                let Value::Object(unit) = unit else { return bad("A unit must be an object") };
                let id = match unit.get("id") {
                    None => i as u64 + 1,
                    Some(_) => uint(unit, "id", UNIT_ID_MAX as u64)?,
                } as UnitId;
                let kind =
                    unit.get("kind").and_then(kind_of).map_or_else(|| bad("\"kind\" must be a piece letter"), Ok)?;
                let home = uint(unit, "home", 15)? as Square;
                if army.iter().any(|u| u.id == id || u.home == home) {
                    return bad("Two units have the same id or the same home");
                }
                army.push(Unit { id, kind, home });
            }
            if army.iter().filter(|u| u.kind == Kind::King).count() != 1 {
                return bad("The army must have one king");
            }
            run.next_id = army.iter().map(|u| u.id).max().unwrap_or(0) + 1;
            run.army = army;
        }
        Command::DebugSetRelic => {
            let id = relic(args, "relic")?;
            let on = boolean(args, "on")?;
            if on && Offer::Relic(id).blocked(run) == Some(Blocked::RelicsFull) {
                return fail(Code::Blocked, format!("The run has at most {} relics", content::RELICS_MAX));
            }
            set_relic(&mut run.relics, id, on);
        }
        Command::DebugSetTrait => {
            let id = relic(args, "relic")?;
            let on = boolean(args, "on")?;
            if !id.is_trait() {
                return bad(format!("\"{}\" is not a trait", id.key()));
            }
            if on && !run.enemy.traits.contains(&id) && run.enemy.traits.len() >= content::traits_max() {
                return fail(Code::Blocked, format!("The enemy has at most {} traits", content::traits_max()));
            }
            set_relic(&mut run.enemy.traits, id, on);
        }
        Command::DebugSetEnemy => {
            let list = array(args, "pieces")?;
            if list.len() > ENEMY_PIECES_MAX {
                return bad(format!("The enemy has at most {ENEMY_PIECES_MAX} pieces"));
            }
            let mut pieces: Vec<EnemyPiece> = Vec::new();
            for piece in list {
                let Value::Object(piece) = piece else { return bad("A piece must be an object") };
                let kind =
                    piece.get("kind").and_then(kind_of).map_or_else(|| bad("\"kind\" must be a piece letter"), Ok)?;
                let square = square(piece, "square")?;
                if pieces.iter().any(|p| p.square == square) {
                    return bad("Two enemy pieces have the same square");
                }
                pieces.push(EnemyPiece { kind, square });
            }
            if pieces.iter().filter(|p| p.kind == Kind::King).count() != 1 {
                return bad("The enemy must have one king");
            }
            let traits = match args.get("traits") {
                None => run.enemy.traits.clone(),
                Some(_) => traits(array(args, "traits")?)?,
            };
            run.enemy.pieces = pieces;
            run.enemy.traits = traits;
        }
        Command::DebugSetShop => run.shop = offers(array(args, "offers")?)?,
        Command::DebugSetDraft => {
            run.draft = match args.get("offers") {
                Some(Value::Null) => None,
                _ => Some(offers(array(args, "offers")?)?),
            }
        }
        _ => return fail(Code::Internal, format!("\"{}\" has no handler", command.name())),
    }
    Ok(())
}

/// Adds a relic to a list, or removes it (`setRelic`).
fn set_relic(list: &mut Vec<RelicId>, id: RelicId, on: bool) {
    let at = list.iter().position(|&other| other == id);
    match (on, at) {
        (true, None) => list.push(id),
        (false, Some(i)) => {
            list.remove(i);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each command has a handler on each screen: a refusal is never `internal`.
    #[test]
    fn each_command_has_a_handler() {
        let run = Run::new(&Meta::default(), 1, &Tuning::default());
        let battle = Box::new(Battle::new(&run).unwrap());
        let screens = [
            Screen::Title { run: None },
            Screen::Title { run: Some(run.clone()) },
            Screen::Upgrades { run: None },
            Screen::Battle { run: run.clone(), battle, settled: None },
            Screen::Camp { run, reward: None },
            Screen::Over { summary: RunSummary { won: false, cleared: 0, bonus: 0, crowns: 0, new_best: false } },
        ];
        for screen in screens {
            for &command in Command::ALL {
                let mut game = Game {
                    meta: Meta::default(),
                    screen: screen.clone(),
                    dice: Dice::new(1),
                    tuning: Tuning::default(),
                    quit: false,
                };
                if let Err(fail) = apply(&mut game, command, &Args::new(), true) {
                    assert_ne!(fail.code, Code::Internal, "{} on {}: {}", command.name(), screen.name(), fail.message);
                }
            }
        }
    }
}
