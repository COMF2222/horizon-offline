// Copyright (c) 2026 d3grando
// SPDX-License-Identifier: MIT
// Mouse Tricks' adapter to the game (Horizon 142388e1c136e622652ec7505af15d8ad195e041,
// see horizon-mixin.toml). Injected as `crate::game::mousetricks_native`; binds
// the game-independent drags, wheel moves and sorts in `crate::game::mousetricks`
// (runtime.rs) to the window's inventory screens.
//
// Every item it moves goes through the game's own container click path
// (`menu_click`: the verified menu engine in a world of this game's own, a
// predicted `ContainerClick` per click on a server or a LAN host), the clicks
// a hand makes, so a server accepts them as it accepts any player's.
use super::mousetricks::{
    self as mt, Button, Click, Config, Drag, Effect, Kind, Layout, Loaded, Menu, Moment, Press,
    Sorting, Wheel,
};
use super::{Game, Screen};
use horizon_client::bundle::Contents;
use horizon_client::containers::BlockEntity;
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::keybinds;
use horizon_client::menu::Input;
use horizon_client::stations::hover_name;
use std::cell::RefCell;
use std::time::Duration;
use winit::event::MouseButton;

/// The window's state: mouse input only ever reaches the window's game, on
/// its own thread.
#[derive(Default)]
struct State {
    /// `config.json` as last read; None until something needs it.
    config: Option<Loaded>,
    /// The drag from the last press, until its release.
    drag: Option<Drag>,
    wheel: Wheel,
    /// A sort still under way (paced, on a server).
    sorting: Option<Sorting<ItemStack>>,
}
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}
/// Never held while the game runs.
fn state<T>(run: impl FnOnce(&mut State) -> T) -> T {
    STATE.with(|s| run(&mut s.borrow_mut()))
}

/// `config.json` in the mod's settings folder; missing is the defaults.
fn read_config(game: &Game<'_>) -> Loaded {
    match game.native_setting(mt::ID, mt::CONFIG_FILE) {
        Some(text) => Config::from_json(&text).map_err(|e| format!("{}: {e}", mt::CONFIG_FILE)),
        None => Ok(Config::default()),
    }
}
/// Puts `loaded` in effect.
fn apply(loaded: Loaded) {
    if let Err(error) = &loaded {
        eprintln!("Mouse Tricks disabled: {error}");
    }
    state(|s| s.config = Some(loaded));
}
/// The configuration as read (once).
fn loaded(game: &Game<'_>) -> Loaded {
    if let Some(loaded) = state(|s| s.config.clone()) {
        return loaded;
    }
    let read = read_config(game);
    if let Err(error) = &read {
        eprintln!("Mouse Tricks disabled: {error}");
    }
    state(|s| s.config.get_or_insert(read).clone())
}
/// The configuration in effect; None when the file is broken (fails closed:
/// the screens click as the game's own).
fn config(game: &Game<'_>) -> Option<Config> {
    loaded(game).ok()
}
/// Not turned off in Options > Mods, in the window's game: every hook
/// passes through untouched otherwise (the list and settings hooks still
/// run, so the row can turn it back on).
fn on(game: &Game<'_>) -> bool {
    game.native_window() && game.native_on(mt::ID)
}

/// The open screen through the game's slots and clicks.
struct Native<'g, 'r> {
    game: &'g mut Game<'r>,
}
impl Menu for Native<'_, '_> {
    type Stack = ItemStack;
    fn slot(&mut self, slot: usize) -> Option<ItemStack> {
        self.game.slot_view(slot)
    }
    fn carried(&self) -> Option<ItemStack> {
        self.game.carried.clone()
    }
    fn count(&self, stack: &ItemStack) -> u32 {
        stack.count
    }
    fn max(&self, stack: &ItemStack) -> u32 {
        self.game.max_stack(stack).max(1)
    }
    fn same(&self, a: &ItemStack, b: &ItemStack) -> bool {
        a.same_item_same_components(b)
    }
    /// `AbstractContainerMenu.clicked` as a hand's click: the menu engine
    /// here, a predicted `ContainerClick` on a server (`menu_ops`).
    fn click(&mut self, slot: usize, click: Click) -> bool {
        let (button, input) = match click {
            Click::Left => (0, Input::Pickup),
            Click::Right => (1, Input::Pickup),
            Click::QuickMove => (0, Input::QuickMove),
        };
        let game = &mut *self.game;
        let done = game.menu_click(Some(slot), button, input);
        if done
            && game.remote.is_none()
            && let Some(key) = game.container
        {
            game.container_changed(key);
        }
        done
    }
    /// The game's quick craft drag and its release's click are dropped, as
    /// for a press the game handles itself (a double click's `PICKUP_ALL`),
    /// and so is Horizon's own Mouse Tweaks-style drag (its extra).
    fn take_over(&mut self) {
        let game = &mut *self.game;
        game.inv_drag = None;
        game.inv_clicks.skip_release();
        game.tweak_release();
    }
}

impl Game<'_> {
    /// The open screen is one Mouse Tricks works in: the inventory, or a
    /// chest-like container (chests, barrels, ender chests, shulker boxes,
    /// hoppers, dispensers), where every slot is a plain one. Never a
    /// station (furnace, anvil...), a mount's inventory or the creative one.
    fn mousetricks_screen(&self) -> bool {
        if self.screen != Screen::Inventory || self.station.is_some() || self.mount_active() {
            return false;
        }
        if self.remote.is_some() {
            self.remote_storage_open()
        } else {
            matches!(self.container_kind(), None | Some(BlockEntity::Chest(_)))
        }
    }
    /// The slot under the cursor (never the crafting result).
    fn mousetricks_hovered(&self) -> Option<usize> {
        let (slot, outside) = self.inventory_target();
        slot.filter(|&s| !outside && s != 200)
    }

    /// `tweaks::Game::sort_mouse_press`'s HEAD: a mouse press on any screen,
    /// before the game handles it. On an inventory screen a left or right
    /// press starts the drag it may become (the press itself is the game's);
    /// a middle click on an empty spot, or the sort key bound to a mouse
    /// button, sorts: true, and the game leaves the press alone.
    pub(super) fn mousetricks_press(&mut self, button: MouseButton) -> bool {
        state(|s| s.drag = None);
        if !on(self) || !self.mousetricks_screen() {
            return false;
        }
        let button = match button {
            MouseButton::Left => Button::Left,
            MouseButton::Right => Button::Right,
            other => {
                let name = keybinds::mouse_name(other);
                let key = name == self.mousetricks_sort_key();
                let sorted = (key || other == MouseButton::Middle) && self.mousetricks_sort(!key);
                if sorted && self.settings.keys.get("key.pickItem") == name {
                    // The release of a pick block press the game never saw
                    // clicks nothing either.
                    self.inv_clicks.skip_release();
                }
                return sorted;
            }
        };
        let Some(config) = config(self) else {
            return false;
        };
        let press = Press {
            button,
            slot: self.mousetricks_hovered(),
            shift: self.has_shift_down(),
            holding: self.carried.is_some(),
        };
        let drag = Drag::start(&press, &config);
        state(|s| s.drag = drag);
        false
    }

    /// `Game::inventory_drag_move`'s HEAD: true when the move belongs to
    /// Mouse Tricks' drag (the game's own drag doesn't grow).
    pub(super) fn mousetricks_drag_move(&mut self) -> bool {
        let Some(mut drag) = state(|s| s.drag.take()) else {
            return false;
        };
        if !on(self) || !self.mousetricks_screen() {
            return false;
        }
        let kind = drag.button().drag_type();
        let now = Moment {
            hovered: self.mousetricks_hovered(),
            shift: self.has_shift_down(),
            vanilla: self
                .inv_drag
                .as_ref()
                .filter(|(ty, _)| *ty == kind)
                .map(|(_, slots)| slots.clone()),
        };
        let owned = drag.moved(&mut Native { game: self }, &now);
        state(|s| s.drag = Some(drag));
        owned
    }

    /// `Game::inventory_mouse_release`'s HEAD: the drag ends. A drag the
    /// mod took over already dropped the game's, so the release clicks
    /// nothing more.
    pub(super) fn mousetricks_release(&mut self) {
        state(|s| s.drag = None);
    }

    /// `tweaks::Game::tweak_scroll`'s HEAD: true when the wheel moved items
    /// (or would have, over a stack it works on); false leaves the wheel to
    /// the game. A bundle under the cursor takes the wheel before this.
    pub(super) fn mousetricks_scroll(&mut self, delta: f32, slot: Option<usize>) -> bool {
        let usable = on(self) && self.mousetricks_screen() && self.carried.is_none();
        let Some(config) = config(self).filter(|c| usable && c.wheel) else {
            state(|s| s.wheel.reset());
            return false;
        };
        let layout = Layout {
            container: self
                .slots()
                .into_iter()
                .filter(|s| (300..500).contains(s))
                .collect(),
        };
        let Some(slot) = slot.filter(|&s| !layout.other_side(s).is_empty()) else {
            state(|s| s.wheel.reset());
            return false;
        };
        let steps = state(|s| s.wheel.steps(delta));
        mt::scroll(&mut Native { game: self }, &layout, &config, slot, steps);
        true
    }

    /// `native_frame` (a hook point): a drag or wheel left over from a
    /// screen that closed (its release went to another screen) ends; a
    /// paced sort makes the clicks the time since the last frame allows, or
    /// stops when its screen is gone.
    pub(super) fn mousetricks_frame(&mut self, elapsed: Duration) {
        if self.screen != Screen::Inventory {
            state(|s| {
                s.drag = None;
                s.wheel.reset();
                s.sorting = None;
            });
            return;
        }
        let Some(mut sorting) = state(|s| s.sorting.take()) else {
            return;
        };
        if on(self)
            && self.mousetricks_screen()
            && sorting.run(&mut Native { game: self }, elapsed.as_secs_f32())
        {
            state(|s| s.sorting = Some(sorting));
        }
    }

    /// `native_key` (a hook point): the sort key, over an inventory screen.
    pub(super) fn mousetricks_key(&mut self, key: &str, pressed: bool) {
        if pressed && self.screen == Screen::Inventory && key == self.mousetricks_sort_key() {
            self.mousetricks_sort(false);
        }
    }
    /// The key the player chose for sorting (Options > Mods > Mouse Tricks).
    fn mousetricks_sort_key(&self) -> String {
        self.native_mod_key(mt::ID, mt::SORT_KEY, mt::SORT_KEY_DEFAULT)
    }

    /// The open container's slots when it is plain storage a sort works in
    /// (chests, barrels, ender chests, shulker boxes; none for the inventory
    /// screen). None on the screens a sort leaves alone: hoppers,
    /// dispensers, crafting tables, and every screen Mouse Tricks does
    /// (furnaces, anvils, a mount's inventory, the creative screen...).
    fn mousetricks_storage(&self) -> Option<Vec<usize>> {
        if !self.mousetricks_screen() {
            return None;
        }
        let storage = if self.remote.is_some() {
            self.remote_storage_open()
        } else {
            match self.container_kind() {
                None => true,
                Some(BlockEntity::Chest(_)) => {
                    self.chest_rows().is_some() || self.container_block_is_shulker()
                }
                Some(_) => false,
            }
        };
        storage.then(|| {
            self.slots()
                .into_iter()
                .filter(|s| (300..500).contains(s))
                .collect()
        })
    }
    /// The cursor is nearer the container's slots than the inventory's.
    fn mousetricks_near(&self, container: &[usize]) -> bool {
        let Some(r) = self.renderer.as_ref() else {
            return false;
        };
        let batch = HudBatch::new([r.config.width, r.config.height]);
        let (ox, oy) = self.inventory_origin(&batch);
        let (cx, cy) = self.cursor;
        let nearest = |slots: &mut dyn Iterator<Item = usize>| {
            slots
                .map(|s| {
                    let (x, y) = self.slot_position(s);
                    let (dx, dy) = (ox + x + 8.0 - cx, oy + y + 8.0 - cy);
                    dx * dx + dy * dy
                })
                .fold(f32::INFINITY, f32::min)
        };
        nearest(&mut container.iter().copied()) < nearest(&mut (0..36))
    }
    /// What a sort knows of `stack`'s kind: its item's place in the creative
    /// tabs (the item registry's after them), the name it shows, its damage,
    /// whether it is a bundle (which stays where it is).
    fn mousetricks_kind(&self, stack: &ItemStack) -> Kind {
        let items = self.res.items();
        let bare = ItemStack {
            item: stack.item.clone(),
            count: 1,
            damage: 0,
            extra: None,
        };
        let order = match self.creative_screen.order_of(&bare) {
            Some(place) => place as u64,
            None => items
                .registry_order
                .iter()
                .position(|i| *i == stack.item)
                .map_or(u64::MAX, |i| (1 << 32) + i as u64),
        };
        let names = |id: &str| self.pack.names.display_item(id);
        Kind {
            max: self.max_stack(stack).max(1),
            order,
            name: hover_name(&names, stack),
            damage: stack.damage,
            pinned: Contents::of(items, stack).is_some(),
        }
    }
    /// The sort key, or a middle click (`middle`: only over an empty slot
    /// or no slot): sorts the part of the open screen under the cursor with
    /// the game's own left clicks. Nothing with a stack in hand, while the
    /// recipe book's search box has the keys, or on a screen a sort leaves
    /// alone. True when the input was the sort's.
    fn mousetricks_sort(&mut self, middle: bool) -> bool {
        if !on(self) || self.recipe_search_focused() {
            return false;
        }
        let Some(config) = config(self).filter(|c| c.sort) else {
            return false;
        };
        let Some(container) = self.mousetricks_storage() else {
            return false;
        };
        let (hovered, outside) = self.inventory_target();
        if outside
            || self.carried.is_some()
            || (middle && hovered.is_some_and(|s| self.slot_view(s).is_some()))
        {
            return false;
        }
        if state(|s| s.sorting.is_some()) {
            // One sort at a time: the one under way goes on.
            return true;
        }
        let near = hovered.is_none() && !container.is_empty() && self.mousetricks_near(&container);
        let slots = mt::sort_section(hovered, &container, near, config.sort_hotbar);
        let snapshot = mt::snapshot(&mut Native { game: self }, &slots);
        let kinds: Vec<Kind> = snapshot
            .stacks
            .iter()
            .map(|s| self.mousetricks_kind(s))
            .collect();
        let steps = mt::plan_sort(&snapshot.pieces, &kinds, config.sort_order);
        // On a server the clicks go out at a steady pace; in a world of the
        // game's own, all at once.
        let paced = self.remote.is_some();
        let mut sorting = Sorting::new(slots, snapshot.stacks, steps, paced);
        if sorting.run(&mut Native { game: self }, 0.0) {
            state(|s| s.sorting = Some(sorting));
        }
        true
    }

    /// `native_settings` (a hook point): the settings page's buttons.
    pub(super) fn mousetricks_settings(&self) -> Vec<String> {
        mt::labels(&loaded(self))
    }

    /// `native_settings_click` (a hook point): change the setting, write
    /// `config.json` and put it in effect at once (or read the file again).
    pub(super) fn mousetricks_settings_click(&mut self, button: usize) {
        match mt::click(&loaded(self), button) {
            Some(Effect::Save(config)) => {
                self.native_set_setting(mt::ID, mt::CONFIG_FILE, &config.to_json());
                apply(Ok(config));
            }
            Some(Effect::Reload) => apply(read_config(self)),
            None => {}
        }
    }
}
