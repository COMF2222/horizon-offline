// Copyright (c) 2026 d3grando
// SPDX-License-Identifier: MIT
// TreeFall's adapter to the game (Horizon 142388e1c136e622652ec7505af15d8ad195e041,
// see horizon-mixin.toml). Injected as `crate::game::treefall_native`; binds the
// game-independent engine in `crate::game::treefall` (runtime.rs) to the running
// game, and keeps the settings where the native hook points' helpers do.
use super::treefall::{
    self as tf, Block, Blocks, BreakSpeed, Command, Config, Effect, Host, Kind, Loaded, MineFlags,
    Player, Position, Runtime, SpeedKey, Tool, fell_tree, plan_tree,
};
use super::{Game, Result};
use std::sync::atomic::{AtomicU8, Ordering};

/// What the window's game and single player's server thread share (single
/// player runs on its own "Server thread", one process): the configuration,
/// the players who turned TreeFall off, and the last felling-speed answer.
/// The window mines on its own side too, so it needs the same answers to cut
/// as slowly as the server (`breakSpeed: "scaled"`).
struct Shared {
    runtime: Runtime,
    /// `config.json` as last read; None until something needs it.
    config: Option<Loaded>,
}
static SHARED: std::sync::Mutex<Shared> = std::sync::Mutex::new(Shared {
    runtime: Runtime::new(),
    config: None,
});
/// Options > Mods' on/off as the window's game last saw it (`WINDOW_UNSEEN`,
/// `WINDOW_ON`, `WINDOW_OFF`): single player's server thread keeps the
/// options it started with, so it follows the window when there is one.
static WINDOW: AtomicU8 = AtomicU8::new(WINDOW_UNSEEN);
const WINDOW_UNSEEN: u8 = 0;
const WINDOW_ON: u8 = 1;
const WINDOW_OFF: u8 = 2;

thread_local! {
    // Where the drops of the blocks being felled go (`dropsAtStump`).
    static GATHER: std::cell::Cell<Option<Position>> = const { std::cell::Cell::new(None) };
    // Set while `destroy_progress` runs the game's own computation.
    static MEASURING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
/// Never held while the game runs a hook of ours.
fn shared<T>(run: impl FnOnce(&mut Shared) -> T) -> T {
    let mut shared = SHARED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    run(&mut shared)
}

/// `config.json` in the mod's settings folder; missing is the defaults.
fn read_config(game: &Game<'_>) -> Loaded {
    match game.native_setting(tf::ID, tf::CONFIG_FILE) {
        Some(text) => Config::from_json(&text).map_err(|e| format!("{}: {e}", tf::CONFIG_FILE)),
        None => Ok(Config::default()),
    }
}
/// Puts `loaded` in effect, for every player at once.
fn apply(loaded: Loaded) {
    if let Err(error) = &loaded {
        eprintln!("TreeFall disabled: {error}");
    }
    shared(|s| {
        s.config = Some(loaded);
        s.runtime.forget_speed();
    });
}
/// The configuration as read (once, by whichever thread needs it first).
fn loaded(game: &Game<'_>) -> Loaded {
    if let Some(loaded) = shared(|s| s.config.clone()) {
        return loaded;
    }
    let read = read_config(game);
    shared(|s| {
        s.config
            .get_or_insert_with(|| {
                if let Err(error) = &read {
                    eprintln!("TreeFall disabled: {error}");
                }
                read
            })
            .clone()
    })
}
/// The configuration in effect: a broken file disables the mod (fails closed).
fn config(game: &Game<'_>) -> Config {
    loaded(game).unwrap_or_else(|_| Config {
        enabled: false,
        ..Config::default()
    })
}
/// Not turned off in Options > Mods: every hook passes through untouched
/// when this is false (the list and settings hooks still run, so the row
/// can turn it back on).
fn on(game: &Game<'_>) -> bool {
    let own = game.native_on(tf::ID);
    if game.native_window() {
        let seen = if own { WINDOW_ON } else { WINDOW_OFF };
        WINDOW.store(seen, Ordering::Relaxed);
        return own;
    }
    match WINDOW.load(Ordering::Relaxed) {
        WINDOW_UNSEEN => own,
        seen => seen == WINDOW_ON,
    }
}

/// Read-only view of the game: blocks, the acting player and their tool.
struct View<'g, 'r>(&'g Game<'r>);
impl Blocks for View<'_, '_> {
    fn block(&self, p: Position) -> Option<Block> {
        let game = self.0;
        if p[1] < game.world.shape.min_y
            || p[1] >= game.world.shape.max_y()
            || !game.world.loaded(p[0], p[2])
        {
            return None;
        }
        let state = game.world.block(p[0], p[1], p[2]);
        let facts = game.res.facts().get(state);
        let items = game.res.items();
        let name: &str = &facts.name;
        let kind = if game.world.is_air(state) || facts.hardness < 0.0 {
            Kind::Other
        } else if items.block_in_tag("minecraft:logs", name) {
            Kind::Log
        } else if items.block_in_tag("minecraft:leaves", name) {
            // LeavesBlock.PERSISTENT: placed by a player.
            let persistent = game
                .pack
                .assets
                .registry
                .state(state)
                .and_then(|s| s.properties.get("persistent"))
                .map(String::as_str)
                == Some("true");
            Kind::Leaves {
                natural: !persistent,
            }
        } else if items.block_in_tag("minecraft:wart_blocks", name)
            || name == "minecraft:shroomlight"
        {
            Kind::Crown
        } else if name == "minecraft:mushroom_stem" {
            Kind::MushroomStem
        } else if name == "minecraft:red_mushroom_block" || name == "minecraft:brown_mushroom_block"
        {
            Kind::Cap
        } else {
            Kind::Other
        };
        Some(Block {
            name: name.to_owned(),
            state: state.0,
            kind,
        })
    }
}
impl View<'_, '_> {
    fn player(&self, actor: [i32; 4]) -> Player {
        let game = self.0;
        Player {
            actor,
            // Survival or adventure in a world of this game's own: played in
            // the frame, hosted, or single player's server thread and the
            // window joined to it (which mines on its side too). Never as a
            // client of another computer's server.
            survival: game.has_singleplayer_server()
                && game.player.is_some()
                && !game.creative
                && !game.spectator(),
            // Sneak is the left shift key; a LAN guest's sneak flag is mapped
            // onto it for their turn (`enter_guest`).
            sneaking: game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft)
                || game.player.as_ref().is_some_and(|p| p.shift_key_down),
        }
    }
    /// The acting player, as the server knows them: a LAN guest (single
    /// player's own window is one of its server thread), the window's profile
    /// when joined to a server, else the world's player. `acting_player_uuid`
    /// without its `&mut`.
    fn actor(&self) -> [i32; 4] {
        let game = self.0;
        match (game.lan_acting_uuid, &game.remote) {
            (Some(guest), _) => horizon_client::lan::playerdata::uuid_ints(guest),
            (None, Some(remote)) => horizon_client::lan::playerdata::uuid_ints(remote.profile()),
            (None, None) => game.level.player_uuid.unwrap_or_default(),
        }
    }
    fn tool(&self) -> Option<Tool> {
        let stack = self.0.held()?;
        let items = self.0.res.items();
        let info = items.get(&stack.item);
        let max = info.map_or(0, |i| i.max_damage);
        let axe = match items.item_tags.get("minecraft:axes") {
            Some(axes) => axes.contains(&*stack.item),
            None => stack.item.ends_with("_axe"),
        };
        Some(Tool {
            item: stack.item.to_string(),
            axe,
            tool: info.is_some_and(|i| !i.tool.is_empty() || i.tool_damage_per_block.is_some()),
            remaining: (max > 0).then_some(max.saturating_sub(stack.damage)),
        })
    }
}

struct Native<'g, 'r> {
    game: &'g mut Game<'r>,
}
impl Blocks for Native<'_, '_> {
    fn block(&self, p: Position) -> Option<Block> {
        View(&*self.game).block(p)
    }
}
impl Host for Native<'_, '_> {
    type Error = Box<dyn std::error::Error>;
    fn player(&mut self) -> Player {
        let actor = self.game.acting_player_uuid();
        View(&*self.game).player(actor)
    }
    fn tool(&self) -> Option<Tool> {
        View(&*self.game).tool()
    }
    fn mine(&mut self, p: Position, flags: MineFlags) -> Result<bool> {
        let before = self.block(p);
        let selected = self.game.selected;
        let held = flags
            .keep_durability
            .then(|| self.game.inventory[selected].clone());
        let exhaustion = flags
            .keep_exhaustion
            .then(|| self.game.player.as_ref().map(|p| p.health.food.exhaustion))
            .flatten();
        // Clear the damage for the break and put it back afterwards.
        if flags.keep_durability
            && let Some(stack) = self.game.inventory[selected].as_mut()
        {
            stack.damage = 0;
        }
        let result = self.game.break_block_as_player(p);
        if let Some(original) = held {
            match (self.game.inventory[selected].as_mut(), original) {
                (Some(now), Some(was)) if now.item == was.item => now.damage = was.damage,
                (None, Some(was)) => self.game.inventory[selected] = Some(was),
                _ => {}
            }
        }
        if let Some(exhaustion) = exhaustion
            && let Some(player) = self.game.player.as_mut()
        {
            player.health.food.exhaustion = exhaustion;
        }
        result?;
        Ok(before.is_some() && self.block(p) != before)
    }
    /// `ItemStack.hurtAndBreak` for the held tool, as `mine_block_with_held`
    /// does it (Unbreaking applies).
    fn wear(&mut self, amount: u32) {
        let selected = self.game.selected;
        if amount == 0 {
            return;
        }
        let Some(stack) = self.game.inventory[selected].clone() else {
            return;
        };
        let max = self
            .game
            .res
            .items()
            .get(&stack.item)
            .map_or(0, |i| i.max_damage);
        if max == 0 {
            return;
        }
        let loss = self.game.durability_loss(&stack, amount);
        if let Some(held) = self.game.inventory[selected].as_mut() {
            held.damage += loss;
            if held.damage >= max {
                self.game.inventory[selected] = None;
            }
        }
    }
    fn gather(&mut self, at: Option<Position>) {
        GATHER.set(at);
    }
    fn replant(&mut self, at: Position, item: &str) -> bool {
        let game = &mut *self.game;
        let Some(state) = game.pack.assets.registry.default_state(item) else {
            return false;
        };
        let slot = (0..game.inventory.len())
            .find(|&i| game.inventory[i].as_ref().is_some_and(|s| &*s.item == item));
        let offhand = game.offhand.as_ref().is_some_and(|s| &*s.item == item);
        if slot.is_none() && !offhand {
            return false;
        }
        let free = game
            .res
            .facts()
            .is_air(game.world.block(at[0], at[1], at[2]));
        let survives = free
            && game.with_level(|b, level, ticks| {
                b.can_survive(
                    level,
                    ticks,
                    horizon_core::position::BlockPos::new(at[0], at[1], at[2]),
                    state,
                )
            });
        if !survives || game.level_set(at, state).is_err() {
            return false;
        }
        let stack = match slot {
            Some(i) => &mut game.inventory[i],
            None => &mut game.offhand,
        };
        if let Some(s) = stack.as_mut() {
            s.count = s.count.saturating_sub(1);
            if s.count == 0 {
                *stack = None;
            }
        }
        true
    }
}

impl Game<'_> {
    /// `Game::break_block`'s HEAD: Some when the break fells a tree (the whole
    /// break, the clicked block included, happened here), None to let the
    /// game (and lower-priority hooks) break the block as usual. Only the
    /// game that owns the world fells (not a window joined to a server).
    pub(super) fn treefall_break_block(&mut self, pos: Position) -> Option<Result<()>> {
        if !on(self) || self.remote.is_some() {
            return None;
        }
        let config = config(self);
        let plan = shared(|s| s.runtime.plan(&mut Native { game: self }, &config, pos)).ok()?;
        shared(|s| s.runtime.forget_speed());
        Some(self.with_player_actor(|game| {
            let result = fell_tree(&mut Native { game }, &config, &plan);
            GATHER.set(None);
            result.map(|_| ())
        }))
    }

    /// `Game::destroy_progress`'s HEAD (`breakSpeed: "scaled"`): the cut
    /// log's progress per tick divided by the tree's slowdown; None leaves the
    /// game's own value. Asked by the game that owns the world and, in single
    /// player, by the window as it mines too, so both cut as slowly.
    pub(super) fn treefall_destroy_progress(&mut self, pos: Position) -> Option<f32> {
        if MEASURING.get() || !on(self) {
            return None;
        }
        let config = config(self);
        if config.break_speed == BreakSpeed::Single || !config.enabled {
            return None;
        }
        let view = View(self);
        let actor = view.actor();
        let tool = view.tool();
        let key = SpeedKey {
            actor,
            position: pos,
            state: self.world.block(pos[0], pos[1], pos[2]).0,
            item: tool.as_ref().map(|t| t.item.clone()),
        };
        let slowdown = shared(|s| {
            let enabled = s.runtime.enabled(actor);
            s.runtime.slowdown(key, self.world.game_time, || {
                let player = view.player(actor);
                plan_tree(&view, &config, &player, enabled, tool, pos, false)
                    .map_or(1.0, |plan| plan.slowdown(&config))
            })
        });
        if slowdown <= 1.0 {
            return None;
        }
        struct Measuring;
        impl Drop for Measuring {
            fn drop(&mut self) {
                MEASURING.set(false);
            }
        }
        MEASURING.set(true);
        let _measuring = Measuring;
        Some(self.destroy_progress(pos) / slowdown)
    }

    /// `Game::pop_resource`'s HEAD (`dropsAtStump`): while a tree is felled,
    /// pop `stack` at the stump instead; false leaves it where it is. Only a
    /// felling sets the stump, so this passes through when TreeFall is off.
    pub(super) fn treefall_pop_at_stump(
        &mut self,
        pos: Position,
        stack: &horizon_client::items::ItemStack,
    ) -> bool {
        let Some(stump) = GATHER.get() else {
            return false;
        };
        if stump == pos {
            return false;
        }
        GATHER.set(None);
        self.pop_resource(stump, stack.clone());
        GATHER.set(Some(stump));
        true
    }

    /// `native_command` (a hook point): `/treefall [status|on|off|toggle|reload]`;
    /// true when it was TreeFall's. The game that owns the world runs it: a
    /// window joined to a server (single player's server thread included)
    /// leaves the line to the server, as a guest's command reaches it there.
    pub(super) fn treefall_command(&mut self, command: &str) -> bool {
        if !on(self) || self.remote.is_some() {
            return false;
        }
        let Some(parsed) = tf::parse_command(command) else {
            return false;
        };
        let message = match parsed {
            Err(usage) => {
                self.command_failure(horizon_client::text::Text::literal(usage));
                return true;
            }
            Ok(Command::Reload) => {
                if self.ui.registries.permission < 2 {
                    self.command_failure(horizon_client::text::Text::literal(
                        "TreeFall reload requires permission level 2",
                    ));
                    return true;
                }
                match read_config(self) {
                    Ok(config) => {
                        apply(Ok(config));
                        "TreeFall configuration reloaded".to_owned()
                    }
                    Err(error) => {
                        self.command_failure(horizon_client::text::Text::literal(error));
                        return true;
                    }
                }
            }
            Ok(command) => {
                let actor = self.acting_player_uuid();
                let config = config(self);
                shared(|s| s.runtime.command(actor, command, &config))
            }
        };
        self.system_message(horizon_client::text::Text::literal(message));
        true
    }

    /// `lan_actions::Game::guest_command`'s HEAD: queue a LAN guest's
    /// `/treefall` so it runs as that guest (their own on/off state), whatever
    /// their permission level; true when it was one.
    pub(super) fn treefall_guest_command(
        &mut self,
        host: &mut super::lan::LanHost,
        conn: u32,
        command: &str,
    ) -> bool {
        if !on(self) || !tf::is_command(command) {
            return false;
        }
        if let Some(permission) = host.hub.guest(conn).map(|g| g.permission) {
            host.commands.push(super::lan_actions::GuestCommand {
                conn,
                command: command.trim().trim_start_matches('/').to_owned(),
                permission,
                integrated: host.hub.config.integrated,
                quiet: false,
            });
        }
        true
    }

    /// `native_tick` (a hook point): the window's game notes each tick
    /// whether TreeFall is turned off in Options > Mods, for its server thread.
    pub(super) fn treefall_tick(&self) {
        if self.native_window() {
            on(self);
        }
    }

    /// `native_settings` (a hook point): the settings page's buttons.
    pub(super) fn treefall_settings(&self) -> Vec<String> {
        tf::labels(&loaded(self))
    }

    /// `native_settings_click` (a hook point): change the setting, write
    /// `config.json` and put it in effect at once (or read the file again).
    pub(super) fn treefall_settings_click(&mut self, button: usize) {
        match tf::click(&loaded(self), button) {
            Some(Effect::Save(config)) => {
                self.native_set_setting(tf::ID, tf::CONFIG_FILE, &config.to_json());
                apply(Ok(config));
            }
            Some(Effect::Reload) => apply(read_config(self)),
            None => {}
        }
    }
}
