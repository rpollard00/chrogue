//! The names of the protocol: commands, events, and error codes. Each list is one table, and
//! `PROTOCOL.md` documents each name in it (a test checks this).

use serde_json::{Map, Value};

/// Makes an enum of names with `ALL`, `name`, and `parse`.
macro_rules! names {
    ($(#[$meta:meta])* pub enum $name:ident { $($variant:ident = $text:literal,)* }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum $name {
            $($variant,)*
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant,)*];

            pub const fn name(self) -> &'static str {
                match self {
                    $($name::$variant => $text,)*
                }
            }

            pub fn parse(text: &str) -> Option<$name> {
                $name::ALL.iter().copied().find(|item| item.name() == text)
            }
        }
    };
}
pub(crate) use names;

/// The version of the protocol. It changes when a change can break a client.
pub const PROTOCOL_VERSION: u32 = 1;

/// The largest request in bytes. A longer line gets the error `too_long`.
pub const MAX_REQUEST_BYTES: usize = 64 * 1024;

names! {
    /// The commands. The screen that accepts each one is in `session.rs`.
    pub enum Command {
        Hello = "hello",
        View = "view",
        Quit = "quit",
        NewRun = "new_run",
        ContinueRun = "continue_run",
        OpenUpgrades = "open_upgrades",
        BuyUpgrade = "buy_upgrade",
        OpenRelics = "open_relics",
        BuyRelic = "buy_relic",
        Back = "back",
        Move = "move",
        EnemyMove = "enemy_move",
        GiveUp = "give_up",
        Continue = "continue",
        TakeReward = "take_reward",
        SkipReward = "skip_reward",
        Buy = "buy",
        Reroll = "reroll",
        DiscardRelic = "discard_relic",
        Place = "place",
        StartBattle = "start_battle",
        ToTitle = "to_title",
        DebugState = "debug_state",
        DebugSetSeed = "debug_set_seed",
        DebugSetRelicSlots = "debug_set_relic_slots",
        DebugTune = "debug_tune",
        DebugSetCrowns = "debug_set_crowns",
        DebugSetUpgrade = "debug_set_upgrade",
        DebugSetUnlock = "debug_set_unlock",
        DebugSetFloor = "debug_set_floor",
        DebugSetGold = "debug_set_gold",
        DebugAddUnit = "debug_add_unit",
        DebugRemoveUnit = "debug_remove_unit",
        DebugSetArmy = "debug_set_army",
        DebugSetRelic = "debug_set_relic",
        DebugSetTrait = "debug_set_trait",
        DebugSetEnemy = "debug_set_enemy",
        DebugBarRelic = "debug_bar_relic",
        DebugSetShop = "debug_set_shop",
        DebugSetDraft = "debug_set_draft",
        DebugEnemyMove = "debug_enemy_move",
        DebugAiMove = "debug_ai_move",
    }
}

impl Command {
    pub fn is_debug(self) -> bool {
        self.name().starts_with("debug_")
    }
}

names! {
    pub enum EventKind {
        Screen = "screen",
        RunStart = "run_start",
        BattleStart = "battle_start",
        Move = "move",
        Capture = "capture",
        Castle = "castle",
        EnPassant = "en_passant",
        Promote = "promote",
        Check = "check",
        UnitLost = "unit_lost",
        UnitRescued = "unit_rescued",
        Relic = "relic",
        Result = "result",
        CampEnter = "camp_enter",
        CampAction = "camp_action",
        UnitPlaced = "unit_placed",
        UpgradeBought = "upgrade_bought",
        RelicUnlocked = "relic_unlocked",
        RunEnd = "run_end",
        DebugChanged = "debug_changed",
        SaveFailed = "save_failed",
        SaveProblem = "save_problem",
        Quit = "quit",
    }
}

names! {
    pub enum Code {
        BadJson = "bad_json",
        BadRequest = "bad_request",
        TooLong = "too_long",
        UnknownCommand = "unknown_command",
        DebugDisabled = "debug_disabled",
        BadArgs = "bad_args",
        WrongScreen = "wrong_screen",
        WrongPhase = "wrong_phase",
        IllegalMove = "illegal_move",
        PromoRequired = "promo_required",
        NoRun = "no_run",
        RewardPending = "reward_pending",
        RewardClosed = "reward_closed",
        Blocked = "blocked",
        NotAffordable = "not_affordable",
        MaxLevel = "max_level",
        BadIndex = "bad_index",
        Internal = "internal",
    }
}

/// A refused command: an error code and a message for a person.
#[derive(Clone, Debug, PartialEq)]
pub struct Fail {
    pub code: Code,
    pub message: String,
}

impl Fail {
    pub fn new(code: Code, message: impl Into<String>) -> Fail {
        Fail { code, message: message.into() }
    }
}

pub fn fail<T>(code: Code, message: impl Into<String>) -> Result<T, Fail> {
    Err(Fail::new(code, message))
}

/// Makes an event: the fields of an object, with the name in `type`.
pub fn event(kind: EventKind, fields: Value) -> Value {
    let mut map = match fields {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    map.insert("type".into(), Value::from(kind.name()));
    Value::Object(map)
}

/// A JSON number for gold that can have a fraction: a whole number has no fraction part.
pub fn gold_number(gold: f64) -> Value {
    if gold.fract() == 0.0 && gold.abs() < 9e15 { Value::from(gold as i64) } else { Value::from(gold) }
}
