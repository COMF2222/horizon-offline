// Copyright (c) 2026 d3grando
// SPDX-License-Identifier: MIT
// TreeFall: cutting one log of a natural tree fells the whole tree.
//
// Original implementation for Horizon (a horizon-mixins native mod). This file is
// both the standalone, unit-tested Cargo library and the injected
// `crate::game::treefall` module. It has no Horizon dependency: the game is
// reached only through the small `Blocks`/`Host` traits, which `native.rs`
// implements for the running game.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub type Position = [i32; 3];
pub type Actor = [i32; 4];

/// The mod's id: its row in Options > Mods and its settings folder
/// (`horizon-mods/.horizon-mod-data/treefall/`).
pub const ID: &str = "treefall";
pub const VERSION: &str = "0.3.0";
pub const DESCRIPTION: &str =
    "Cut one log of a natural tree with an axe and the whole tree comes down.";
/// The configuration, in the mod's settings folder: what the settings page
/// writes, and where the woods lists are edited by hand.
pub const CONFIG_FILE: &str = "config.json";
/// The chat command (without its slash).
pub const COMMAND: &str = "treefall";
pub const USAGE: &str = "Usage: /treefall [status|on|off|toggle|reload]";
/// `LeavesBlock`: a leaves block more than this many steps (through leaves,
/// six faces) from any log decays.
pub const LEAF_REACH: u32 = 6;
/// Upper bound on leaves examined when `breakLeaves` is on; a canopy region
/// larger than this is left entirely to natural decay.
pub const LEAF_SCAN_LIMIT: usize = 16_384;
/// Hard upper bound for `maxLogs`.
pub const MAX_LOGS_LIMIT: usize = 4096;
/// How long (game ticks) a cached felling-speed answer for one target is reused.
pub const SPEED_CACHE_TICKS: i64 = 100;

/// How long cutting the first log of a tree takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BreakSpeed {
    /// As long as that one log.
    Single,
    /// That log's time times `1 + (logs - 1) * speedFactor`, at most `maxSlowdown`.
    Scaled,
}
/// How much the tool wears for a felled tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Durability {
    /// As if every log were mined by hand (the game's own wear, Unbreaking included).
    PerLog,
    /// One use for the whole tree.
    Once,
    /// `ceil(logs * durabilityFactor)` for the whole tree.
    Multiplier,
    /// No wear at all, not even for the cut log.
    #[serde(rename = "none")]
    Free,
}
/// What the main hand must hold for a break to fell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Tools {
    /// An item in the `minecraft:axes` item tag.
    Axe,
    /// Any item with a tool component (axes, pickaxes, shovels, hoes, swords, shears...).
    AnyTool,
    /// Anything, bare hands included.
    Any,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Sneak {
    /// Sneaking breaks a single log.
    Disables,
    /// Only sneaking fells.
    Enables,
    /// Sneaking makes no difference.
    Ignored,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Exhaustion {
    /// The game's hunger cost for every block broken.
    PerLog,
    /// Only the cut log's.
    Once,
}

/// Wood keys (`oak`, `minecraft:crimson`, `mushroom`...) that may or may not fell.
/// An empty `allow` allows every wood; `deny` wins.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Woods {
    pub allow: BTreeSet<String>,
    pub deny: BTreeSet<String>,
}
fn namespaced(key: &str) -> String {
    if key.contains(':') {
        key.to_owned()
    } else {
        format!("minecraft:{key}")
    }
}
impl Woods {
    pub fn permits(&self, wood: &str) -> bool {
        let listed = |set: &BTreeSet<String>| set.iter().any(|w| namespaced(w) == wood);
        (self.allow.is_empty() || listed(&self.allow)) && !listed(&self.deny)
    }
    fn validate(&self) -> Result<(), String> {
        for w in self.allow.iter().chain(&self.deny) {
            let valid = !w.is_empty()
                && w.split(':').count() <= 2
                && w.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-/:".contains(&b));
            if !valid {
                return Err(format!("woods: invalid wood key {w:?}"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Config {
    /// Global switch.
    pub enabled: bool,
    pub sneak: Sneak,
    pub tool: Tools,
    /// The largest tree (connected logs at or above the cut) that is felled.
    /// Bigger log structures are not felled at all.
    pub max_logs: usize,
    /// Natural (non-persistent) leaves that must touch the log group.
    pub min_leaves: usize,
    /// Also break the leaves that would decay once the tree is gone.
    pub break_leaves: bool,
    pub break_speed: BreakSpeed,
    pub speed_factor: f32,
    pub max_slowdown: f32,
    pub durability: Durability,
    pub durability_factor: f32,
    /// Felling stops before the tool would have fewer durability points left.
    pub keep_durability: u32,
    /// Drop everything at the cut block instead of where each block was.
    pub drops_at_stump: bool,
    /// Plant a sapling of the felled wood from the inventory at the cut block.
    pub replant: bool,
    pub exhaustion: Exhaustion,
    pub woods: Woods,
    /// Fell huge mushrooms too (mushroom stems with a mushroom-block cap).
    pub mushrooms: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            sneak: Sneak::Disables,
            tool: Tools::Axe,
            max_logs: 256,
            min_leaves: 5,
            break_leaves: false,
            break_speed: BreakSpeed::Single,
            speed_factor: 0.5,
            max_slowdown: 20.0,
            durability: Durability::PerLog,
            durability_factor: 0.5,
            keep_durability: 1,
            drops_at_stump: false,
            replant: false,
            exhaustion: Exhaustion::PerLog,
            woods: Woods::default(),
            mushrooms: false,
        }
    }
}
impl Config {
    pub fn from_json(source: &str) -> Result<Self, String> {
        let config: Self = serde_json::from_str(source).map_err(|e| e.to_string())?;
        config.validate()?;
        Ok(config)
    }
    /// The file the settings page writes: every key, readable by `from_json`.
    pub fn to_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self).unwrap_or_default();
        json.push('\n');
        json
    }
    pub fn validate(&self) -> Result<(), String> {
        let within = |v: f32, lo: f32, hi: f32| v.is_finite() && (lo..=hi).contains(&v);
        if !(1..=MAX_LOGS_LIMIT).contains(&self.max_logs) {
            return Err(format!("maxLogs must be 1..={MAX_LOGS_LIMIT}"));
        }
        if self.min_leaves > 1024 {
            return Err("minLeaves must be 0..=1024".into());
        }
        if !within(self.speed_factor, 0.0, 10.0) {
            return Err("speedFactor must be 0..=10".into());
        }
        if !within(self.max_slowdown, 1.0, 100.0) {
            return Err("maxSlowdown must be 1..=100".into());
        }
        if !within(self.durability_factor, 0.0, 10.0) {
            return Err("durabilityFactor must be 0..=10".into());
        }
        if self.keep_durability > 100_000 {
            return Err("keepDurability must be 0..=100000".into());
        }
        self.woods.validate()
    }
    /// Total tool wear for the first `logs` logs of a tree.
    pub fn wear_for(&self, logs: usize) -> u32 {
        let logs = logs as u32;
        match self.durability {
            Durability::PerLog => logs,
            Durability::Once => logs.min(1),
            Durability::Multiplier => {
                // Rounded up, with a guard against float noise (0.1 * 30 = 3.0000002).
                let exact = f64::from(logs) * f64::from(self.durability_factor);
                (exact - 1e-6).ceil().max(0.0) as u32
            }
            Durability::Free => 0,
        }
    }
    /// The wear the `nth` log (1 = the cut log) adds.
    pub fn wear_of(&self, nth: usize) -> u32 {
        self.wear_for(nth) - self.wear_for(nth.saturating_sub(1))
    }
}

/// What a block is to the tree finder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// In the `minecraft:logs` block tag (stripped variants included).
    Log,
    /// In the `minecraft:leaves` block tag; `natural` is `persistent=false`.
    Leaves {
        natural: bool,
    },
    /// A huge fungus' crown: `#minecraft:wart_blocks` or shroomlight.
    Crown,
    /// `minecraft:mushroom_stem`.
    MushroomStem,
    /// A huge mushroom's cap: red or brown mushroom block.
    Cap,
    Other,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub name: String,
    pub state: u32,
    pub kind: Kind,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Player {
    pub actor: Actor,
    /// Survival or adventure, playing locally or hosting (not a remote client).
    pub survival: bool,
    pub sneaking: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tool {
    pub item: String,
    pub axe: bool,
    /// Has a tool component.
    pub tool: bool,
    /// Durability points left; None for items without durability.
    pub remaining: Option<u32>,
}
/// How one block is broken.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MineFlags {
    /// Leave the held tool's damage as it was.
    pub keep_durability: bool,
    /// Leave the player's food exhaustion as it was.
    pub keep_exhaustion: bool,
}

/// Read-only block access. None means unloaded or outside the build height;
/// it must never load or generate a chunk.
pub trait Blocks {
    fn block(&self, position: Position) -> Option<Block>;
}
pub trait Host: Blocks {
    type Error;
    fn player(&mut self) -> Player;
    fn tool(&self) -> Option<Tool>;
    /// Break the block through the game's ordinary player-mining path (drops,
    /// tool wear, statistics, hunger). Returns whether the block was removed.
    fn mine(&mut self, position: Position, flags: MineFlags) -> Result<bool, Self::Error>;
    /// Wear the held tool by `amount` uses (the game's Unbreaking applies).
    fn wear(&mut self, amount: u32);
    /// Send the drops of the blocks broken from now on to `at` (None: where they were).
    fn gather(&mut self, at: Option<Position>);
    /// Place `item` (a sapling, fungus or propagule) from the inventory at
    /// `at` if the block there is free and the one below can hold it.
    fn replant(&mut self, at: Position, item: &str) -> bool;
}

/// The wood a log belongs to (`minecraft:oak_log` and `minecraft:oak_wood` are
/// both `minecraft:oak`; `minecraft:mushroom_stem` is `minecraft:mushroom`);
/// None for stripped logs, which are not felled.
pub fn wood_key(name: &str) -> Option<&str> {
    let path = name.rsplit_once(':').map_or(name, |(_, path)| path);
    if path.starts_with("stripped_") || path.contains("/stripped_") {
        return None;
    }
    for suffix in ["_log", "_wood", "_stem", "_hyphae"] {
        if let Some(base) = name.strip_suffix(suffix) {
            return Some(base);
        }
    }
    Some(name)
}
/// Nether "trees" (huge fungi) have stems and a wart-block crown, no leaves.
pub fn is_stem(name: &str) -> bool {
    name.ends_with("_stem") || name.ends_with("_hyphae")
}
/// The item that regrows `wood` (a wood key), if any.
pub fn sapling_for(wood: &str) -> Option<String> {
    let (namespace, path) = wood.split_once(':').unwrap_or(("minecraft", wood));
    Some(match path {
        "mushroom" => return None,
        "crimson" | "warped" => format!("{namespace}:{path}_fungus"),
        "mangrove" => format!("{namespace}:mangrove_propagule"),
        _ => format!("{namespace}:{path}_sapling"),
    })
}

fn offset(p: Position, d: [i32; 3]) -> Option<Position> {
    Some([
        p[0].checked_add(d[0])?,
        p[1].checked_add(d[1])?,
        p[2].checked_add(d[2])?,
    ])
}
/// The 26 blocks around `p`.
fn around(p: Position) -> impl Iterator<Item = Position> {
    (-1..=1)
        .flat_map(|x| (-1..=1).flat_map(move |y| (-1..=1).map(move |z| [x, y, z])))
        .filter(|d| *d != [0, 0, 0])
        .filter_map(move |d| offset(p, d))
}
/// The 6 blocks sharing a face with `p`.
fn faces(p: Position) -> impl Iterator<Item = Position> {
    [
        [1, 0, 0],
        [-1, 0, 0],
        [0, 1, 0],
        [0, -1, 0],
        [0, 0, 1],
        [0, 0, -1],
    ]
    .into_iter()
    .filter_map(move |d| offset(p, d))
}

/// Why a break was left to the game's ordinary behaviour.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Skip {
    Disabled,
    PlayerOff,
    NotSurvival,
    Sneak,
    NoTool,
    NotLog,
    Wood,
    TooBig,
    TooFewLeaves,
    Lone,
}

/// The connected trunk blocks (`kind`) of the same wood as the one at
/// `origin`, origin first, breadth-first over the 26-neighbourhood, never
/// below `origin`'s y. More than `max_logs` is `Skip::TooBig`.
pub fn find_logs<W: Blocks + ?Sized>(
    world: &W,
    origin: Position,
    max_logs: usize,
) -> Result<Vec<(Position, Block)>, Skip> {
    let seed = world.block(origin).ok_or(Skip::NotLog)?;
    if !matches!(seed.kind, Kind::Log | Kind::MushroomStem) {
        return Err(Skip::NotLog);
    }
    let wood = wood_key(&seed.name).ok_or(Skip::NotLog)?.to_owned();
    let mut logs = Vec::new();
    let mut seen = BTreeSet::from([origin]);
    let mut queue = VecDeque::from([origin]);
    while let Some(p) = queue.pop_front() {
        let Some(block) = world.block(p) else {
            continue;
        };
        if block.kind != seed.kind || wood_key(&block.name) != Some(wood.as_str()) {
            continue;
        }
        if logs.len() >= max_logs {
            return Err(Skip::TooBig);
        }
        logs.push((p, block));
        for n in around(p) {
            if n[1] >= origin[1] && seen.insert(n) {
                queue.push_back(n);
            }
        }
    }
    Ok(logs)
}

/// Natural foliage touching (26-neighbourhood) any of `logs`: leaves with
/// `persistent=false`, or `crown` blocks for trunks that have no leaves.
pub fn count_foliage<W: Blocks + ?Sized>(
    world: &W,
    logs: &BTreeSet<Position>,
    crown: Option<Kind>,
) -> usize {
    let mut found = BTreeSet::new();
    for &log in logs {
        for n in around(log) {
            if logs.contains(&n) || found.contains(&n) {
                continue;
            }
            if let Some(b) = world.block(n)
                && (b.kind == Kind::Leaves { natural: true } || Some(b.kind) == crown)
            {
                found.insert(n);
            }
        }
    }
    found.len()
}

/// The natural leaves that would decay once `logs` are gone: within
/// `LEAF_REACH` of the felled logs and not within `LEAF_REACH` of any other log
/// (vanilla's leaf distance: six faces, through any leaves). Unloaded blocks
/// next to the canopy count as logs, so nothing that might be supported is
/// touched. An oversized canopy returns nothing.
pub fn decaying_leaves<W: Blocks + ?Sized>(
    world: &W,
    logs: &BTreeSet<Position>,
) -> Vec<(Position, Block)> {
    let mut depth: BTreeMap<Position, u32> = BTreeMap::new();
    let mut blocks: BTreeMap<Position, Block> = BTreeMap::new();
    let mut sources: BTreeSet<Position> = BTreeSet::new();
    let mut queue = VecDeque::new();
    for &log in logs {
        for n in faces(log) {
            if logs.contains(&n) || depth.contains_key(&n) {
                continue;
            }
            if let Some(b) = world.block(n)
                && matches!(b.kind, Kind::Leaves { .. })
            {
                depth.insert(n, 1);
                blocks.insert(n, b);
                queue.push_back(n);
            }
        }
    }
    // The region: leaves up to twice the reach away, so that every path from a
    // candidate to another supporting log lies inside it.
    while let Some(p) = queue.pop_front() {
        if depth.len() > LEAF_SCAN_LIMIT {
            return Vec::new();
        }
        let d = depth[&p];
        for n in faces(p) {
            if logs.contains(&n) || depth.contains_key(&n) {
                continue;
            }
            match world.block(n) {
                None => {
                    sources.insert(n);
                }
                Some(b) if b.kind == Kind::Log => {
                    sources.insert(n);
                }
                Some(b) if matches!(b.kind, Kind::Leaves { .. }) && d < 2 * LEAF_REACH => {
                    depth.insert(n, d + 1);
                    blocks.insert(n, b);
                    queue.push_back(n);
                }
                Some(_) => {}
            }
        }
    }
    let mut support: BTreeMap<Position, u32> = BTreeMap::new();
    let mut queue = VecDeque::new();
    for &s in &sources {
        for n in faces(s) {
            if depth.contains_key(&n) && !support.contains_key(&n) {
                support.insert(n, 1);
                queue.push_back(n);
            }
        }
    }
    while let Some(p) = queue.pop_front() {
        let d = support[&p];
        if d >= LEAF_REACH {
            continue;
        }
        for n in faces(p) {
            if depth.contains_key(&n) && !support.contains_key(&n) {
                support.insert(n, d + 1);
                queue.push_back(n);
            }
        }
    }
    let mut out: Vec<(Position, Block)> = depth
        .iter()
        .filter(|(p, d)| **d <= LEAF_REACH && !support.contains_key(*p))
        .filter_map(|(p, _)| blocks.remove(p).map(|b| (*p, b)))
        .filter(|(_, b)| b.kind == Kind::Leaves { natural: true })
        .collect();
    // Top first, like the logs.
    out.sort_by_key(|(p, _)| std::cmp::Reverse(p[1]));
    out
}

/// A tree that will be felled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// The block the player broke; broken last, as the game would, once the
    /// logs above it are down.
    pub origin: (Position, Block),
    /// The other logs, highest first (so a stop leaves a standing stump).
    pub logs: Vec<(Position, Block)>,
    /// Leaves to break afterwards (only with `breakLeaves`).
    pub leaves: Vec<(Position, Block)>,
    /// The held tool when planning; felling stops if it changes.
    pub tool: Option<Tool>,
    /// What regrows this tree.
    pub sapling: Option<String>,
}
impl Plan {
    /// How many times longer the cut log takes to break (`breakSpeed`).
    pub fn slowdown(&self, config: &Config) -> f32 {
        match config.break_speed {
            BreakSpeed::Single => 1.0,
            BreakSpeed::Scaled => (1.0 + self.logs.len() as f32 * config.speed_factor)
                .min(config.max_slowdown)
                .max(1.0),
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub logs: usize,
    pub leaves: usize,
    pub changed: usize,
    pub stopped: bool,
    pub replanted: bool,
}

/// Decide, without changing anything, whether breaking `origin` fells a tree.
/// `leaves` computes the leaves to break (skip it when only the size matters).
pub fn plan_tree<W: Blocks + ?Sized>(
    world: &W,
    config: &Config,
    player: &Player,
    player_enabled: bool,
    tool: Option<Tool>,
    origin: Position,
    leaves: bool,
) -> Result<Plan, Skip> {
    if !config.enabled || config.validate().is_err() {
        return Err(Skip::Disabled);
    }
    if !player_enabled {
        return Err(Skip::PlayerOff);
    }
    if !player.survival {
        return Err(Skip::NotSurvival);
    }
    let sneak_ok = match config.sneak {
        Sneak::Disables => !player.sneaking,
        Sneak::Enables => player.sneaking,
        Sneak::Ignored => true,
    };
    if !sneak_ok {
        return Err(Skip::Sneak);
    }
    let tool_ok = match config.tool {
        Tools::Axe => tool.as_ref().is_some_and(|t| t.axe),
        Tools::AnyTool => tool.as_ref().is_some_and(|t| t.axe || t.tool),
        Tools::Any => true,
    };
    if !tool_ok {
        return Err(Skip::NoTool);
    }
    let seed = world.block(origin).ok_or(Skip::NotLog)?;
    if seed.kind == Kind::MushroomStem && !config.mushrooms {
        return Err(Skip::NotLog);
    }
    let wood = wood_key(&seed.name).ok_or(Skip::NotLog)?.to_owned();
    if !config.woods.permits(&wood) {
        return Err(Skip::Wood);
    }
    let mut logs = find_logs(world, origin, config.max_logs)?;
    let set: BTreeSet<Position> = logs.iter().map(|(p, _)| *p).collect();
    let crown = match seed.kind {
        Kind::MushroomStem => Some(Kind::Cap),
        _ if is_stem(&seed.name) => Some(Kind::Crown),
        _ => None,
    };
    if count_foliage(world, &set, crown) < config.min_leaves {
        return Err(Skip::TooFewLeaves);
    }
    let leaves = if leaves && config.break_leaves {
        decaying_leaves(world, &set)
    } else {
        Vec::new()
    };
    if logs.len() == 1 && leaves.is_empty() {
        return Err(Skip::Lone);
    }
    let origin = logs.remove(0);
    // Stable: breadth-first order within a layer.
    logs.sort_by_key(|(p, _)| std::cmp::Reverse(p[1]));
    Ok(Plan {
        origin,
        logs,
        leaves,
        tool,
        sapling: sapling_for(&wood),
    })
}

/// A parsed `/treefall` command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Status,
    On,
    Off,
    Toggle,
    Reload,
}
/// Whether `line` (with or without its slash) is a `/treefall` command.
pub fn is_command(line: &str) -> bool {
    line.trim()
        .trim_start_matches('/')
        .split_whitespace()
        .next()
        == Some(COMMAND)
}
/// None when the line is another command; Err(usage) for bad arguments.
pub fn parse_command(line: &str) -> Option<Result<Command, &'static str>> {
    if !is_command(line) {
        return None;
    }
    let args: Vec<&str> = line
        .trim()
        .trim_start_matches('/')
        .split_whitespace()
        .skip(1)
        .collect();
    Some(match args.as_slice() {
        [] | ["status"] => Ok(Command::Status),
        ["on"] => Ok(Command::On),
        ["off"] => Ok(Command::Off),
        ["toggle"] => Ok(Command::Toggle),
        ["reload"] => Ok(Command::Reload),
        _ => Err(USAGE),
    })
}

/// What a cached felling-speed answer was computed for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpeedKey {
    pub actor: Actor,
    pub position: Position,
    pub state: u32,
    pub item: Option<String>,
}

/// Runtime state: the players who turned TreeFall off (until the game
/// closes), and the last felling-speed answer (mining asks every tick).
#[derive(Default)]
pub struct Runtime {
    disabled: BTreeSet<Actor>,
    speed: Option<(SpeedKey, i64, f32)>,
}
impl Runtime {
    /// No one turned off and nothing cached (`Default`, usable in a `static`).
    pub const fn new() -> Self {
        Self {
            disabled: BTreeSet::new(),
            speed: None,
        }
    }
    pub fn enabled(&self, actor: Actor) -> bool {
        !self.disabled.contains(&actor)
    }
    pub fn set_enabled(&mut self, actor: Actor, enabled: bool) {
        self.forget_speed();
        if enabled {
            self.disabled.remove(&actor);
        } else {
            self.disabled.insert(actor);
        }
    }
    /// Apply a status/on/off/toggle command for `actor`; the chat reply.
    /// `Reload` needs the game and is handled by the caller.
    pub fn command(&mut self, actor: Actor, command: Command, config: &Config) -> String {
        let on = match command {
            Command::On => true,
            Command::Off => false,
            Command::Toggle => !self.enabled(actor),
            Command::Status | Command::Reload => self.enabled(actor),
        };
        self.set_enabled(actor, on);
        let mut reply = format!("TreeFall: {}", if on { "on" } else { "off" });
        if !config.enabled {
            reply.push_str(" (disabled in the server's configuration)");
        }
        reply
    }
    /// The slowdown for `key` at game tick `now`: reused while the target,
    /// its block, the player and the held item stay the same, for at most
    /// `SPEED_CACHE_TICKS`; otherwise `compute` runs.
    pub fn slowdown(&mut self, key: SpeedKey, now: i64, compute: impl FnOnce() -> f32) -> f32 {
        if let Some((cached, at, value)) = &self.speed
            && *cached == key
            && (0..SPEED_CACHE_TICKS).contains(&(now - at))
        {
            return *value;
        }
        let value = compute();
        self.speed = Some((key, now, value));
        value
    }
    pub fn forget_speed(&mut self) {
        self.speed = None;
    }

    /// Decide, without changing anything, whether breaking `origin` fells a tree.
    pub fn plan<H: Host>(
        &self,
        host: &mut H,
        config: &Config,
        origin: Position,
    ) -> Result<Plan, Skip> {
        let player = host.player();
        let tool = host.tool();
        let enabled = self.enabled(player.actor);
        plan_tree(&*host, config, &player, enabled, tool, origin, true)
    }
}

/// Fell a planned tree from the top down: every log that is still there, then
/// the broken block as the game would, then, if every log came down, the
/// planned leaves and the sapling. Felling stops before the tool would go
/// below `keepDurability` (the cut log's own wear kept in reserve) or if it
/// is swapped; what is left, the cut log with it, stands as a stump. When not
/// even one log above the cut can come down, the cut log breaks alone, as in
/// vanilla.
pub fn fell_tree<H: Host>(host: &mut H, config: &Config, plan: &Plan) -> Result<Report, H::Error> {
    let mut report = Report::default();
    if config.drops_at_stump {
        host.gather(Some(plan.origin.0));
    }
    let felled = fell_logs(host, config, plan, &mut report);
    if config.drops_at_stump {
        host.gather(None);
    }
    felled?;
    if config.replant
        && !report.stopped
        && report.changed == 0
        && let Some(item) = &plan.sapling
    {
        report.replanted = host.replant(plan.origin.0, item);
    }
    Ok(report)
}

fn fell_logs<H: Host>(
    host: &mut H,
    config: &Config,
    plan: &Plan,
    report: &mut Report,
) -> Result<(), H::Error> {
    let own_wear = config.durability != Durability::PerLog;
    let flags = MineFlags {
        keep_durability: own_wear,
        keep_exhaustion: config.exhaustion == Exhaustion::Once,
    };
    let item = plan.tool.as_ref().map(|t| t.item.as_str());
    // The cut log counts as the first log for wear, and breaks last.
    let reserve = config.wear_of(1);
    for (p, block) in &plan.logs {
        let wear = config.wear_of(report.logs + 2);
        let tool = host.tool();
        let needed = u64::from(wear) + u64::from(reserve) + u64::from(config.keep_durability);
        if tool.as_ref().map(|t| t.item.as_str()) != item
            || tool
                .and_then(|t| t.remaining)
                .is_some_and(|n| u64::from(n) < needed)
        {
            report.stopped = true;
            break;
        }
        if host.block(*p).as_ref() != Some(block) {
            report.changed += 1;
            continue;
        }
        if !host.mine(*p, flags)? {
            report.stopped = true;
            break;
        }
        report.logs += 1;
        if own_wear {
            host.wear(wear);
        }
    }
    // Stopped part way: the logs left stand on the cut log, a stump.
    if report.stopped && report.logs > 0 {
        return Ok(());
    }
    let origin = MineFlags {
        keep_durability: own_wear,
        keep_exhaustion: false,
    };
    if !host.mine(plan.origin.0, origin)? {
        report.stopped = true;
        return Ok(());
    }
    report.logs += 1;
    if own_wear {
        host.wear(reserve);
    }
    if report.stopped || report.changed > 0 {
        return Ok(());
    }
    let leaves = MineFlags {
        keep_durability: true,
        ..flags
    };
    for (p, block) in &plan.leaves {
        if host.block(*p).as_ref() == Some(block) && host.mine(*p, leaves)? {
            report.leaves += 1;
        }
    }
    Ok(())
}

/// The configuration as read from [`CONFIG_FILE`]: Err when the file is broken
/// (TreeFall is then off until it is fixed or reset).
pub type Loaded = Result<Config, String>;

/// A button of the settings page (Options > Mods > TreeFall > Settings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    /// The file is broken: its error. A click reads it again.
    Broken,
    Enabled,
    Tool,
    Sneak,
    MaxLogs,
    MinLeaves,
    BreakLeaves,
    /// `breakSpeed`.
    CutTime,
    SpeedFactor,
    MaxSlowdown,
    /// `durability`.
    Wear,
    /// `durabilityFactor`.
    WearFactor,
    KeepDurability,
    /// `exhaustion`.
    Hunger,
    DropsAtStump,
    Replant,
    Mushrooms,
    /// The `woods` lists, edited in the file. A click reads it again.
    Woods,
    /// Every setting back to its default (the woods lists kept).
    Defaults,
}
/// What a click on the settings page asks of the game.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Write this configuration to the file; it applies at once.
    Save(Config),
    /// Read the file again (after editing it by hand).
    Reload,
}

/// The steps the numeric settings cycle through (each holds the default).
pub const MAX_LOGS_STEPS: [usize; 9] = [16, 32, 64, 128, 256, 512, 1024, 2048, 4096];
pub const MIN_LEAVES_STEPS: [usize; 8] = [0, 1, 3, 5, 10, 20, 50, 100];
pub const SPEED_FACTOR_STEPS: [f32; 9] = [0.0, 0.1, 0.25, 0.5, 0.75, 1.0, 2.0, 5.0, 10.0];
pub const MAX_SLOWDOWN_STEPS: [f32; 8] = [1.0, 2.0, 3.0, 5.0, 10.0, 20.0, 50.0, 100.0];
pub const DURABILITY_FACTOR_STEPS: [f32; 9] = [0.0, 0.1, 0.25, 0.5, 0.75, 1.0, 2.0, 5.0, 10.0];
pub const KEEP_DURABILITY_STEPS: [u32; 8] = [0, 1, 2, 5, 10, 25, 50, 100];
/// Longest error shown on the page (the whole one goes to stderr).
const ERROR_WIDTH: usize = 40;

/// The next step above `now`, back to the first after the last (a value
/// between two steps goes to the one above it).
fn step<T: Copy + PartialOrd>(steps: &[T], now: T) -> T {
    steps.iter().copied().find(|&s| s > now).unwrap_or(steps[0])
}
fn on_off(on: bool) -> &'static str {
    if on { "ON" } else { "OFF" }
}

/// The page's buttons, in order. A factor shows only in the mode it is for.
pub fn page(loaded: &Loaded) -> Vec<Setting> {
    let Ok(c) = loaded else {
        return vec![Setting::Broken, Setting::Defaults];
    };
    let mut page = vec![
        Setting::Enabled,
        Setting::Tool,
        Setting::Sneak,
        Setting::MaxLogs,
        Setting::MinLeaves,
        Setting::BreakLeaves,
        Setting::CutTime,
    ];
    if c.break_speed == BreakSpeed::Scaled {
        page.extend([Setting::SpeedFactor, Setting::MaxSlowdown]);
    }
    page.push(Setting::Wear);
    if c.durability == Durability::Multiplier {
        page.push(Setting::WearFactor);
    }
    page.extend([
        Setting::KeepDurability,
        Setting::Hunger,
        Setting::DropsAtStump,
        Setting::Replant,
        Setting::Mushrooms,
        Setting::Woods,
        Setting::Defaults,
    ]);
    page
}

impl Setting {
    /// Its button's label.
    pub fn label(self, loaded: &Loaded) -> String {
        let c = match loaded {
            Ok(c) => c,
            Err(error) => {
                return match self {
                    Self::Defaults => "Reset to defaults".into(),
                    _ => {
                        let mut short: String = error.chars().take(ERROR_WIDTH).collect();
                        if short.len() < error.len() {
                            short = format!("{}...", short.trim_end());
                        }
                        format!("{CONFIG_FILE}: {short} (click to reload)")
                    }
                };
            }
        };
        match self {
            Self::Broken => String::new(),
            Self::Enabled => format!("Felling: {}", on_off(c.enabled)),
            Self::Tool => match c.tool {
                Tools::Axe => "Tool: axe",
                Tools::AnyTool => "Tool: any tool",
                Tools::Any => "Tool: anything, bare hands too",
            }
            .into(),
            Self::Sneak => match c.sneak {
                Sneak::Disables => "Sneaking: cuts a single log",
                Sneak::Enables => "Sneaking: needed to fell",
                Sneak::Ignored => "Sneaking: makes no difference",
            }
            .into(),
            Self::MaxLogs => format!("Largest tree: {} logs", c.max_logs),
            Self::MinLeaves if c.min_leaves == 0 => "Leaves needed: 0 (any log structure)".into(),
            Self::MinLeaves => format!("Leaves needed: {}", c.min_leaves),
            Self::BreakLeaves => format!("Break the leaves: {}", on_off(c.break_leaves)),
            Self::CutTime => match c.break_speed {
                BreakSpeed::Single => "Cut time: as one log",
                BreakSpeed::Scaled => "Cut time: grows with the tree",
            }
            .into(),
            Self::SpeedFactor => format!("Extra time per log: {}x", c.speed_factor),
            Self::MaxSlowdown => format!("Longest cut: {}x one log", c.max_slowdown),
            Self::Wear => match c.durability {
                Durability::PerLog => "Tool wear: every log",
                Durability::Once => "Tool wear: once per tree",
                Durability::Multiplier => "Tool wear: logs x factor",
                Durability::Free => "Tool wear: none",
            }
            .into(),
            Self::WearFactor => format!("Wear factor: {} per log", c.durability_factor),
            Self::KeepDurability if c.keep_durability == 0 => {
                "Keep durability: 0 (tool may break)".into()
            }
            Self::KeepDurability => format!("Keep durability: {}", c.keep_durability),
            Self::Hunger => match c.exhaustion {
                Exhaustion::PerLog => "Hunger: every log",
                Exhaustion::Once => "Hunger: cut log only",
            }
            .into(),
            Self::DropsAtStump => format!("Drops at the stump: {}", on_off(c.drops_at_stump)),
            Self::Replant => format!("Replant a sapling: {}", on_off(c.replant)),
            Self::Mushrooms => format!("Huge mushrooms: {}", on_off(c.mushrooms)),
            Self::Woods => {
                let (allow, deny) = (c.woods.allow.len(), c.woods.deny.len());
                let woods = match (allow, deny) {
                    (0, 0) => "all".to_owned(),
                    (0, d) => format!("all but {d}"),
                    (a, 0) => format!("{a} allowed"),
                    (a, d) => format!("{a} allowed, {d} denied"),
                };
                format!("Woods: {woods} ({CONFIG_FILE}, click to reload)")
            }
            Self::Defaults => "Reset to defaults (woods kept)".into(),
        }
    }

    /// What a click on it does to `loaded`.
    pub fn click(self, loaded: &Loaded) -> Effect {
        let Ok(c) = loaded else {
            return match self {
                Self::Defaults => Effect::Save(Config::default()),
                _ => Effect::Reload,
            };
        };
        let mut c = c.clone();
        match self {
            Self::Broken | Self::Woods => return Effect::Reload,
            Self::Enabled => c.enabled = !c.enabled,
            Self::Tool => {
                c.tool = match c.tool {
                    Tools::Axe => Tools::AnyTool,
                    Tools::AnyTool => Tools::Any,
                    Tools::Any => Tools::Axe,
                }
            }
            Self::Sneak => {
                c.sneak = match c.sneak {
                    Sneak::Disables => Sneak::Enables,
                    Sneak::Enables => Sneak::Ignored,
                    Sneak::Ignored => Sneak::Disables,
                }
            }
            Self::MaxLogs => c.max_logs = step(&MAX_LOGS_STEPS, c.max_logs),
            Self::MinLeaves => c.min_leaves = step(&MIN_LEAVES_STEPS, c.min_leaves),
            Self::BreakLeaves => c.break_leaves = !c.break_leaves,
            Self::CutTime => {
                c.break_speed = match c.break_speed {
                    BreakSpeed::Single => BreakSpeed::Scaled,
                    BreakSpeed::Scaled => BreakSpeed::Single,
                }
            }
            Self::SpeedFactor => c.speed_factor = step(&SPEED_FACTOR_STEPS, c.speed_factor),
            Self::MaxSlowdown => c.max_slowdown = step(&MAX_SLOWDOWN_STEPS, c.max_slowdown),
            Self::Wear => {
                c.durability = match c.durability {
                    Durability::PerLog => Durability::Once,
                    Durability::Once => Durability::Multiplier,
                    Durability::Multiplier => Durability::Free,
                    Durability::Free => Durability::PerLog,
                }
            }
            Self::WearFactor => {
                c.durability_factor = step(&DURABILITY_FACTOR_STEPS, c.durability_factor);
            }
            Self::KeepDurability => {
                c.keep_durability = step(&KEEP_DURABILITY_STEPS, c.keep_durability);
            }
            Self::Hunger => {
                c.exhaustion = match c.exhaustion {
                    Exhaustion::PerLog => Exhaustion::Once,
                    Exhaustion::Once => Exhaustion::PerLog,
                }
            }
            Self::DropsAtStump => c.drops_at_stump = !c.drops_at_stump,
            Self::Replant => c.replant = !c.replant,
            Self::Mushrooms => c.mushrooms = !c.mushrooms,
            Self::Defaults => {
                c = Config {
                    woods: c.woods,
                    ..Config::default()
                }
            }
        }
        Effect::Save(c)
    }
}

/// The settings page's labels (`native_settings`).
pub fn labels(loaded: &Loaded) -> Vec<String> {
    page(loaded).into_iter().map(|s| s.label(loaded)).collect()
}
/// A click on button `button` of the page `labels` gave (`native_settings_click`).
pub fn click(loaded: &Loaded, button: usize) -> Option<Effect> {
    page(loaded).get(button).map(|s| s.click(loaded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct World {
        blocks: BTreeMap<Position, Block>,
        unloaded: BTreeSet<Position>,
        player: Player,
        tool: Option<Tool>,
        calls: Vec<(Position, MineFlags)>,
        fail: BTreeSet<Position>,
        /// Replace this block after the first mine call (the top log).
        change_after_first: Option<Position>,
        gather: Option<Position>,
        /// Where each broken block's drop landed.
        drops: Vec<Position>,
        exhaustion: f32,
        inventory: BTreeMap<String, u32>,
    }
    fn block(name: &str) -> Block {
        let kind = if name.ends_with("_leaves") {
            Kind::Leaves { natural: true }
        } else if name == "minecraft:mushroom_stem" {
            Kind::MushroomStem
        } else if name.ends_with("_mushroom_block") {
            Kind::Cap
        } else if name.ends_with("_log")
            || name.ends_with("_wood")
            || name.ends_with("_stem")
            || name.ends_with("_hyphae")
        {
            Kind::Log
        } else if name.ends_with("wart_block") || name == "minecraft:shroomlight" {
            Kind::Crown
        } else {
            Kind::Other
        };
        Block {
            name: name.into(),
            state: 1,
            kind,
        }
    }
    fn persistent(name: &str) -> Block {
        Block {
            kind: Kind::Leaves { natural: false },
            ..block(name)
        }
    }
    fn axe(remaining: Option<u32>) -> Option<Tool> {
        Some(Tool {
            item: "minecraft:iron_axe".into(),
            axe: true,
            tool: true,
            remaining,
        })
    }
    impl World {
        fn new() -> Self {
            Self {
                blocks: BTreeMap::new(),
                unloaded: BTreeSet::new(),
                player: Player {
                    actor: [0, 0, 0, 1],
                    survival: true,
                    sneaking: false,
                },
                tool: axe(Some(250)),
                calls: Vec::new(),
                fail: BTreeSet::new(),
                change_after_first: None,
                gather: None,
                drops: Vec::new(),
                exhaustion: 0.0,
                inventory: BTreeMap::new(),
            }
        }
        fn set(&mut self, p: Position, b: Block) -> &mut Self {
            self.blocks.insert(p, b);
            self
        }
        /// A small oak: `height` logs from `base` up, a 5x5 canopy around the
        /// top two logs and a 3x3 cap of leaves above.
        fn oak(&mut self, base: Position, height: i32) -> &mut Self {
            self.tree(base, height, "minecraft:oak_log", "minecraft:oak_leaves")
        }
        fn tree(&mut self, base: Position, height: i32, log: &str, leaves: &str) -> &mut Self {
            let top = base[1] + height - 1;
            for y in top - 1..=top + 1 {
                let r = if y > top { 1 } else { 2 };
                for x in -r..=r {
                    for z in -r..=r {
                        self.blocks
                            .entry([base[0] + x, y, base[2] + z])
                            .or_insert_with(|| block(leaves));
                    }
                }
            }
            for y in 0..height {
                self.set([base[0], base[1] + y, base[2]], block(log));
            }
            self
        }
        fn mined(&self) -> Vec<Position> {
            self.calls.iter().map(|(p, _)| *p).collect()
        }
        fn remaining(&self) -> Option<u32> {
            self.tool.as_ref().and_then(|t| t.remaining)
        }
        fn damage(&mut self, n: u32) {
            if let Some(t) = self.tool.as_mut()
                && let Some(r) = t.remaining.as_mut()
            {
                *r = r.saturating_sub(n);
                if *r == 0 {
                    self.tool = None;
                }
            }
        }
    }
    impl Blocks for World {
        fn block(&self, p: Position) -> Option<Block> {
            if self.unloaded.contains(&p) {
                return None;
            }
            Some(
                self.blocks
                    .get(&p)
                    .cloned()
                    .unwrap_or_else(|| block("minecraft:air")),
            )
        }
    }
    impl Host for World {
        type Error = &'static str;
        fn player(&mut self) -> Player {
            self.player.clone()
        }
        fn tool(&self) -> Option<Tool> {
            self.tool.clone()
        }
        fn mine(&mut self, p: Position, flags: MineFlags) -> Result<bool, Self::Error> {
            self.calls.push((p, flags));
            if self.fail.contains(&p) {
                return Ok(false);
            }
            let removed = self.blocks.remove(&p).is_some();
            if removed {
                self.drops.push(self.gather.unwrap_or(p));
                if !flags.keep_exhaustion {
                    self.exhaustion += 0.005;
                }
                if !flags.keep_durability {
                    self.damage(1);
                }
            }
            if self.calls.len() == 1
                && let Some(c) = self.change_after_first
            {
                self.blocks.insert(c, block("minecraft:oak_planks"));
            }
            Ok(removed)
        }
        fn wear(&mut self, amount: u32) {
            self.damage(amount);
        }
        fn gather(&mut self, at: Option<Position>) {
            self.gather = at;
        }
        fn replant(&mut self, at: Position, item: &str) -> bool {
            let below = [at[0], at[1] - 1, at[2]];
            let soil = self
                .blocks
                .get(&below)
                .is_some_and(|b| b.name == "minecraft:dirt");
            let count = self.inventory.get(item).copied().unwrap_or(0);
            if self.blocks.contains_key(&at) || !soil || count == 0 {
                return false;
            }
            self.inventory.insert(item.to_owned(), count - 1);
            self.set(at, block(item));
            true
        }
    }
    fn fell(w: &mut World, c: &Config, origin: Position) -> Result<Report, Skip> {
        let plan = Runtime::default().plan(w, c, origin)?;
        Ok(fell_tree(w, c, &plan).unwrap())
    }
    fn oak5() -> World {
        let mut w = World::new();
        w.oak([0, 64, 0], 5)
            .set([0, 63, 0], block("minecraft:dirt"));
        w
    }
    fn with(f: impl FnOnce(&mut Config)) -> Config {
        let mut c = Config::default();
        f(&mut c);
        c
    }

    #[test]
    fn wood_keys_group_logs_and_wood_and_exclude_stripped() {
        assert_eq!(wood_key("minecraft:oak_log"), Some("minecraft:oak"));
        assert_eq!(wood_key("minecraft:oak_wood"), Some("minecraft:oak"));
        assert_eq!(
            wood_key("minecraft:dark_oak_log"),
            Some("minecraft:dark_oak")
        );
        assert_eq!(
            wood_key("minecraft:crimson_stem"),
            Some("minecraft:crimson")
        );
        assert_eq!(
            wood_key("minecraft:crimson_hyphae"),
            Some("minecraft:crimson")
        );
        assert_eq!(
            wood_key("minecraft:mushroom_stem"),
            Some("minecraft:mushroom")
        );
        assert_eq!(wood_key("minecraft:stripped_oak_log"), None);
        assert_eq!(wood_key("minecraft:stripped_warped_hyphae"), None);
        assert_eq!(wood_key("somemod:trees/stripped_maple_log"), None);
        assert_eq!(wood_key("somemod:palm_trunk"), Some("somemod:palm_trunk"));
        assert!(is_stem("minecraft:warped_stem") && !is_stem("minecraft:oak_log"));
    }

    #[test]
    fn saplings_match_their_wood() {
        assert_eq!(
            sapling_for("minecraft:oak").unwrap(),
            "minecraft:oak_sapling"
        );
        assert_eq!(
            sapling_for("minecraft:pale_oak").unwrap(),
            "minecraft:pale_oak_sapling"
        );
        assert_eq!(
            sapling_for("minecraft:mangrove").unwrap(),
            "minecraft:mangrove_propagule"
        );
        assert_eq!(
            sapling_for("minecraft:warped").unwrap(),
            "minecraft:warped_fungus"
        );
        assert_eq!(sapling_for("minecraft:mushroom"), None);
    }

    #[test]
    fn natural_oak_is_felled_top_down_and_the_cut_log_last() {
        let mut w = oak5();
        let r = fell(&mut w, &Config::default(), [0, 64, 0]).unwrap();
        assert_eq!(
            r,
            Report {
                logs: 5,
                ..Report::default()
            }
        );
        assert_eq!(
            w.mined(),
            vec![[0, 68, 0], [0, 67, 0], [0, 66, 0], [0, 65, 0], [0, 64, 0]]
        );
        assert!(w.calls.iter().all(|(_, f)| *f == MineFlags::default()));
        assert_eq!(w.remaining(), Some(245));
        assert_eq!(w.drops[0], [0, 68, 0]);
        // Leaves stay for natural decay by default.
        assert!(w.blocks.values().any(|b| b.name == "minecraft:oak_leaves"));
    }

    #[test]
    fn a_leafless_player_built_pillar_is_not_felled() {
        let mut w = World::new();
        for y in 64..69 {
            w.set([0, y, 0], block("minecraft:oak_log"));
        }
        assert_eq!(
            fell(&mut w, &Config::default(), [0, 64, 0]),
            Err(Skip::TooFewLeaves)
        );
        assert!(w.calls.is_empty());
        assert_eq!(w.blocks.len(), 5);
    }

    #[test]
    fn logs_below_the_cut_stay() {
        let mut w = World::new();
        w.oak([0, 64, 0], 6);
        fell(&mut w, &Config::default(), [0, 66, 0]).unwrap();
        assert_eq!(w.blocks[&[0, 64, 0]].name, "minecraft:oak_log");
        assert_eq!(w.blocks[&[0, 65, 0]].name, "minecraft:oak_log");
        assert!(!w.blocks.contains_key(&[0, 69, 0]));
    }

    #[test]
    fn diagonal_and_two_by_two_trunks_are_connected() {
        // An acacia-like diagonal trunk with a canopy at the top.
        let mut w = World::new();
        let trunk = [[0, 64, 0], [0, 65, 0], [1, 66, 1], [2, 67, 1], [2, 68, 2]];
        for p in trunk {
            w.set(p, block("minecraft:acacia_log"));
        }
        for x in 0..=4 {
            for z in 0..=4 {
                w.blocks
                    .entry([x, 69, z])
                    .or_insert_with(|| block("minecraft:acacia_leaves"));
            }
        }
        assert_eq!(
            fell(&mut w, &Config::default(), [0, 64, 0]).unwrap().logs,
            5
        );
        // A 2x2 trunk.
        let mut w = World::new();
        for (x, z) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            w.tree(
                [x, 64, z],
                8,
                "minecraft:dark_oak_log",
                "minecraft:dark_oak_leaves",
            );
        }
        assert_eq!(
            fell(&mut w, &Config::default(), [0, 64, 0]).unwrap().logs,
            32
        );
    }

    #[test]
    fn log_cabins_and_persistent_leaves_are_not_trees() {
        // A log wall with no leaves.
        let mut w = World::new();
        for x in 0..6 {
            for y in 64..68 {
                w.set([x, y, 0], block("minecraft:oak_log"));
            }
        }
        assert_eq!(
            fell(&mut w, &Config::default(), [0, 64, 0]),
            Err(Skip::TooFewLeaves)
        );
        // Decorated with player-placed (persistent) leaves.
        for x in 0..6 {
            w.set([x, 68, 0], persistent("minecraft:oak_leaves"));
            w.set([x, 66, 1], persistent("minecraft:oak_leaves"));
        }
        assert_eq!(
            fell(&mut w, &Config::default(), [0, 64, 0]),
            Err(Skip::TooFewLeaves)
        );
        // A few natural leaves blown against it are not enough.
        for x in 0..4 {
            w.set([x, 64, 1], block("minecraft:oak_leaves"));
        }
        assert_eq!(
            fell(&mut w, &Config::default(), [0, 64, 0]),
            Err(Skip::TooFewLeaves)
        );
        assert!(w.calls.is_empty());
        // minLeaves 0 fells any log structure.
        let c = with(|c| c.min_leaves = 0);
        assert_eq!(fell(&mut w, &c, [0, 64, 0]).unwrap().logs, 24);
    }

    #[test]
    fn other_woods_and_stripped_logs_are_not_part_of_the_tree() {
        let mut w = World::new();
        w.oak([0, 64, 0], 5);
        w.tree(
            [1, 64, 0],
            5,
            "minecraft:birch_log",
            "minecraft:birch_leaves",
        );
        w.set([0, 69, 0], block("minecraft:stripped_oak_log"));
        assert_eq!(
            fell(&mut w, &Config::default(), [0, 64, 0]).unwrap().logs,
            5
        );
        assert!((64..69).all(|y| w.blocks[&[1, y, 0]].name == "minecraft:birch_log"));
        assert_eq!(w.blocks[&[0, 69, 0]].name, "minecraft:stripped_oak_log");
        // A stripped log is never the start of a felling.
        let mut w = World::new();
        w.tree(
            [0, 64, 0],
            5,
            "minecraft:stripped_oak_log",
            "minecraft:oak_leaves",
        );
        assert_eq!(
            fell(&mut w, &Config::default(), [0, 64, 0]),
            Err(Skip::NotLog)
        );
        // Nor is anything else.
        assert_eq!(
            fell(&mut w, &Config::default(), [5, 64, 5]),
            Err(Skip::NotLog)
        );
    }

    #[test]
    fn oversized_groups_are_left_alone_and_max_logs_is_inclusive() {
        let mut w = World::new();
        w.oak([0, 64, 0], 10);
        let mut c = with(|c| c.max_logs = 9);
        assert_eq!(fell(&mut w.clone(), &c, [0, 64, 0]), Err(Skip::TooBig));
        c.max_logs = 10;
        assert_eq!(fell(&mut w, &c, [0, 64, 0]).unwrap().logs, 10);
    }

    #[test]
    fn nether_crowns_count_only_for_stems() {
        let mut w = World::new();
        w.tree(
            [0, 64, 0],
            6,
            "minecraft:crimson_stem",
            "minecraft:nether_wart_block",
        );
        w.set([0, 70, 0], block("minecraft:shroomlight"));
        assert_eq!(
            fell(&mut w, &Config::default(), [0, 64, 0]).unwrap().logs,
            6
        );
        // Shroomlight lamps on an oak log cabin do not make it a tree.
        let mut w = World::new();
        w.tree([0, 64, 0], 6, "minecraft:oak_log", "minecraft:shroomlight");
        assert_eq!(
            fell(&mut w, &Config::default(), [0, 64, 0]),
            Err(Skip::TooFewLeaves)
        );
    }

    #[test]
    fn huge_mushrooms_only_when_enabled() {
        let mut w = World::new();
        w.tree(
            [0, 64, 0],
            6,
            "minecraft:mushroom_stem",
            "minecraft:red_mushroom_block",
        );
        assert_eq!(
            fell(&mut w.clone(), &Config::default(), [0, 64, 0]),
            Err(Skip::NotLog)
        );
        let c = with(|c| c.mushrooms = true);
        let r = fell(&mut w, &c, [0, 64, 0]).unwrap();
        assert_eq!(r.logs, 6);
        // Caps are not leaves: they stay, even with breakLeaves.
        assert!(w.blocks.values().all(|b| b.kind == Kind::Cap));
        // Mushroom stems without a cap are not a mushroom.
        let mut w = World::new();
        for y in 64..70 {
            w.set([0, y, 0], block("minecraft:mushroom_stem"));
        }
        assert_eq!(fell(&mut w, &c, [0, 64, 0]), Err(Skip::TooFewLeaves));
    }

    #[test]
    fn wood_allow_and_deny_lists() {
        let mut w = World::new();
        w.tree(
            [0, 64, 0],
            6,
            "minecraft:crimson_stem",
            "minecraft:nether_wart_block",
        );
        let deny = Config::from_json(r#"{"woods":{"deny":["crimson","warped"]}}"#).unwrap();
        assert_eq!(fell(&mut w.clone(), &deny, [0, 64, 0]), Err(Skip::Wood));
        let allow = Config::from_json(r#"{"woods":{"allow":["minecraft:oak"]}}"#).unwrap();
        assert_eq!(fell(&mut w.clone(), &allow, [0, 64, 0]), Err(Skip::Wood));
        assert_eq!(fell(&mut oak5(), &allow, [0, 64, 0]).unwrap().logs, 5);
        assert_eq!(fell(&mut oak5(), &deny, [0, 64, 0]).unwrap().logs, 5);
        assert!(Config::from_json(r#"{"woods":{"deny":["Oak Tree"]}}"#).is_err());
    }

    #[test]
    fn sneak_and_tool_modes() {
        fn sneaking(w: &mut World) {
            w.player.sneaking = true;
        }
        fn shovel(w: &mut World) {
            w.tool = Some(Tool {
                item: "minecraft:iron_shovel".into(),
                axe: false,
                tool: true,
                remaining: Some(100),
            });
        }
        fn empty(w: &mut World) {
            w.tool = None;
        }
        fn creative(w: &mut World) {
            w.player.survival = false;
        }
        fn same(_: &mut World) {}
        fn default(_: &mut Config) {}
        fn enables(c: &mut Config) {
            c.sneak = Sneak::Enables;
        }
        fn ignored(c: &mut Config) {
            c.sneak = Sneak::Ignored;
        }
        fn any_tool(c: &mut Config) {
            c.tool = Tools::AnyTool;
        }
        fn any(c: &mut Config) {
            c.tool = Tools::Any;
        }
        fn off(c: &mut Config) {
            c.enabled = false;
        }
        type Case = (fn(&mut World), fn(&mut Config), Result<usize, Skip>);
        let cases: [Case; 12] = [
            (same, default, Ok(5)),
            (sneaking, default, Err(Skip::Sneak)),
            (same, enables, Err(Skip::Sneak)),
            (sneaking, enables, Ok(5)),
            (sneaking, ignored, Ok(5)),
            (same, ignored, Ok(5)),
            (shovel, default, Err(Skip::NoTool)),
            (shovel, any_tool, Ok(5)),
            (empty, any_tool, Err(Skip::NoTool)),
            (empty, any, Ok(5)),
            (creative, default, Err(Skip::NotSurvival)),
            (same, off, Err(Skip::Disabled)),
        ];
        for (i, (world, config, expected)) in cases.into_iter().enumerate() {
            let mut w = oak5();
            world(&mut w);
            let c = with(config);
            assert_eq!(
                fell(&mut w, &c, [0, 64, 0]).map(|r| r.logs),
                expected,
                "case {i}"
            );
        }
        let mut rt = Runtime::default();
        let mut w = oak5();
        rt.set_enabled(w.player.actor, false);
        assert_eq!(
            rt.plan(&mut w, &Config::default(), [0, 64, 0]),
            Err(Skip::PlayerOff)
        );
    }

    #[test]
    fn durability_modes() {
        // (mode, factor) -> wear of a 5-log tree from 250.
        let cases = [
            (Durability::PerLog, 0.5, 245, [false; 5]),
            (Durability::Once, 0.5, 249, [true; 5]),
            (Durability::Multiplier, 0.5, 247, [true; 5]),
            (Durability::Multiplier, 0.1, 249, [true; 5]),
            (Durability::Multiplier, 2.0, 240, [true; 5]),
            (Durability::Free, 0.5, 250, [true; 5]),
        ];
        for (mode, factor, left, kept) in cases {
            let mut w = oak5();
            let c = with(|c| {
                c.durability = mode;
                c.durability_factor = factor;
            });
            assert_eq!(fell(&mut w, &c, [0, 64, 0]).unwrap().logs, 5);
            assert_eq!(w.remaining(), Some(left), "{mode:?} {factor}");
            let flags: Vec<bool> = w.calls.iter().map(|(_, f)| f.keep_durability).collect();
            assert_eq!(flags, kept, "{mode:?}");
        }
        let c = with(|c| {
            c.durability = Durability::Multiplier;
            c.durability_factor = 0.1;
        });
        assert_eq!(c.wear_for(30), 3);
        assert_eq!((1..=30).map(|n| c.wear_of(n)).sum::<u32>(), 3);
    }

    #[test]
    fn keep_durability_stops_before_the_tool_goes_below_it() {
        // 3 points, 1 kept and 1 for the cut log: one log from the top.
        let mut w = oak5();
        w.tool = axe(Some(3));
        let r = fell(&mut w, &Config::default(), [0, 64, 0]).unwrap();
        assert_eq!((r.logs, r.stopped), (1, true));
        assert_eq!(w.remaining(), Some(2));
        // The rest stands on the ground as a stump, the cut log with it.
        assert!((64..68).all(|y| w.blocks.contains_key(&[0, y, 0])));
        assert!(!w.blocks.contains_key(&[0, 68, 0]));
        // keepDurability 10 from 13: two logs, the cut log's point kept.
        let mut w = oak5();
        w.tool = axe(Some(13));
        let c = with(|c| c.keep_durability = 10);
        assert_eq!(fell(&mut w, &c, [0, 64, 0]).unwrap().logs, 2);
        assert_eq!(w.remaining(), Some(11));
        // keepDurability 0 may use the axe up, on the cut log.
        let mut w = oak5();
        w.tool = axe(Some(5));
        let c = with(|c| c.keep_durability = 0);
        let r = fell(&mut w, &c, [0, 64, 0]).unwrap();
        assert_eq!((r.logs, r.stopped, w.tool.is_none()), (5, false, true));
        assert_eq!(w.mined().last(), Some(&[0, 64, 0]));
        // With "once" the whole tree costs one use, so a worn axe fells it all.
        let mut w = oak5();
        w.tool = axe(Some(2));
        let c = with(|c| c.durability = Durability::Once);
        assert_eq!(fell(&mut w, &c, [0, 64, 0]).unwrap().logs, 5);
        assert_eq!(w.remaining(), Some(1));
        // Nothing above can come down: the cut log breaks alone, like
        // vanilla, even if it breaks the axe.
        let mut w = oak5();
        w.tool = axe(Some(1));
        let r = fell(&mut w, &Config::default(), [0, 64, 0]).unwrap();
        assert_eq!((r.logs, r.stopped, w.tool.is_none()), (1, true, true));
        assert_eq!(w.mined(), vec![[0, 64, 0]]);
        // Durability-less items never stop.
        let mut w = oak5();
        w.tool = axe(None);
        assert_eq!(
            fell(&mut w, &Config::default(), [0, 64, 0]).unwrap().logs,
            5
        );
    }

    #[test]
    fn exhaustion_modes() {
        let mut w = oak5();
        fell(&mut w, &Config::default(), [0, 64, 0]).unwrap();
        assert!((w.exhaustion - 0.025).abs() < 1e-6);
        let mut w = oak5();
        let c = with(|c| {
            c.exhaustion = Exhaustion::Once;
            c.break_leaves = true;
        });
        let r = fell(&mut w, &c, [0, 64, 0]).unwrap();
        assert!(r.leaves > 0);
        assert!((w.exhaustion - 0.005).abs() < 1e-6);
    }

    #[test]
    fn drops_at_stump() {
        let mut w = oak5();
        let c = with(|c| {
            c.drops_at_stump = true;
            c.break_leaves = true;
        });
        let r = fell(&mut w, &c, [0, 64, 0]).unwrap();
        assert_eq!(w.drops.len(), r.logs + r.leaves);
        assert!(w.drops.iter().all(|p| *p == [0, 64, 0]));
        assert_eq!(w.gather, None);
        // Off: each where it was.
        let mut w = oak5();
        fell(&mut w, &Config::default(), [0, 64, 0]).unwrap();
        assert_eq!(w.drops, w.mined());
    }

    #[test]
    fn replant_takes_a_sapling_when_the_floor_can_hold_it() {
        let c = with(|c| c.replant = true);
        let mut w = oak5();
        w.inventory.insert("minecraft:oak_sapling".into(), 2);
        assert!(fell(&mut w, &c, [0, 64, 0]).unwrap().replanted);
        assert_eq!(w.blocks[&[0, 64, 0]].name, "minecraft:oak_sapling");
        assert_eq!(w.inventory["minecraft:oak_sapling"], 1);
        // No sapling, or off: nothing.
        let mut w = oak5();
        assert!(!fell(&mut w, &c, [0, 64, 0]).unwrap().replanted);
        let mut w = oak5();
        w.inventory.insert("minecraft:oak_sapling".into(), 2);
        assert!(
            !fell(&mut w, &Config::default(), [0, 64, 0])
                .unwrap()
                .replanted
        );
        // Cut above the stump: the floor is a log.
        let mut w = World::new();
        w.oak([0, 64, 0], 6);
        w.inventory.insert("minecraft:oak_sapling".into(), 2);
        assert!(!fell(&mut w, &c, [0, 65, 0]).unwrap().replanted);
        // A stopped felling does not replant under the logs left standing.
        let mut w = oak5();
        w.tool = axe(Some(3));
        w.inventory.insert("minecraft:oak_sapling".into(), 2);
        assert!(!fell(&mut w, &c, [0, 64, 0]).unwrap().replanted);
    }

    #[test]
    fn break_speed_scales_with_the_tree_and_is_capped() {
        let w = oak5();
        let player = w.player.clone();
        let plan = |c: &Config, w: &World| {
            plan_tree(w, c, &player, true, w.tool.clone(), [0, 64, 0], false).unwrap()
        };
        assert_eq!(
            plan(&Config::default(), &w).slowdown(&Config::default()),
            1.0
        );
        let c = with(|c| c.break_speed = BreakSpeed::Scaled);
        assert_eq!(plan(&c, &w).slowdown(&c), 3.0);
        let c = with(|c| {
            c.break_speed = BreakSpeed::Scaled;
            c.speed_factor = 10.0;
            c.max_slowdown = 20.0;
        });
        assert_eq!(plan(&c, &w).slowdown(&c), 20.0);
    }

    #[test]
    fn speed_answers_are_cached_per_target_and_expire() {
        let mut rt = Runtime::default();
        let key = |pos: Position| SpeedKey {
            actor: [0, 0, 0, 1],
            position: pos,
            state: 7,
            item: Some("minecraft:iron_axe".into()),
        };
        let mut runs = 0;
        let mut ask = |rt: &mut Runtime, pos, now| {
            rt.slowdown(key(pos), now, || {
                runs += 1;
                3.0
            })
        };
        assert_eq!(ask(&mut rt, [0, 64, 0], 0), 3.0);
        for tick in 1..SPEED_CACHE_TICKS {
            ask(&mut rt, [0, 64, 0], tick);
        }
        ask(&mut rt, [0, 64, 0], SPEED_CACHE_TICKS);
        ask(&mut rt, [1, 64, 0], SPEED_CACHE_TICKS);
        rt.forget_speed();
        ask(&mut rt, [1, 64, 0], SPEED_CACHE_TICKS);
        assert_eq!(runs, 4);
    }

    #[test]
    fn failed_or_changed_blocks_are_handled() {
        // The cut log, broken last, fails: the top is down, it stands.
        let mut w = oak5();
        w.fail.insert([0, 64, 0]);
        let r = fell(&mut w, &Config::default(), [0, 64, 0]).unwrap();
        assert_eq!((r.logs, r.stopped), (4, true));
        assert_eq!(w.calls.len(), 5);
        assert!(w.blocks.contains_key(&[0, 64, 0]));
        // A log that fails stops the felling: it and the stump stand.
        let mut w = oak5();
        w.fail.insert([0, 67, 0]);
        let r = fell(&mut w, &Config::default(), [0, 64, 0]).unwrap();
        assert_eq!((r.logs, r.stopped), (1, true));
        assert!((64..68).all(|y| w.blocks.contains_key(&[0, y, 0])));
        // The first log above the cut fails: the cut log breaks alone.
        let mut w = oak5();
        w.fail.insert([0, 68, 0]);
        let r = fell(&mut w, &Config::default(), [0, 64, 0]).unwrap();
        assert_eq!((r.logs, r.stopped), (1, true));
        assert!(!w.blocks.contains_key(&[0, 64, 0]));
        let mut w = oak5();
        w.change_after_first = Some([0, 66, 0]);
        let r = fell(&mut w, &Config::default(), [0, 64, 0]).unwrap();
        assert_eq!((r.logs, r.changed), (4, 1));
        assert_eq!(w.blocks[&[0, 66, 0]].name, "minecraft:oak_planks");
    }

    #[test]
    fn break_leaves_takes_only_leaves_that_would_decay() {
        let config = with(|c| c.break_leaves = true);
        let mut w = World::new();
        w.oak([0, 64, 0], 5);
        let total = w.blocks.values().filter(|b| b.kind != Kind::Log).count();
        let r = fell(&mut w, &config, [0, 64, 0]).unwrap();
        assert_eq!((r.logs, r.leaves), (5, total));
        assert!(w.blocks.is_empty());
        assert!(w.calls[5..].iter().all(|(_, f)| f.keep_durability));
        assert_eq!(w.remaining(), Some(245));
        // A neighbouring tree keeps the leaves within its reach, and
        // persistent leaves are never broken.
        let mut w = World::new();
        w.oak([0, 64, 0], 5).oak([4, 64, 0], 5);
        w.set([-2, 67, -2], persistent("minecraft:oak_leaves"));
        let r = fell(&mut w, &config, [0, 64, 0]).unwrap();
        assert_eq!(r.logs, 5);
        assert!(r.leaves > 0);
        assert!((64..69).all(|y| w.blocks.contains_key(&[4, y, 0])));
        assert!(w.blocks.contains_key(&[2, 67, 0]));
        assert!(w.blocks.contains_key(&[-2, 67, -2]));
        assert!(!w.blocks.contains_key(&[-2, 67, 0]));
        // Leaves next to unloaded chunks might be held by an unseen log.
        let mut w = World::new();
        w.oak([0, 64, 0], 5);
        w.unloaded.insert([3, 67, 0]);
        fell(&mut w, &config, [0, 64, 0]).unwrap();
        assert!(w.blocks.contains_key(&[2, 67, 0]));
        // A stopped felling leaves the canopy alone.
        let mut w = World::new();
        w.oak([0, 64, 0], 5);
        w.tool = axe(Some(3));
        assert_eq!(fell(&mut w, &config, [0, 64, 0]).unwrap().leaves, 0);
    }

    #[test]
    fn decaying_leaves_matches_vanilla_leaf_distance() {
        // A straight line of leaves from a felled log to another log 10 away:
        // leaves 1..=3 from the felled log are more than 6 from the other one.
        let mut w = World::new();
        w.set([0, 64, 0], block("minecraft:oak_log"));
        w.set([10, 64, 0], block("minecraft:oak_log"));
        for x in 1..10 {
            w.set([x, 64, 0], block("minecraft:oak_leaves"));
        }
        let felled = BTreeSet::from([[0, 64, 0]]);
        let got: Vec<i32> = decaying_leaves(&w, &felled)
            .iter()
            .map(|(p, _)| p[0])
            .collect();
        assert_eq!(got, vec![1, 2, 3]);
    }

    #[test]
    fn single_logs_are_left_to_the_game() {
        let mut w = World::new();
        w.tree(
            [0, 64, 0],
            1,
            "minecraft:jungle_log",
            "minecraft:jungle_leaves",
        );
        assert_eq!(
            fell(&mut w, &Config::default(), [0, 64, 0]),
            Err(Skip::Lone)
        );
        let c = with(|c| c.break_leaves = true);
        let r = fell(&mut w, &c, [0, 64, 0]).unwrap();
        assert_eq!(r.logs, 1);
        assert!(r.leaves > 0);
    }

    #[test]
    fn coordinates_at_integer_limits_do_not_wrap() {
        let mut w = World::new();
        let p = [i32::MAX, 0, i32::MIN];
        w.set(p, block("minecraft:oak_log"));
        assert_eq!(find_logs(&w, p, 10).unwrap().len(), 1);
    }

    #[test]
    fn configuration_parses_defaults_and_rejects_bad_values() {
        assert_eq!(Config::from_json("{}").unwrap(), Config::default());
        let c = Config::from_json(
            r#"{"enabled":true,"sneak":"enables","tool":"anyTool","maxLogs":64,"minLeaves":0,
                "breakLeaves":true,"breakSpeed":"scaled","speedFactor":1.5,"maxSlowdown":8,
                "durability":"multiplier","durabilityFactor":0.25,"keepDurability":5,
                "dropsAtStump":true,"replant":true,"exhaustion":"once",
                "woods":{"allow":["oak"],"deny":["minecraft:warped"]},"mushrooms":true}"#,
        )
        .unwrap();
        assert_eq!(c.sneak, Sneak::Enables);
        assert_eq!(c.tool, Tools::AnyTool);
        assert_eq!((c.max_logs, c.min_leaves, c.keep_durability), (64, 0, 5));
        assert_eq!(c.break_speed, BreakSpeed::Scaled);
        assert_eq!((c.speed_factor, c.max_slowdown), (1.5, 8.0));
        assert_eq!(
            (c.durability, c.durability_factor),
            (Durability::Multiplier, 0.25)
        );
        assert!(c.break_leaves && c.drops_at_stump && c.replant && c.mushrooms);
        assert_eq!(c.exhaustion, Exhaustion::Once);
        assert!(c.woods.permits("minecraft:oak") && !c.woods.permits("minecraft:birch"));
        let free =
            Config::from_json(r#"{"durability":"none","tool":"any","sneak":"ignored"}"#).unwrap();
        assert_eq!(
            (free.durability, free.tool, free.sneak),
            (Durability::Free, Tools::Any, Sneak::Ignored)
        );
        for bad in [
            r#"{"maxLogs":0}"#,
            r#"{"maxLogs":5000}"#,
            r#"{"minLeaves":2000}"#,
            r#"{"maxLogs":-1}"#,
            r#"{"mustSneak":true}"#,
            r#"{"requireAxe":false}"#,
            r#"{"sneak":"sometimes"}"#,
            r#"{"tool":"pickaxe"}"#,
            r#"{"durability":"free"}"#,
            r#"{"breakSpeed":"slow"}"#,
            r#"{"speedFactor":-1}"#,
            r#"{"maxSlowdown":0.5}"#,
            r#"{"durabilityFactor":11}"#,
            r#"{"keepDurability":-1}"#,
            r#"{"woods":{"deny":[""]}}"#,
            r#"{"woods":{"block":["oak"]}}"#,
            "not json",
        ] {
            assert!(Config::from_json(bad).is_err(), "{bad}");
        }
        let json = serde_json::to_string(&Config::default()).unwrap();
        for key in [
            "\"maxLogs\":256",
            "\"breakLeaves\":false",
            "\"sneak\":\"disables\"",
            "\"tool\":\"axe\"",
            "\"durability\":\"perLog\"",
            "\"breakSpeed\":\"single\"",
            "\"exhaustion\":\"perLog\"",
        ] {
            assert!(json.contains(key), "{key} in {json}");
        }
        assert_eq!(Config::from_json(&json).unwrap(), Config::default());
    }

    #[test]
    fn commands_parse_and_toggle_per_player() {
        assert_eq!(parse_command("treefall"), Some(Ok(Command::Status)));
        assert_eq!(parse_command("/treefall status"), Some(Ok(Command::Status)));
        assert_eq!(parse_command("treefall  on "), Some(Ok(Command::On)));
        assert_eq!(parse_command("treefall off"), Some(Ok(Command::Off)));
        assert_eq!(parse_command("treefall toggle"), Some(Ok(Command::Toggle)));
        assert_eq!(parse_command("treefall reload"), Some(Ok(Command::Reload)));
        assert_eq!(parse_command("treefall maybe"), Some(Err(USAGE)));
        assert_eq!(parse_command("treefall on now"), Some(Err(USAGE)));
        assert_eq!(parse_command("veinminer on"), None);
        assert_eq!(parse_command("treefallx"), None);
        assert!(is_command("/treefall") && !is_command("give @s oak_log"));

        let (a, b) = ([0, 0, 0, 1], [0, 0, 0, 2]);
        let mut rt = Runtime::default();
        let c = Config::default();
        assert_eq!(rt.command(a, Command::Status, &c), "TreeFall: on");
        assert_eq!(rt.command(a, Command::Toggle, &c), "TreeFall: off");
        assert!(!rt.enabled(a) && rt.enabled(b));
        assert_eq!(rt.command(a, Command::Toggle, &c), "TreeFall: on");
        assert_eq!(rt.command(b, Command::Off, &c), "TreeFall: off");
        assert_eq!(rt.command(b, Command::Off, &c), "TreeFall: off");
        assert_eq!(rt.command(b, Command::On, &c), "TreeFall: on");
        let off = with(|c| c.enabled = false);
        assert!(
            rt.command(a, Command::Status, &off)
                .ends_with("configuration)")
        );
    }

    /// The configuration a click saves (a reload or nothing is a failure).
    fn saved(loaded: &Loaded, setting: Setting) -> Config {
        let button = page(loaded).iter().position(|s| *s == setting).unwrap();
        match click(loaded, button) {
            Some(Effect::Save(c)) => c,
            other => panic!("{setting:?}: {other:?}"),
        }
    }

    #[test]
    fn settings_page_lists_every_key_and_the_factors_in_their_mode() {
        let defaults: Loaded = Ok(Config::default());
        assert_eq!(
            labels(&defaults),
            [
                "Felling: ON",
                "Tool: axe",
                "Sneaking: cuts a single log",
                "Largest tree: 256 logs",
                "Leaves needed: 5",
                "Break the leaves: OFF",
                "Cut time: as one log",
                "Tool wear: every log",
                "Keep durability: 1",
                "Hunger: every log",
                "Drops at the stump: OFF",
                "Replant a sapling: OFF",
                "Huge mushrooms: OFF",
                "Woods: all (config.json, click to reload)",
                "Reset to defaults (woods kept)",
            ]
        );
        // The factors only show in the mode they are for.
        let scaled: Loaded = Ok(with(|c| {
            c.break_speed = BreakSpeed::Scaled;
            c.durability = Durability::Multiplier;
        }));
        let shown = labels(&scaled);
        assert_eq!(shown.len(), 18);
        assert_eq!(
            shown[6..9],
            [
                "Cut time: grows with the tree",
                "Extra time per log: 0.5x",
                "Longest cut: 20x one log",
            ]
        );
        assert_eq!(
            shown[9..11],
            ["Tool wear: logs x factor", "Wear factor: 0.5 per log"]
        );
        // Every key of the file but the woods lists has its button.
        let page = page(&scaled);
        assert_eq!(page.len(), 18);
        let json = serde_json::to_value(Config::default()).unwrap();
        assert_eq!(json.as_object().unwrap().len(), page.len() - 1);
        // Clicks map to the button under the pointer, nothing past the last.
        assert_eq!(
            click(&defaults, 0),
            Some(Effect::Save(with(|c| c.enabled = false)))
        );
        assert_eq!(click(&defaults, 13), Some(Effect::Reload));
        assert_eq!(click(&defaults, 15), None);
        // Special values say what they do; the woods give their counts.
        let mut c = with(|c| {
            c.min_leaves = 0;
            c.keep_durability = 0;
        });
        c.woods.allow.insert("oak".into());
        c.woods
            .deny
            .extend(["birch".to_owned(), "minecraft:spruce".to_owned()]);
        let shown = labels(&Ok(c));
        assert!(shown.contains(&"Leaves needed: 0 (any log structure)".to_owned()));
        assert!(shown.contains(&"Keep durability: 0 (tool may break)".to_owned()));
        assert!(
            shown.contains(&"Woods: 1 allowed, 2 denied (config.json, click to reload)".to_owned())
        );
    }

    #[test]
    fn settings_clicks_cycle_every_value_and_stay_valid() {
        let start = with(|c| {
            c.break_speed = BreakSpeed::Scaled;
            c.durability = Durability::Multiplier;
        });
        // Each click changes its own key only, and comes back round.
        // A setting, how many values it has, and how to read it.
        type Cycle = (Setting, usize, fn(&Config) -> String);
        let cycles: [Cycle; 15] = [
            (Setting::Enabled, 2, |c| c.enabled.to_string()),
            (Setting::Tool, 3, |c| format!("{:?}", c.tool)),
            (Setting::Sneak, 3, |c| format!("{:?}", c.sneak)),
            (Setting::MaxLogs, MAX_LOGS_STEPS.len(), |c| {
                c.max_logs.to_string()
            }),
            (Setting::MinLeaves, MIN_LEAVES_STEPS.len(), |c| {
                c.min_leaves.to_string()
            }),
            (Setting::BreakLeaves, 2, |c| c.break_leaves.to_string()),
            (Setting::SpeedFactor, SPEED_FACTOR_STEPS.len(), |c| {
                c.speed_factor.to_string()
            }),
            (Setting::MaxSlowdown, MAX_SLOWDOWN_STEPS.len(), |c| {
                c.max_slowdown.to_string()
            }),
            (Setting::WearFactor, DURABILITY_FACTOR_STEPS.len(), |c| {
                c.durability_factor.to_string()
            }),
            (Setting::KeepDurability, KEEP_DURABILITY_STEPS.len(), |c| {
                c.keep_durability.to_string()
            }),
            (Setting::Hunger, 2, |c| format!("{:?}", c.exhaustion)),
            (Setting::DropsAtStump, 2, |c| c.drops_at_stump.to_string()),
            (Setting::Replant, 2, |c| c.replant.to_string()),
            (Setting::Mushrooms, 2, |c| c.mushrooms.to_string()),
            (Setting::CutTime, 2, |c| format!("{:?}", c.break_speed)),
        ];
        for (setting, values, read) in cycles {
            let mut c = start.clone();
            let mut seen = BTreeSet::new();
            for _ in 0..values {
                seen.insert(read(&c));
                c.validate().unwrap();
                let next = saved(&Ok(c.clone()), setting);
                // Nothing else changed.
                let mut back = next.clone();
                match setting {
                    Setting::Enabled => back.enabled = c.enabled,
                    Setting::Tool => back.tool = c.tool,
                    Setting::Sneak => back.sneak = c.sneak,
                    Setting::MaxLogs => back.max_logs = c.max_logs,
                    Setting::MinLeaves => back.min_leaves = c.min_leaves,
                    Setting::BreakLeaves => back.break_leaves = c.break_leaves,
                    Setting::SpeedFactor => back.speed_factor = c.speed_factor,
                    Setting::MaxSlowdown => back.max_slowdown = c.max_slowdown,
                    Setting::WearFactor => back.durability_factor = c.durability_factor,
                    Setting::KeepDurability => back.keep_durability = c.keep_durability,
                    Setting::Hunger => back.exhaustion = c.exhaustion,
                    Setting::DropsAtStump => back.drops_at_stump = c.drops_at_stump,
                    Setting::Replant => back.replant = c.replant,
                    Setting::Mushrooms => back.mushrooms = c.mushrooms,
                    _ => back.break_speed = c.break_speed,
                }
                assert_eq!(back, c, "{setting:?}");
                c = next;
            }
            assert_eq!(seen.len(), values, "{setting:?} {seen:?}");
            assert_eq!(c, start, "{setting:?} comes back round");
        }
        // Tool wear cycles its four modes (its factor's button comes and goes).
        let mut c = Config::default();
        let mut modes = Vec::new();
        for _ in 0..4 {
            c = saved(&Ok(c), Setting::Wear);
            modes.push(c.durability);
        }
        assert_eq!(
            modes,
            [
                Durability::Once,
                Durability::Multiplier,
                Durability::Free,
                Durability::PerLog
            ]
        );
        // A hand-written value between two steps goes to the step above it,
        // the largest back to the smallest.
        let c = with(|c| {
            c.max_logs = 300;
            c.speed_factor = 0.3;
            c.break_speed = BreakSpeed::Scaled;
            c.keep_durability = 100_000;
        });
        assert_eq!(saved(&Ok(c.clone()), Setting::MaxLogs).max_logs, 512);
        assert_eq!(
            saved(&Ok(c.clone()), Setting::SpeedFactor).speed_factor,
            0.5
        );
        assert_eq!(saved(&Ok(c), Setting::KeepDurability).keep_durability, 0);
        // Every step is a valid value, and the defaults are steps.
        let d = Config::default();
        assert!(MAX_LOGS_STEPS.contains(&d.max_logs) && MIN_LEAVES_STEPS.contains(&d.min_leaves));
        assert!(SPEED_FACTOR_STEPS.contains(&d.speed_factor));
        assert!(MAX_SLOWDOWN_STEPS.contains(&d.max_slowdown));
        assert!(DURABILITY_FACTOR_STEPS.contains(&d.durability_factor));
        assert!(KEEP_DURABILITY_STEPS.contains(&d.keep_durability));
    }

    #[test]
    fn settings_reset_reload_and_a_broken_file() {
        // Reset keeps the woods lists, which only the file holds.
        let mut c = with(|c| {
            c.max_logs = 64;
            c.tool = Tools::Any;
        });
        c.woods.deny.insert("birch".into());
        let reset = saved(&Ok(c.clone()), Setting::Defaults);
        assert_eq!(reset.woods, c.woods);
        assert_eq!(
            Config {
                woods: Woods::default(),
                ..reset
            },
            Config::default()
        );
        // The woods button reads the file again.
        assert_eq!(Setting::Woods.click(&Ok(c)), Effect::Reload);
        // A broken file: its error (cut short) and a reset, nothing else.
        let error =
            "unknown field `mustSneak`, expected one of `enabled`, `sneak`, `tool`".to_owned();
        let broken: Loaded = Err(error.clone());
        assert_eq!(page(&broken), [Setting::Broken, Setting::Defaults]);
        let shown = labels(&broken);
        assert_eq!(
            shown[0],
            "config.json: unknown field `mustSneak`, expected one... (click to reload)"
        );
        assert_eq!(shown[1], "Reset to defaults");
        assert_eq!(click(&broken, 0), Some(Effect::Reload));
        assert_eq!(click(&broken, 1), Some(Effect::Save(Config::default())));
        assert_eq!(click(&broken, 2), None);
        let short: Loaded = Err("expected value at line 1 column 1".into());
        assert_eq!(
            labels(&short)[0],
            "config.json: expected value at line 1 column 1 (click to reload)"
        );
    }

    #[test]
    fn settings_are_saved_as_the_file_they_are_read_from() {
        let mut c = with(|c| {
            c.durability = Durability::Free;
            c.speed_factor = 0.1;
            c.keep_durability = 25;
        });
        c.woods.allow.insert("minecraft:cherry".into());
        let json = c.to_json();
        assert!(json.ends_with("}\n"));
        assert!(json.contains("\"durability\": \"none\""));
        assert!(json.contains("\"speedFactor\": 0.1"));
        assert_eq!(Config::from_json(&json).unwrap(), c);
        assert_eq!(
            Config::from_json(&Config::default().to_json()).unwrap(),
            Config::default()
        );
    }
}
