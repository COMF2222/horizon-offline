// Copyright (c) 2026 d3grando
// SPDX-License-Identifier: MIT
// Mouse Tricks: inventory mouse drags, wheel moves and sorting.
//
// Original implementation for Horizon (a horizon-mixins native mod). This file
// is both the standalone, unit-tested Cargo library and the injected
// `crate::game::mousetricks` module. It has no Horizon dependency: the open
// screen is reached only through the small `Menu` trait, which `native.rs`
// implements with the game's own slot clicks (the same clicks a player's
// hand makes, so a server sees nothing but ordinary clicks).

use serde::{Deserialize, Serialize};

/// The mod's id: its row in Options > Mods and its settings folder
/// (`horizon-mods/.horizon-mod-data/mousetricks/`).
pub const ID: &str = "mousetricks";
pub const VERSION: &str = "0.2.0";
pub const DESCRIPTION: &str = "Right-drag places one item per slot, left-drag gathers or moves stacks, the wheel moves items one by one, a key or a middle click sorts.";
/// The settings, in the mod's settings folder: what the settings page writes.
pub const CONFIG_FILE: &str = "config.json";
/// The mod's keys (Options > Mods > Mouse Tricks lists them above the
/// settings): name, label, default key.
pub const KEYS: [[&str; 3]; 1] = [[SORT_KEY, "Sort the inventory", SORT_KEY_DEFAULT]];
/// The sort key's name among the mod's keys.
pub const SORT_KEY: &str = "sort";
pub const SORT_KEY_DEFAULT: &str = "key.keyboard.r";

/// Which way the wheel sends the hovered stack's items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WheelDirection {
    /// Down sends one item to the other side, up brings one back.
    Normal,
    /// Up sends, down brings back.
    Inverted,
    /// The item goes the way the wheel turns: up when the other side is
    /// above the hovered slot (the container above the inventory, the main
    /// inventory above the hotbar), down when it is below.
    Positional,
}

/// Where the wheel looks first for an item to bring back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WheelSearch {
    FirstToLast,
    LastToFirst,
}

/// The order a sort puts stacks in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortOrder {
    /// The creative tabs' order of the items (the item registry's after
    /// them), then the name.
    Registry,
    /// The name, then the creative tabs' order.
    Name,
    /// The kinds the section holds most of first.
    Count,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Config {
    /// Right-drag holding a stack: one item into each slot entered, at once,
    /// and one more each time a slot is entered again.
    pub right_drag: bool,
    /// Left-drag with a stack in hand (or picked up by the press): slots of
    /// the same item are gathered into the hand; with Shift, quick-moved.
    pub left_drag: bool,
    /// Shift + left-drag with an empty hand: every slot entered is quick-moved.
    pub shift_drag: bool,
    /// The wheel over a stack moves its items one at a time.
    pub wheel: bool,
    pub wheel_direction: WheelDirection,
    pub wheel_search: WheelSearch,
    /// The sort key, or a middle click on an empty spot, sorts the part of
    /// the screen under the cursor.
    pub sort: bool,
    pub sort_order: SortOrder,
    /// Sorting the player's inventory takes the hotbar too.
    pub sort_hotbar: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            right_drag: true,
            left_drag: true,
            shift_drag: true,
            wheel: true,
            wheel_direction: WheelDirection::Normal,
            wheel_search: WheelSearch::LastToFirst,
            sort: true,
            sort_order: SortOrder::Registry,
            sort_hotbar: false,
        }
    }
}
impl Config {
    pub fn from_json(source: &str) -> Result<Self, String> {
        serde_json::from_str(source).map_err(|e| e.to_string())
    }
    /// The file the settings page writes: every key, readable by `from_json`.
    pub fn to_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self).unwrap_or_default();
        json.push('\n');
        json
    }
}

/// The configuration as read from [`CONFIG_FILE`]: Err when the file is
/// broken (Mouse Tricks then does nothing until it is fixed or reset).
pub type Loaded = Result<Config, String>;

/// One ordinary slot click, as the player's mouse makes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    /// `PICKUP` with the left button: take the stack, put the hand's down,
    /// merge it, or swap.
    Left,
    /// `PICKUP` with the right button: one item down, or half the stack up.
    Right,
    /// `QUICK_MOVE` (a Shift click): the stack to the other side.
    QuickMove,
}

/// The open screen's slots as Mouse Tricks sees and clicks them.
pub trait Menu {
    type Stack: Clone;
    /// What `slot` holds.
    fn slot(&mut self, slot: usize) -> Option<Self::Stack>;
    /// The stack on the cursor.
    fn carried(&self) -> Option<Self::Stack>;
    fn count(&self, stack: &Self::Stack) -> u32;
    /// The most of this item one slot holds.
    fn max(&self, stack: &Self::Stack) -> u32;
    /// Same item and components (counts aside): they stack.
    fn same(&self, a: &Self::Stack, b: &Self::Stack) -> bool;
    /// One click, through the game's container click path. False when the
    /// screen took none.
    fn click(&mut self, slot: usize, click: Click) -> bool;
    /// The drag becomes the mod's: the game's own drag (and its release's
    /// click) is dropped.
    fn take_over(&mut self);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
}
impl Button {
    /// The quick craft type of the game's own drag with this button.
    pub fn drag_type(self) -> i32 {
        match self {
            Self::Left => 0,
            Self::Right => 1,
        }
    }
}

/// A mouse press over an inventory screen, before the game handles it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Press {
    pub button: Button,
    /// The slot under the cursor.
    pub slot: Option<usize>,
    pub shift: bool,
    /// A stack was on the cursor.
    pub holding: bool,
}

/// The cursor moved with the button still down.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Moment {
    /// The slot under the cursor.
    pub hovered: Option<usize>,
    pub shift: bool,
    /// The game's own drag of this button (`quickCraftSlots`), if it runs.
    pub vanilla: Option<Vec<usize>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// Right-drag holding a stack.
    Place,
    /// Left-drag over slots of the hand's item.
    Matching,
    /// Shift + left-drag with an empty hand.
    MoveAll,
}

/// A drag Mouse Tricks follows, from press to release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drag {
    button: Button,
    mode: Mode,
    press: Option<usize>,
    /// The drag is the mod's (the game's own was dropped).
    owned: bool,
    /// The game's own drag spread first: this one is left to it.
    given_up: bool,
    /// The slot under the cursor at the last move.
    last: Option<usize>,
    /// Slots already gathered or moved (a placing drag places again instead).
    visited: Vec<usize>,
}

impl Drag {
    /// The drag a press may start, with the features `config` turns on.
    pub fn start(press: &Press, config: &Config) -> Option<Self> {
        let mode = match press.button {
            Button::Right if config.right_drag && press.holding && !press.shift => Mode::Place,
            Button::Right => return None,
            Button::Left if press.shift && !press.holding => {
                if !config.shift_drag {
                    return None;
                }
                Mode::MoveAll
            }
            Button::Left if config.left_drag => Mode::Matching,
            Button::Left => return None,
        };
        Some(Self {
            button: press.button,
            mode,
            press: press.slot,
            owned: false,
            given_up: false,
            last: press.slot,
            visited: Vec::new(),
        })
    }

    pub fn button(&self) -> Button {
        self.button
    }

    /// The cursor moved: true when the move is the mod's (the game's own
    /// drag must not grow).
    pub fn moved<M: Menu>(&mut self, menu: &mut M, now: &Moment) -> bool {
        if self.given_up {
            return false;
        }
        let entered = now.hovered.filter(|_| now.hovered != self.last);
        self.last = now.hovered;
        if !self.owned {
            return self.claim(menu, now, entered);
        }
        if let Some(slot) = entered {
            self.enter(menu, slot, now.shift);
        }
        true
    }

    /// Takes the drag over when it is one of the mod's.
    fn claim<M: Menu>(&mut self, menu: &mut M, now: &Moment, entered: Option<usize>) -> bool {
        match self.mode {
            Mode::Place => {
                // Only once the cursor leaves the pressed slot: a right click
                // that stays in it is the game's (it may be a swap).
                let Some(vanilla) = &now.vanilla else {
                    // The game started no drag (or something else took it).
                    self.given_up = true;
                    return false;
                };
                if now.hovered == self.press {
                    return false;
                }
                let first = vanilla.clone();
                self.own(menu);
                for slot in first {
                    place(menu, slot);
                }
                if let Some(slot) = entered {
                    place(menu, slot);
                }
                true
            }
            Mode::Matching => {
                if now.vanilla.as_ref().is_some_and(|v| v.len() > 1) {
                    // The game's drag spread over a second slot first.
                    self.given_up = true;
                    return false;
                }
                let Some(slot) = entered.filter(|&s| Some(s) != self.press) else {
                    return false;
                };
                let Some(held) = menu.carried() else {
                    return false;
                };
                if !menu.slot(slot).is_some_and(|s| menu.same(&s, &held)) {
                    return false;
                }
                self.own(menu);
                if let Some(press) = self.press {
                    self.enter(menu, press, now.shift);
                }
                self.enter(menu, slot, now.shift);
                true
            }
            Mode::MoveAll => {
                let Some(slot) = entered.filter(|&s| Some(s) != self.press) else {
                    return false;
                };
                self.own(menu);
                // The press quick-moved its own slot already.
                self.visited.extend(self.press);
                self.enter(menu, slot, now.shift);
                true
            }
        }
    }

    fn own<M: Menu>(&mut self, menu: &mut M) {
        menu.take_over();
        self.owned = true;
    }

    /// The cursor entered `slot` during the mod's drag.
    fn enter<M: Menu>(&mut self, menu: &mut M, slot: usize, shift: bool) {
        match self.mode {
            Mode::Place => place(menu, slot),
            Mode::Matching => {
                if self.visited.contains(&slot) {
                    return;
                }
                self.visited.push(slot);
                matching(menu, slot, shift);
            }
            Mode::MoveAll => {
                // Without Shift the drag pauses: the slot stays for later.
                if !shift || self.visited.contains(&slot) {
                    return;
                }
                self.visited.push(slot);
                if menu.slot(slot).is_some() {
                    menu.click(slot, Click::QuickMove);
                }
            }
        }
    }
}

/// One item of the hand into `slot`, when it takes it (empty, or the same
/// item with room): never the swap a right click on another item makes.
fn place<M: Menu>(menu: &mut M, slot: usize) {
    let Some(held) = menu.carried() else {
        return;
    };
    let fits = match menu.slot(slot) {
        None => true,
        Some(s) => menu.same(&s, &held) && menu.count(&s) < menu.max(&s),
    };
    if fits {
        menu.click(slot, Click::Right);
    }
}

/// `slot` holds the hand's item: Shift quick-moves it, else it joins the
/// hand when all of it fits.
fn matching<M: Menu>(menu: &mut M, slot: usize, shift: bool) {
    let Some(held) = menu.carried() else {
        return;
    };
    let Some(stack) = menu.slot(slot).filter(|s| menu.same(s, &held)) else {
        return;
    };
    if shift {
        menu.click(slot, Click::QuickMove);
    } else if menu.count(&held) + menu.count(&stack) <= menu.max(&held)
        && menu.click(slot, Click::Left)
    {
        // The hand's stack goes into the slot, then the slot's comes up whole.
        menu.click(slot, Click::Left);
    }
}

/// The open screen's layout for the wheel: the container's slots (300 and
/// up, in order), none for the inventory screen. Slots 0..9 are the hotbar,
/// 9..36 the main inventory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Layout {
    pub container: Vec<usize>,
}

const HOTBAR: std::ops::Range<usize> = 0..9;
const MAIN: std::ops::Range<usize> = 9..36;

impl Layout {
    /// The slots across from `slot` in quick-move order (where a Shift click
    /// would put its stack): a container's go to the inventory, hotbar first
    /// from its right end; the inventory's go to the container; without
    /// one, the hotbar and the main inventory trade. Empty for other slots.
    pub fn other_side(&self, slot: usize) -> Vec<usize> {
        let player = || HOTBAR.rev().chain(MAIN.rev());
        if self.container.contains(&slot) {
            player().collect()
        } else if !MAIN.contains(&slot) && !HOTBAR.contains(&slot) {
            Vec::new()
        } else if !self.container.is_empty() {
            self.container.clone()
        } else if HOTBAR.contains(&slot) {
            MAIN.collect()
        } else {
            HOTBAR.collect()
        }
    }
    /// The other side's slots in screen order (top left to bottom right).
    fn screen_order(&self, slot: usize) -> Vec<usize> {
        let mut side = self.other_side(slot);
        // The main inventory is drawn above the hotbar.
        side.sort_by_key(|&s| (HOTBAR.contains(&s), s));
        side
    }
    /// `slot` is above the other side on screen.
    pub fn upper(&self, slot: usize) -> bool {
        if self.container.is_empty() {
            MAIN.contains(&slot)
        } else {
            self.container.contains(&slot)
        }
    }
}

/// Wheel steps not yet acted on (a trackpad scrolls in fractions).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wheel {
    rest: f32,
}
impl Wheel {
    /// Whole steps in `delta` and what is left over; positive is up.
    pub fn steps(&mut self, delta: f32) -> i32 {
        if !delta.is_finite() {
            return 0;
        }
        self.rest += delta;
        let steps = self.rest.trunc();
        self.rest -= steps;
        steps.clamp(-64.0, 64.0) as i32
    }
    pub fn reset(&mut self) {
        self.rest = 0.0;
    }
}

/// `steps` wheel steps over `slot` (positive is up), with an empty hand:
/// each sends one of its items to the other side, or brings one of the same
/// item back, as `config` says. Returns how many items moved.
pub fn scroll<M: Menu>(
    menu: &mut M,
    layout: &Layout,
    config: &Config,
    slot: usize,
    steps: i32,
) -> u32 {
    if steps == 0 || menu.carried().is_some() {
        return 0;
    }
    let up = steps > 0;
    let send = match config.wheel_direction {
        WheelDirection::Normal => !up,
        WheelDirection::Inverted => up,
        WheelDirection::Positional => up != layout.upper(slot),
    };
    let mut moved = 0;
    for _ in 0..steps.unsigned_abs() {
        let done = if send {
            send_one(menu, layout, slot)
        } else {
            bring_one(menu, layout, config.wheel_search, slot)
        };
        if !done {
            break;
        }
        moved += 1;
    }
    moved
}

/// One item of `slot` to the other side: onto a stack of it with room, else
/// into the first empty slot (as a Shift click fills them).
fn send_one<M: Menu>(menu: &mut M, layout: &Layout, slot: usize) -> bool {
    let Some(stack) = menu.slot(slot) else {
        return false;
    };
    let side: Vec<(usize, Option<M::Stack>)> = layout
        .other_side(slot)
        .into_iter()
        .map(|s| (s, menu.slot(s)))
        .collect();
    let target = side
        .iter()
        .find(|(_, s)| {
            s.as_ref()
                .is_some_and(|s| menu.same(s, &stack) && menu.count(s) < menu.max(s))
        })
        .or_else(|| side.iter().find(|(_, s)| s.is_none()))
        .map(|(s, _)| *s);
    let Some(target) = target else {
        return false;
    };
    let before = menu.slot(target).map_or(0, |s| menu.count(&s));
    carry_one(menu, slot, target);
    menu.slot(target).map_or(0, |s| menu.count(&s)) > before
}

/// One item of `slot`'s kind from the other side into `slot`.
fn bring_one<M: Menu>(menu: &mut M, layout: &Layout, search: WheelSearch, slot: usize) -> bool {
    let Some(stack) = menu.slot(slot) else {
        return false;
    };
    if menu.count(&stack) >= menu.max(&stack) {
        return false;
    }
    let mut side = layout.screen_order(slot);
    if search == WheelSearch::LastToFirst {
        side.reverse();
    }
    let source = side
        .into_iter()
        .find(|&s| menu.slot(s).is_some_and(|s| menu.same(&s, &stack)));
    let Some(source) = source else {
        return false;
    };
    let before = menu.count(&stack);
    carry_one(menu, source, slot);
    menu.slot(slot).map_or(0, |s| menu.count(&s)) > before
}

/// Takes `from`'s stack up, puts one item down on `to`, and the rest back.
fn carry_one<M: Menu>(menu: &mut M, from: usize, to: usize) {
    if !menu.click(from, Click::Left) {
        return;
    }
    if menu.carried().is_some() {
        menu.click(to, Click::Right);
    }
    if menu.carried().is_some() {
        menu.click(from, Click::Left);
    }
}

/// The slots a sort under the cursor rearranges, in screen order (sorted
/// stacks fill them from the first): the container's when the cursor is
/// over one of its slots, or over no slot but nearer the container than
/// the inventory; else the main inventory, followed by the hotbar when
/// `hotbar` (the hotbar is drawn below it). Never the armour, the offhand
/// or the crafting grid. `container` is empty when the open screen has no
/// container a sort works in.
pub fn sort_section(
    hovered: Option<usize>,
    container: &[usize],
    near_container: bool,
    hotbar: bool,
) -> Vec<usize> {
    let over_container = match hovered {
        Some(slot) => container.contains(&slot),
        None => near_container,
    };
    if over_container && !container.is_empty() {
        return container.to_vec();
    }
    let mut slots: Vec<usize> = MAIN.collect();
    if hotbar {
        slots.extend(HOTBAR);
    }
    slots
}

/// What a sort knows of one kind of stack: one item with the same
/// components (stacks of a kind stack together; anything else that
/// differs, a name, an enchantment, damage, makes another kind).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kind {
    /// The most of it one slot holds.
    pub max: u32,
    /// Its item's place in the creative tabs (the item registry's after
    /// them).
    pub order: u64,
    /// The name it shows.
    pub name: String,
    pub damage: u32,
    /// It stays in its slot: a bundle (a click with a stack on a bundle, or
    /// with a bundle on a stack, puts the stack into the bundle instead of
    /// swapping them).
    pub pinned: bool,
}

/// A stack as the sort sees it: its kind (an index into the kinds) and
/// its count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub kind: usize,
    pub count: u32,
}

/// One part of a sort: left clicks (indices into the section's slots) that
/// start and end with an empty hand, and the section as they expect it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub before: Vec<Option<Piece>>,
    pub clicks: Vec<usize>,
}

/// A left click (`PICKUP` with the left button) on plain slot `i`, as the
/// game's menus make it: the hand takes the slot's stack, puts its own down,
/// tops up a stack of its kind (keeping what doesn't fit), or swaps.
pub fn left_click(slots: &mut [Option<Piece>], hand: &mut Option<Piece>, kinds: &[Kind], i: usize) {
    let Some(slot) = slots.get_mut(i) else {
        return;
    };
    match (slot.as_mut(), hand.as_mut()) {
        (None, None) => {}
        (Some(here), Some(held)) if here.kind == held.kind => {
            let max = kinds.get(here.kind).map_or(0, |k| k.max);
            let moved = held.count.min(max.saturating_sub(here.count));
            here.count += moved;
            held.count -= moved;
            if held.count == 0 {
                *hand = None;
            }
        }
        // Take, put down, or swap.
        _ => std::mem::swap(slot, hand),
    }
}

/// The section once sorted: in the slots that aren't pinned, every kind's
/// stacks merged (full stacks and at most one more), in `order`, then the
/// empty slots. Kinds that tie stay apart, in the order they first appear;
/// a kind's full stacks come before its last one.
pub fn sorted(slots: &[Option<Piece>], kinds: &[Kind], order: SortOrder) -> Vec<Option<Piece>> {
    let pinned = |p: &Option<Piece>| p.is_some_and(|p| kinds[p.kind].pinned);
    let mut totals = vec![0u64; kinds.len()];
    let mut first = vec![usize::MAX; kinds.len()];
    for (i, p) in slots.iter().enumerate() {
        if let Some(p) = p.filter(|_| !pinned(p)) {
            totals[p.kind] += u64::from(p.count);
            first[p.kind] = first[p.kind].min(i);
        }
    }
    let names: Vec<String> = kinds.iter().map(|k| k.name.to_lowercase()).collect();
    let mut present: Vec<usize> = (0..kinds.len()).filter(|&k| totals[k] > 0).collect();
    present.sort_by(|&a, &b| {
        let (ka, kb) = (&kinds[a], &kinds[b]);
        let tabs = ka.order.cmp(&kb.order);
        let name = names[a].cmp(&names[b]);
        let most = totals[b].cmp(&totals[a]);
        match order {
            SortOrder::Registry => tabs.then(name).then(most),
            SortOrder::Name => name.then(tabs).then(most),
            SortOrder::Count => most.then(tabs).then(name),
        }
        .then(ka.damage.cmp(&kb.damage))
        .then(first[a].cmp(&first[b]))
    });
    let mut stacks = Vec::new();
    for k in present {
        let max = u64::from(kinds[k].max.max(1));
        let (full, rest) = (totals[k] / max, totals[k] % max);
        for _ in 0..full {
            stacks.push(Piece {
                kind: k,
                count: max as u32,
            });
        }
        if rest > 0 {
            stacks.push(Piece {
                kind: k,
                count: rest as u32,
            });
        }
    }
    let mut stacks = stacks.into_iter();
    slots
        .iter()
        .map(|p| if pinned(p) { *p } else { stacks.next() })
        .collect()
}

/// The clicks that sort `slots` (into [`sorted`]'s arrangement), in steps
/// that each start and end with an empty hand. Only left clicks on the
/// section's own slots, so nothing leaves it: a kind's partial stacks are
/// merged first, then every stack goes to its place along chains and cycles
/// of swaps (each costs one click more than the stacks it moves; a stack
/// already in a slot of its kind stays), and a kind's partial stack trades
/// counts with a full one when it isn't last. Never a click with a stack
/// on a stack of its kind but to merge them, never one on a pinned stack.
/// Empty when the section is sorted already, or holds a stack the sort
/// can't account for (more than its kind's maximum).
pub fn plan_sort(slots: &[Option<Piece>], kinds: &[Kind], order: SortOrder) -> Vec<Step> {
    let valid = slots.iter().flatten().all(|p| {
        kinds
            .get(p.kind)
            .is_some_and(|k| k.max > 0 && p.count > 0 && p.count <= k.max)
    });
    if !valid {
        return Vec::new();
    }
    let target = sorted(slots, kinds, order);
    let mut plan = Plan {
        kinds,
        cur: slots.to_vec(),
        steps: Vec::new(),
        broken: false,
    };
    plan.merge();
    plan.arrange(&target);
    plan.settle(&target);
    if plan.broken || plan.cur != target {
        return Vec::new();
    }
    plan.steps
}

/// A sort being planned: the section as its steps so far leave it.
struct Plan<'k> {
    kinds: &'k [Kind],
    cur: Vec<Option<Piece>>,
    steps: Vec<Step>,
    /// A step that wouldn't end with an empty hand (never: the plan would
    /// be dropped rather than run).
    broken: bool,
}

impl Plan<'_> {
    fn step(&mut self, clicks: Vec<usize>) {
        let before = self.cur.clone();
        let mut hand = None;
        for &i in &clicks {
            left_click(&mut self.cur, &mut hand, self.kinds, i);
        }
        self.broken |= hand.is_some();
        self.steps.push(Step { before, clicks });
    }
    fn count(&self, i: usize) -> u32 {
        self.cur[i].map_or(0, |p| p.count)
    }

    /// Each kind's partial stacks, the last into the first, until at most
    /// one is left.
    fn merge(&mut self) {
        for (kind, k) in self.kinds.iter().enumerate() {
            if k.pinned {
                continue;
            }
            let partial: Vec<usize> = (0..self.cur.len())
                .filter(|&i| self.cur[i].is_some_and(|p| p.kind == kind && p.count < k.max))
                .collect();
            let (mut first, mut last) = (0, partial.len().saturating_sub(1));
            while first < last {
                let (to, from) = (partial[first], partial[last]);
                if self.count(from) > k.max - self.count(to) {
                    // What doesn't fit goes back.
                    self.step(vec![from, to, from]);
                } else {
                    self.step(vec![from, to]);
                }
                if self.count(to) >= k.max {
                    first += 1;
                }
                if self.cur[from].is_none() {
                    last -= 1;
                }
            }
        }
    }

    /// Every stack to a slot its kind has in `target`. A slot that holds
    /// its target's kind already stays; the others form chains (from a slot
    /// that ends up empty to one that is empty now) and cycles, each walked
    /// with a stack in hand: the hand puts it down and takes up the one that
    /// was there, which goes on to its own place.
    fn arrange(&mut self, target: &[Option<Piece>]) {
        let n = self.cur.len();
        let kind = |p: &Option<Piece>| p.map(|p| p.kind);
        let stays: Vec<bool> = (0..n)
            .map(|i| kind(&self.cur[i]) == kind(&target[i]))
            .collect();
        let mut wanted: Vec<Vec<usize>> = vec![Vec::new(); self.kinds.len()];
        for i in (0..n).rev() {
            if let Some(p) = target[i].filter(|_| !stays[i]) {
                wanted[p.kind].push(i);
            }
        }
        let mut next: Vec<Option<usize>> = vec![None; n];
        for i in 0..n {
            if let Some(p) = self.cur[i].filter(|_| !stays[i]) {
                next[i] = wanted[p.kind].pop();
                self.broken |= next[i].is_none();
            }
        }
        let mut walked = vec![false; n];
        // Chains first (they start where the target is empty), then cycles.
        let starts: Vec<usize> = (0..n)
            .filter(|&i| next[i].is_some() && target[i].is_none())
            .chain((0..n).filter(|&i| next[i].is_some()))
            .collect();
        for start in starts {
            if walked[start] || self.broken {
                continue;
            }
            walked[start] = true;
            let mut clicks = vec![start];
            let mut at = start;
            while let Some(to) = next[at] {
                clicks.push(to);
                if to == start || self.cur[to].is_none() || clicks.len() > n + 1 {
                    break;
                }
                walked[to] = true;
                at = to;
            }
            self.step(clicks);
        }
    }

    /// A kind whose partial stack isn't in its last slot: the full stack
    /// there tops it up and takes the rest (a swap of their counts).
    fn settle(&mut self, target: &[Option<Piece>]) {
        for (kind, k) in self.kinds.iter().enumerate() {
            if k.pinned || self.broken {
                continue;
            }
            let wrong: Vec<usize> = (0..self.cur.len())
                .filter(|&i| target[i].is_some_and(|p| p.kind == kind) && self.cur[i] != target[i])
                .collect();
            match wrong[..] {
                [] => {}
                [x, y] => {
                    let (full, partial) = if self.count(x) == k.max {
                        (x, y)
                    } else {
                        (y, x)
                    };
                    if self.count(full) == k.max && self.count(partial) < k.max {
                        self.step(vec![full, partial, full]);
                    } else {
                        self.broken = true;
                    }
                }
                _ => self.broken = true,
            }
        }
    }
}

/// The section's stacks and kinds as `plan_sort` takes them.
#[derive(Clone, Debug)]
pub struct Snapshot<S> {
    pub pieces: Vec<Option<Piece>>,
    /// A stack of each kind, in the kinds' order.
    pub stacks: Vec<S>,
}

/// What `slots` hold, grouped in kinds by `Menu::same`.
pub fn snapshot<M: Menu>(menu: &mut M, slots: &[usize]) -> Snapshot<M::Stack> {
    let mut stacks: Vec<M::Stack> = Vec::new();
    let mut pieces = Vec::with_capacity(slots.len());
    for &slot in slots {
        let piece = match menu.slot(slot) {
            None => None,
            Some(stack) => {
                let kind = match stacks.iter().position(|k| menu.same(k, &stack)) {
                    Some(kind) => kind,
                    None => {
                        stacks.push(stack.clone());
                        stacks.len() - 1
                    }
                };
                Some(Piece {
                    kind,
                    count: menu.count(&stack),
                })
            }
        };
        pieces.push(piece);
    }
    Snapshot { pieces, stacks }
}

/// Clicks a paced sort makes per game tick (a step always runs whole).
pub const SORT_CLICKS_PER_TICK: f32 = 8.0;
const TICK_SECONDS: f32 = 0.05;

/// A sort under way: its steps, all at once in a world of the game's own,
/// at most [`SORT_CLICKS_PER_TICK`] a tick on a server. Before each step the
/// section must hold what the plan expects and the hand must be empty, else
/// the sort stops there (between steps, so nothing is left in the hand).
#[derive(Clone, Debug)]
pub struct Sorting<S> {
    slots: Vec<usize>,
    stacks: Vec<S>,
    steps: std::collections::VecDeque<Step>,
    /// The clicks the pace allows now; None: unpaced.
    credit: Option<f32>,
}

impl<S: Clone> Sorting<S> {
    /// `slots` are the section's (the steps' clicks index them), `stacks`
    /// a stack of each kind (`Snapshot::stacks`).
    pub fn new(slots: Vec<usize>, stacks: Vec<S>, steps: Vec<Step>, paced: bool) -> Self {
        Self {
            slots,
            stacks,
            steps: steps.into(),
            credit: paced.then_some(SORT_CLICKS_PER_TICK),
        }
    }
    // Standalone callers can inspect this; the game's adapter uses `run`'s return value.
    #[allow(dead_code)]
    pub fn done(&self) -> bool {
        self.steps.is_empty()
    }
    /// Runs the steps the pace allows `elapsed` seconds later. True while
    /// steps are left for later.
    pub fn run<M: Menu<Stack = S>>(&mut self, menu: &mut M, elapsed: f32) -> bool {
        if let Some(credit) = self.credit.as_mut()
            && elapsed.is_finite()
            && elapsed > 0.0
        {
            *credit =
                (*credit + elapsed / TICK_SECONDS * SORT_CLICKS_PER_TICK).min(SORT_CLICKS_PER_TICK);
        }
        while !self.steps.is_empty() {
            if self.credit.is_some_and(|c| c <= 0.0) {
                return true;
            }
            let ready = self.steps.front().is_some_and(|s| self.ready(menu, s));
            let Some(step) = self.steps.pop_front().filter(|_| ready) else {
                self.steps.clear();
                return false;
            };
            for &i in &step.clicks {
                if !menu.click(self.slots[i], Click::Left) {
                    self.steps.clear();
                    return false;
                }
            }
            if let Some(credit) = self.credit.as_mut() {
                *credit -= step.clicks.len() as f32;
            }
        }
        false
    }
    /// The hand is empty and the section holds what `step` expects.
    fn ready<M: Menu<Stack = S>>(&self, menu: &mut M, step: &Step) -> bool {
        if menu.carried().is_some() || step.before.len() != self.slots.len() {
            return false;
        }
        self.slots
            .iter()
            .zip(&step.before)
            .all(|(&slot, want)| match (menu.slot(slot), want) {
                (None, None) => true,
                (Some(have), Some(want)) => self
                    .stacks
                    .get(want.kind)
                    .is_some_and(|kind| menu.same(&have, kind) && menu.count(&have) == want.count),
                _ => false,
            })
    }
}

/// A button of the settings page (Options > Mods > Mouse Tricks > Settings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    /// The file is broken: its error. A click reads it again.
    Broken,
    RightDrag,
    LeftDrag,
    ShiftDrag,
    Wheel,
    WheelDirection,
    WheelSearch,
    Sort,
    SortOrder,
    SortHotbar,
    /// Every setting back to its default.
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

/// Longest error shown on the page.
const ERROR_WIDTH: usize = 40;

fn on_off(on: bool) -> &'static str {
    if on { "ON" } else { "OFF" }
}

/// The page's buttons, in order. The wheel's and the sort's options show
/// while they are on.
pub fn page(loaded: &Loaded) -> Vec<Setting> {
    let Ok(c) = loaded else {
        return vec![Setting::Broken, Setting::Defaults];
    };
    let mut page = vec![
        Setting::RightDrag,
        Setting::LeftDrag,
        Setting::ShiftDrag,
        Setting::Wheel,
    ];
    if c.wheel {
        page.extend([Setting::WheelDirection, Setting::WheelSearch]);
    }
    page.push(Setting::Sort);
    if c.sort {
        page.extend([Setting::SortOrder, Setting::SortHotbar]);
    }
    page.push(Setting::Defaults);
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
            Self::RightDrag => format!("Right-drag: one item per slot: {}", on_off(c.right_drag)),
            Self::LeftDrag => format!("Left-drag: gather same items: {}", on_off(c.left_drag)),
            Self::ShiftDrag => format!("Shift + left-drag: move stacks: {}", on_off(c.shift_drag)),
            Self::Wheel => format!("Wheel: move items one by one: {}", on_off(c.wheel)),
            Self::WheelDirection => match c.wheel_direction {
                WheelDirection::Normal => "Wheel direction: down sends, up brings back",
                WheelDirection::Inverted => "Wheel direction: up sends, down brings back",
                WheelDirection::Positional => "Wheel direction: items follow the wheel",
            }
            .into(),
            Self::WheelSearch => match c.wheel_search {
                WheelSearch::FirstToLast => "Wheel brings back from: first slot",
                WheelSearch::LastToFirst => "Wheel brings back from: last slot",
            }
            .into(),
            Self::Sort => format!("Sort (key or middle click): {}", on_off(c.sort)),
            Self::SortOrder => match c.sort_order {
                SortOrder::Registry => "Sort order: creative tabs",
                SortOrder::Name => "Sort order: name",
                SortOrder::Count => "Sort order: most items first",
            }
            .into(),
            Self::SortHotbar => format!("Sort the hotbar too: {}", on_off(c.sort_hotbar)),
            Self::Defaults => "Reset to defaults".into(),
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
            Self::Broken => return Effect::Reload,
            Self::RightDrag => c.right_drag = !c.right_drag,
            Self::LeftDrag => c.left_drag = !c.left_drag,
            Self::ShiftDrag => c.shift_drag = !c.shift_drag,
            Self::Wheel => c.wheel = !c.wheel,
            Self::WheelDirection => {
                c.wheel_direction = match c.wheel_direction {
                    WheelDirection::Normal => WheelDirection::Inverted,
                    WheelDirection::Inverted => WheelDirection::Positional,
                    WheelDirection::Positional => WheelDirection::Normal,
                }
            }
            Self::WheelSearch => {
                c.wheel_search = match c.wheel_search {
                    WheelSearch::FirstToLast => WheelSearch::LastToFirst,
                    WheelSearch::LastToFirst => WheelSearch::FirstToLast,
                }
            }
            Self::Sort => c.sort = !c.sort,
            Self::SortOrder => {
                c.sort_order = match c.sort_order {
                    SortOrder::Registry => SortOrder::Name,
                    SortOrder::Name => SortOrder::Count,
                    SortOrder::Count => SortOrder::Registry,
                }
            }
            Self::SortHotbar => c.sort_hotbar = !c.sort_hotbar,
            Self::Defaults => c = Config::default(),
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
    use std::collections::BTreeMap;

    type Stack = (&'static str, u32);

    /// A chest-and-inventory menu that clicks as the game's menus do.
    #[derive(Default)]
    struct Fake {
        slots: BTreeMap<usize, Stack>,
        carried: Option<Stack>,
        layout: Layout,
        clicks: Vec<(usize, Click)>,
        taken_over: u32,
        /// Slots that refuse items placed into them.
        refusing: Vec<usize>,
    }
    fn max_of(item: &str) -> u32 {
        match item.split(':').next() {
            Some("pearl" | "egg") => 16,
            Some("sword" | "bundle") => 1,
            _ => 64,
        }
    }
    fn bundle(stack: Option<Stack>) -> bool {
        stack.is_some_and(|s| s.0.starts_with("bundle"))
    }
    impl Fake {
        fn chest() -> Self {
            Self {
                layout: Layout {
                    container: (300..327).collect(),
                },
                ..Self::default()
            }
        }
        fn with(mut self, slot: usize, item: &'static str, count: u32) -> Self {
            self.slots.insert(slot, (item, count));
            self
        }
        fn holding(mut self, item: &'static str, count: u32) -> Self {
            self.carried = Some((item, count));
            self
        }
        fn count(&self, slot: usize) -> u32 {
            self.slots.get(&slot).map_or(0, |s| s.1)
        }
        fn total(&self, item: &str) -> u32 {
            let held = self.carried.filter(|c| c.0 == item).map_or(0, |c| c.1);
            held + self
                .slots
                .values()
                .filter(|s| s.0 == item)
                .map(|s| s.1)
                .sum::<u32>()
        }
        fn put(&mut self, slot: usize, stack: Option<Stack>) {
            match stack {
                Some(s) if s.1 > 0 => self.slots.insert(slot, s),
                _ => self.slots.remove(&slot),
            };
        }
        fn quick_move(&mut self, slot: usize) {
            let Some((item, mut count)) = self.slots.get(&slot).copied() else {
                return;
            };
            let side = self.layout.other_side(slot);
            for &t in &side {
                if let Some(s) = self.slots.get_mut(&t)
                    && s.0 == item
                {
                    let room = max_of(item) - s.1;
                    let n = room.min(count);
                    s.1 += n;
                    count -= n;
                }
            }
            if count > 0
                && let Some(&t) = side
                    .iter()
                    .find(|t| !self.slots.contains_key(t) && !self.refusing.contains(t))
            {
                self.slots.insert(t, (item, count));
                count = 0;
            }
            self.put(slot, Some((item, count)));
        }
    }
    impl Menu for Fake {
        type Stack = Stack;
        fn slot(&mut self, slot: usize) -> Option<Stack> {
            self.slots.get(&slot).copied()
        }
        fn carried(&self) -> Option<Stack> {
            self.carried
        }
        fn count(&self, s: &Stack) -> u32 {
            s.1
        }
        fn max(&self, s: &Stack) -> u32 {
            max_of(s.0)
        }
        fn same(&self, a: &Stack, b: &Stack) -> bool {
            a.0 == b.0
        }
        fn click(&mut self, slot: usize, click: Click) -> bool {
            self.clicks.push((slot, click));
            let here = self.slots.get(&slot).copied();
            // A bundle's own click: the stack would go into the bundle.
            assert!(
                click == Click::QuickMove
                    || here.is_none()
                    || self.carried.is_none()
                    || !(bundle(here) || bundle(self.carried)),
                "a click of a stack and a bundle on {slot}: {here:?} {:?}",
                self.carried
            );
            let refuses = self.refusing.contains(&slot);
            match (click, here, self.carried) {
                (Click::QuickMove, _, _) => self.quick_move(slot),
                (Click::Left, Some(s), None) => {
                    self.carried = Some(s);
                    self.put(slot, None);
                }
                (Click::Right, Some((item, n)), None) => {
                    let up = n.div_ceil(2);
                    self.carried = Some((item, up));
                    self.put(slot, Some((item, n - up)));
                }
                (_, _, Some(_)) if refuses => {}
                (Click::Left, None, Some(c)) => {
                    self.put(slot, Some(c));
                    self.carried = None;
                }
                (Click::Right, None, Some((item, n))) => {
                    self.put(slot, Some((item, 1)));
                    self.carried = (n > 1).then_some((item, n - 1));
                }
                (click, Some((item, n)), Some((held, h))) if item == held => {
                    let moved = match click {
                        Click::Right => 1,
                        _ => h,
                    }
                    .min(max_of(item) - n);
                    self.put(slot, Some((item, n + moved)));
                    self.carried = (h > moved).then_some((item, h - moved));
                }
                (_, Some(s), Some(c)) => {
                    self.put(slot, Some(c));
                    self.carried = Some(s);
                }
                (_, None, None) => return true,
            }
            true
        }
        fn take_over(&mut self) {
            self.taken_over += 1;
        }
    }

    fn press(button: Button, slot: usize, shift: bool, holding: bool) -> Press {
        Press {
            button,
            slot: Some(slot),
            shift,
            holding,
        }
    }
    fn at(slot: Option<usize>, shift: bool, vanilla: Option<Vec<usize>>) -> Moment {
        Moment {
            hovered: slot,
            shift,
            vanilla,
        }
    }

    #[test]
    fn config_defaults_and_file() {
        let c = Config::default();
        assert!(c.right_drag && c.left_drag && c.shift_drag && c.wheel);
        assert_eq!(Config::from_json("{}"), Ok(c.clone()));
        assert_eq!(Config::from_json(&c.to_json()), Ok(c.clone()));
        let json = c.to_json();
        for key in [
            "rightDrag",
            "leftDrag",
            "shiftDrag",
            "wheel",
            "wheelDirection",
            "wheelSearch",
            "sort",
            "sortOrder",
            "sortHotbar",
        ] {
            assert!(json.contains(&format!("\"{key}\"")), "{key} in {json}");
        }
        let positional = Config::from_json(r#"{"wheelDirection":"positional","wheel":false}"#);
        assert_eq!(
            positional,
            Ok(Config {
                wheel: false,
                wheel_direction: WheelDirection::Positional,
                ..Config::default()
            })
        );
        assert!(Config::from_json(r#"{"rightdrag":true}"#).is_err());
        assert!(Config::from_json(r#"{"wheelSearch":"middle"}"#).is_err());
        let sort = Config::from_json(r#"{"sortOrder":"count","sortHotbar":true}"#);
        assert_eq!(
            sort,
            Ok(Config {
                sort_order: SortOrder::Count,
                sort_hotbar: true,
                ..Config::default()
            })
        );
        assert!(Config::from_json(r#"{"sortOrder":"colour"}"#).is_err());
        assert!(Config::from_json("not json").is_err());
    }

    #[test]
    fn settings_page_cycles_and_resets() {
        let loaded: Loaded = Ok(Config::default());
        let labels = labels(&loaded);
        assert_eq!(labels.len(), 10);
        assert_eq!(labels[0], "Right-drag: one item per slot: ON");
        assert_eq!(labels[4], "Wheel direction: down sends, up brings back");
        assert_eq!(labels[5], "Wheel brings back from: last slot");
        assert_eq!(labels[6], "Sort (key or middle click): ON");
        assert_eq!(labels[7], "Sort order: creative tabs");
        assert_eq!(labels[8], "Sort the hotbar too: OFF");
        let Some(Effect::Save(off)) = click(&loaded, 0) else {
            panic!("a toggle saves");
        };
        assert!(!off.right_drag);
        let mut c = Config::default();
        for want in [
            WheelDirection::Inverted,
            WheelDirection::Positional,
            WheelDirection::Normal,
        ] {
            let Some(Effect::Save(next)) = click(&Ok(c.clone()), 4) else {
                panic!("the direction cycles");
            };
            assert_eq!(next.wheel_direction, want);
            c = next;
        }
        let mut c = Config::default();
        for want in [SortOrder::Name, SortOrder::Count, SortOrder::Registry] {
            let Some(Effect::Save(next)) = click(&Ok(c.clone()), 7) else {
                panic!("the sort order cycles");
            };
            assert_eq!(next.sort_order, want);
            c = next;
        }
        let Some(Effect::Save(hotbar)) = click(&loaded, 8) else {
            panic!("a toggle saves");
        };
        assert!(hotbar.sort_hotbar);
        // The wheel's and the sort's options hide while they are off.
        let no_wheel: Loaded = Ok(Config {
            wheel: false,
            ..Config::default()
        });
        assert_eq!(page(&no_wheel).len(), 8);
        assert_eq!(page(&no_wheel)[4], Setting::Sort);
        assert_eq!(page(&no_wheel)[7], Setting::Defaults);
        assert_eq!(click(&no_wheel, 7), Some(Effect::Save(Config::default())));
        assert_eq!(click(&no_wheel, 9), None);
        let no_sort: Loaded = Ok(Config {
            sort: false,
            ..Config::default()
        });
        assert_eq!(page(&no_sort).len(), 8);
        assert_eq!(page(&no_sort)[6], Setting::Sort);
        assert_eq!(page(&no_sort)[7], Setting::Defaults);
        assert_eq!(
            Setting::Sort.label(&no_sort),
            "Sort (key or middle click): OFF"
        );
    }

    #[test]
    fn a_broken_file_shows_its_error() {
        let broken: Loaded = Err("unknown field `rightdrag`, expected one of many".into());
        let labels = labels(&broken);
        assert_eq!(labels.len(), 2);
        assert!(labels[0].starts_with("config.json: unknown field"));
        assert!(labels[0].ends_with("... (click to reload)"));
        assert_eq!(click(&broken, 0), Some(Effect::Reload));
        assert_eq!(click(&broken, 1), Some(Effect::Save(Config::default())));
    }

    #[test]
    fn presses_start_the_drags_their_features_allow() {
        let all = Config::default();
        let none = Config {
            right_drag: false,
            left_drag: false,
            shift_drag: false,
            ..Config::default()
        };
        let cases = [
            (press(Button::Right, 9, false, true), Some(Mode::Place)),
            (press(Button::Right, 9, false, false), None),
            (press(Button::Right, 9, true, true), None),
            (press(Button::Left, 9, false, true), Some(Mode::Matching)),
            (press(Button::Left, 9, false, false), Some(Mode::Matching)),
            (press(Button::Left, 9, true, true), Some(Mode::Matching)),
            (press(Button::Left, 9, true, false), Some(Mode::MoveAll)),
        ];
        for (p, mode) in cases {
            assert_eq!(Drag::start(&p, &all).map(|d| d.mode), mode, "{p:?}");
            assert_eq!(Drag::start(&p, &none), None, "{p:?}");
        }
    }

    #[test]
    fn right_drag_places_one_per_slot_entered() {
        let mut menu = Fake::chest().with(11, "dirt", 5).with(12, "stone", 3);
        menu = menu.holding("dirt", 10);
        let mut drag = Drag::start(&press(Button::Right, 9, false, true), &Config::default())
            .expect("a placing drag");
        // Still in the pressed slot: the game's (a plain right click).
        assert!(!drag.moved(&mut menu, &at(Some(9), false, Some(vec![9]))));
        assert!(menu.clicks.is_empty());
        // Into the next slot: the pressed one and this one get an item each.
        assert!(drag.moved(&mut menu, &at(Some(10), false, Some(vec![9]))));
        assert_eq!(menu.taken_over, 1);
        assert_eq!((menu.count(9), menu.count(10)), (1, 1));
        // A slot of the same item takes one more; another item takes none
        // (no swap).
        assert!(drag.moved(&mut menu, &at(Some(11), false, None)));
        assert!(drag.moved(&mut menu, &at(Some(12), false, None)));
        assert_eq!((menu.count(11), menu.slot(12)), (6, Some(("stone", 3))));
        // Out and back in: one more each time a slot is entered.
        assert!(drag.moved(&mut menu, &at(None, false, None)));
        assert!(drag.moved(&mut menu, &at(Some(10), false, None)));
        assert!(drag.moved(&mut menu, &at(Some(10), false, None)));
        assert_eq!(menu.count(10), 2);
        assert_eq!(menu.carried, Some(("dirt", 6)));
        assert_eq!(menu.total("dirt"), 15);
        assert!(drag.owned);
    }

    #[test]
    fn right_drag_stops_when_the_hand_is_empty() {
        let mut menu = Fake::chest().holding("pearl", 2).with(10, "pearl", 16);
        let mut drag = Drag::start(&press(Button::Right, 9, false, true), &Config::default())
            .expect("a placing drag");
        drag.moved(&mut menu, &at(Some(10), false, Some(vec![9])));
        // Slot 10 is full: only the pressed slot took one.
        assert_eq!((menu.count(9), menu.count(10)), (1, 16));
        drag.moved(&mut menu, &at(Some(11), false, None));
        drag.moved(&mut menu, &at(Some(12), false, None));
        assert_eq!((menu.count(11), menu.count(12)), (1, 0));
        assert_eq!(menu.carried, None);
    }

    #[test]
    fn right_drag_without_the_games_drag_is_left_alone() {
        let mut menu = Fake::chest().holding("dirt", 10);
        let mut drag = Drag::start(&press(Button::Right, 9, false, true), &Config::default())
            .expect("a placing drag");
        assert!(!drag.moved(&mut menu, &at(Some(10), false, None)));
        assert!(!drag.moved(&mut menu, &at(Some(11), false, Some(vec![9]))));
        assert!(menu.clicks.is_empty());
        assert_eq!(menu.taken_over, 0);
    }

    #[test]
    fn left_drag_gathers_the_same_item() {
        let mut menu = Fake::chest()
            .holding("dirt", 5)
            .with(10, "stone", 4)
            .with(11, "dirt", 20)
            .with(12, "dirt", 50)
            .with(13, "dirt", 39);
        let mut drag = Drag::start(&press(Button::Left, 9, false, true), &Config::default())
            .expect("a matching drag");
        // Another item: still the game's.
        assert!(!drag.moved(&mut menu, &at(Some(10), false, Some(vec![9]))));
        assert!(drag.moved(&mut menu, &at(Some(11), false, Some(vec![9]))));
        assert_eq!(menu.carried, Some(("dirt", 25)));
        assert_eq!(menu.count(11), 0);
        // 25 + 50 doesn't fit in a hand; 25 + 39 does.
        assert!(drag.moved(&mut menu, &at(Some(12), false, None)));
        assert!(drag.moved(&mut menu, &at(Some(13), false, None)));
        assert_eq!(menu.carried, Some(("dirt", 64)));
        assert_eq!((menu.count(12), menu.count(13)), (50, 0));
        assert_eq!(menu.total("dirt"), 114);
        assert_eq!(menu.total("stone"), 4);
    }

    #[test]
    fn left_drag_after_picking_a_stack_up() {
        // The press took slot 9's stack up (the game's click); the drag
        // then gathers that item.
        let mut menu = Fake::chest().holding("dirt", 3).with(10, "dirt", 4);
        let mut drag = Drag::start(&press(Button::Left, 9, false, false), &Config::default())
            .expect("a matching drag");
        assert!(drag.moved(&mut menu, &at(Some(10), false, None)));
        assert_eq!(menu.carried, Some(("dirt", 7)));
        // An empty-handed press on an empty slot gathers nothing.
        let mut empty = Fake::chest().with(10, "dirt", 4);
        let mut drag = Drag::start(&press(Button::Left, 9, false, false), &Config::default())
            .expect("a matching drag");
        assert!(!drag.moved(&mut empty, &at(Some(10), false, None)));
        assert!(empty.clicks.is_empty());
    }

    #[test]
    fn left_drag_gives_way_to_the_games_spread() {
        let mut menu = Fake::chest().holding("dirt", 10).with(11, "dirt", 2);
        let mut drag = Drag::start(&press(Button::Left, 9, false, true), &Config::default())
            .expect("a matching drag");
        assert!(!drag.moved(&mut menu, &at(Some(10), false, Some(vec![9]))));
        // The game's drag took slot 10 too: it splits the stack, not the mod.
        assert!(!drag.moved(&mut menu, &at(Some(11), false, Some(vec![9, 10]))));
        assert!(!drag.moved(&mut menu, &at(Some(9), false, None)));
        assert!(menu.clicks.is_empty());
    }

    #[test]
    fn left_drag_with_shift_moves_the_same_item() {
        let mut menu = Fake::chest()
            .holding("dirt", 1)
            .with(9, "dirt", 8)
            .with(10, "dirt", 20)
            .with(11, "stone", 4)
            .with(12, "dirt", 7);
        let mut drag = Drag::start(&press(Button::Left, 9, false, true), &Config::default())
            .expect("a matching drag");
        assert!(drag.moved(&mut menu, &at(Some(10), true, Some(vec![9]))));
        assert!(drag.moved(&mut menu, &at(Some(11), true, None)));
        assert!(drag.moved(&mut menu, &at(Some(12), true, None)));
        // The pressed slot too, since it holds the same item.
        assert_eq!(menu.count(9) + menu.count(10) + menu.count(12), 0);
        assert_eq!(menu.count(300), 35);
        assert_eq!(menu.count(11), 4);
        assert_eq!(menu.carried, Some(("dirt", 1)));
    }

    #[test]
    fn shift_drag_moves_every_stack_entered_once() {
        let mut menu = Fake::chest()
            .with(10, "dirt", 8)
            .with(11, "stone", 4)
            .with(12, "pearl", 3);
        let mut drag = Drag::start(&press(Button::Left, 9, true, false), &Config::default())
            .expect("a moving drag");
        // In the pressed slot (the game's Shift click moved it).
        assert!(!drag.moved(&mut menu, &at(Some(9), true, None)));
        assert!(drag.moved(&mut menu, &at(Some(10), true, None)));
        assert!(drag.moved(&mut menu, &at(Some(11), true, None)));
        // Shift up: the drag pauses; the slot is left for later.
        assert!(drag.moved(&mut menu, &at(Some(12), false, None)));
        assert_eq!(menu.count(12), 3);
        assert!(drag.moved(&mut menu, &at(None, true, None)));
        assert!(drag.moved(&mut menu, &at(Some(12), true, None)));
        assert_eq!(menu.slot(300), Some(("dirt", 8)));
        assert_eq!(menu.slot(301), Some(("stone", 4)));
        assert_eq!(menu.slot(302), Some(("pearl", 3)));
        // Back over a slot that was moved, now refilled: not moved again.
        menu.slots.insert(10, ("dirt", 1));
        drag.moved(&mut menu, &at(None, true, None));
        drag.moved(&mut menu, &at(Some(10), true, None));
        assert_eq!(menu.count(10), 1);
        assert_eq!(menu.taken_over, 1);
    }

    #[test]
    fn layout_follows_quick_move_order() {
        let chest = Layout {
            container: (300..327).collect(),
        };
        let mut player: Vec<usize> = (0..9).rev().collect();
        player.extend((9..36).rev());
        assert_eq!(chest.other_side(305), player);
        assert_eq!(chest.other_side(4), (300..327).collect::<Vec<_>>());
        assert_eq!(chest.other_side(20), (300..327).collect::<Vec<_>>());
        assert!(chest.other_side(100).is_empty());
        assert!(chest.upper(300) && !chest.upper(0) && !chest.upper(20));
        let inventory = Layout::default();
        assert_eq!(inventory.other_side(3), (9..36).collect::<Vec<_>>());
        assert_eq!(inventory.other_side(30), (0..9).collect::<Vec<_>>());
        assert!(inventory.other_side(500).is_empty());
        assert!(inventory.upper(9) && !inventory.upper(8));
        let mut screen: Vec<usize> = (9..36).collect();
        screen.extend(0..9);
        assert_eq!(chest.screen_order(301), screen);
    }

    #[test]
    fn wheel_down_sends_one_item_at_a_time() {
        let mut menu = Fake::chest().with(300, "dirt", 3).with(5, "dirt", 64);
        menu.slots.insert(8, ("dirt", 10));
        let layout = menu.layout.clone();
        let c = Config::default();
        // Onto the hotbar's dirt with room (slot 8, the first in order).
        assert_eq!(scroll(&mut menu, &layout, &c, 300, -2), 2);
        assert_eq!((menu.count(300), menu.count(8)), (1, 12));
        // The last item: the slot empties.
        assert_eq!(scroll(&mut menu, &layout, &c, 300, -5), 1);
        assert_eq!((menu.count(300), menu.count(8)), (0, 13));
        assert_eq!(menu.carried, None);
        assert_eq!(menu.total("dirt"), 77);
    }

    #[test]
    fn wheel_send_fills_the_first_empty_slot_without_a_stack() {
        let mut menu = Fake::chest().with(20, "stone", 5).with(300, "dirt", 1);
        let layout = menu.layout.clone();
        assert_eq!(scroll(&mut menu, &layout, &Config::default(), 20, -1), 1);
        assert_eq!(menu.slot(301), Some(("stone", 1)));
        assert_eq!(menu.count(20), 4);
        // Nowhere to go: nothing moves.
        let mut full = Fake::chest().with(20, "stone", 5);
        for s in 300..327 {
            full.slots.insert(s, ("dirt", 64));
        }
        let layout = full.layout.clone();
        assert_eq!(scroll(&mut full, &layout, &Config::default(), 20, -1), 0);
        assert!(full.clicks.is_empty());
    }

    #[test]
    fn wheel_send_into_a_refusing_slot_puts_everything_back() {
        let mut menu = Fake::chest().with(20, "stone", 5);
        menu.refusing.push(300);
        let layout = menu.layout.clone();
        assert_eq!(scroll(&mut menu, &layout, &Config::default(), 20, -1), 0);
        assert_eq!((menu.count(20), menu.carried), (5, None));
    }

    #[test]
    fn wheel_up_brings_one_back_from_the_search_end() {
        let mut menu = Fake::chest()
            .with(4, "dirt", 10)
            .with(300, "dirt", 2)
            .with(310, "dirt", 2);
        let layout = menu.layout.clone();
        let mut c = Config::default();
        assert_eq!(scroll(&mut menu, &layout, &c, 4, 1), 1);
        assert_eq!(
            (menu.count(300), menu.count(310), menu.count(4)),
            (2, 1, 11)
        );
        c.wheel_search = WheelSearch::FirstToLast;
        assert_eq!(scroll(&mut menu, &layout, &c, 4, 3), 3);
        assert_eq!(
            (menu.count(300), menu.count(310), menu.count(4)),
            (0, 0, 14)
        );
        // Nothing of it left on the other side.
        assert_eq!(scroll(&mut menu, &layout, &c, 4, 1), 0);
        // A full stack brings nothing; an empty slot neither.
        let mut full = Fake::chest().with(4, "pearl", 16).with(300, "pearl", 1);
        let layout = full.layout.clone();
        assert_eq!(scroll(&mut full, &layout, &c, 4, 1), 0);
        assert_eq!(scroll(&mut full, &layout, &c, 5, 1), 0);
        assert!(full.clicks.is_empty());
    }

    #[test]
    fn wheel_directions() {
        let start = || Fake::chest().with(4, "dirt", 10).with(300, "dirt", 10);
        let cases = [
            // (direction, slot, steps, slot 4 after, slot 300 after)
            (WheelDirection::Normal, 4, -1, 9, 11),
            (WheelDirection::Normal, 4, 1, 11, 9),
            (WheelDirection::Inverted, 4, -1, 11, 9),
            (WheelDirection::Inverted, 4, 1, 9, 11),
            // The container is above the inventory: up over the inventory
            // sends up into it, down over the container sends down.
            (WheelDirection::Positional, 4, 1, 9, 11),
            (WheelDirection::Positional, 4, -1, 11, 9),
            (WheelDirection::Positional, 300, -1, 11, 9),
            (WheelDirection::Positional, 300, 1, 9, 11),
        ];
        for (direction, slot, steps, inv, chest) in cases {
            let mut menu = start();
            let layout = menu.layout.clone();
            let c = Config {
                wheel_direction: direction,
                ..Config::default()
            };
            assert_eq!(scroll(&mut menu, &layout, &c, slot, steps), 1);
            assert_eq!(
                (menu.count(4), menu.count(300)),
                (inv, chest),
                "{direction:?} {slot} {steps}"
            );
        }
    }

    #[test]
    fn wheel_trades_hotbar_and_inventory_without_a_container() {
        let mut menu = Fake::default().with(0, "dirt", 2).with(20, "dirt", 1);
        let layout = Layout::default();
        assert_eq!(scroll(&mut menu, &layout, &Config::default(), 0, -1), 1);
        assert_eq!((menu.count(0), menu.count(20)), (1, 2));
        let c = Config {
            wheel_direction: WheelDirection::Positional,
            ..Config::default()
        };
        // The hotbar is below the main inventory: up sends from it.
        assert_eq!(scroll(&mut menu, &layout, &c, 0, 1), 1);
        assert_eq!((menu.count(0), menu.count(20)), (0, 3));
    }

    #[test]
    fn wheel_needs_an_empty_hand() {
        let mut menu = Fake::chest().with(4, "dirt", 10).holding("stone", 1);
        let layout = menu.layout.clone();
        assert_eq!(scroll(&mut menu, &layout, &Config::default(), 4, -1), 0);
        assert!(menu.clicks.is_empty());
    }

    #[test]
    fn wheel_steps_keep_fractions() {
        let mut wheel = Wheel::default();
        assert_eq!(wheel.steps(0.4), 0);
        assert_eq!(wheel.steps(0.4), 0);
        assert_eq!(wheel.steps(0.4), 1);
        assert_eq!(wheel.steps(-3.0), -2);
        assert_eq!(wheel.steps(f32::NAN), 0);
        assert_eq!(wheel.steps(1000.0), 64);
        wheel.reset();
        assert_eq!(wheel.steps(-0.9), 0);
    }

    // ---- Sorting ----

    /// What the game would tell the sort of each test item: its creative
    /// tabs place, its name, its damage, whether it is a bundle.
    fn kind_of(item: &str) -> Kind {
        let (order, name, damage) = match item {
            "stone" => (5, "Stone", 0),
            "dirt" => (10, "Dirt", 0),
            // Dirt renamed in an anvil: the same item, another kind.
            "dirt:named" => (10, "Aardvark", 0),
            "sword" => (100, "Iron Sword", 0),
            "sword:worn" => (100, "Iron Sword", 40),
            "egg" => (250, "Egg", 0),
            "pearl" => (300, "Ender Pearl", 0),
            "bundle" => (400, "Bundle", 0),
            "bundle:red" => (401, "Red Bundle", 0),
            "zinc" => (u64::MAX, "zinc", 0),
            other => panic!("no test item {other}"),
        };
        Kind {
            max: max_of(item),
            order,
            name: name.into(),
            damage,
            pinned: item.starts_with("bundle"),
        }
    }
    const ITEMS: [&str; 10] = [
        "stone",
        "dirt",
        "dirt:named",
        "sword",
        "sword:worn",
        "egg",
        "pearl",
        "bundle",
        "bundle:red",
        "zinc",
    ];

    /// Plans the sort of `section` in `menu` as the game does.
    fn sorting(
        menu: &mut Fake,
        section: &[usize],
        order: SortOrder,
        paced: bool,
    ) -> Sorting<Stack> {
        let snap = snapshot(menu, section);
        let kinds: Vec<Kind> = snap.stacks.iter().map(|s| kind_of(s.0)).collect();
        let steps = plan_sort(&snap.pieces, &kinds, order);
        Sorting::new(section.to_vec(), snap.stacks, steps, paced)
    }
    fn sort(menu: &mut Fake, section: &[usize], order: SortOrder) {
        let mut s = sorting(menu, section, order, false);
        assert!(!s.run(menu, 0.0));
        assert!(s.done());
    }
    fn contents(menu: &Fake, section: &[usize]) -> Vec<Option<Stack>> {
        section.iter().map(|s| menu.slots.get(s).copied()).collect()
    }
    fn totals(menu: &Fake) -> BTreeMap<&'static str, u32> {
        let mut totals = BTreeMap::new();
        for s in menu.slots.values().chain(&menu.carried) {
            *totals.entry(s.0).or_default() += s.1;
        }
        totals
    }

    #[test]
    fn the_section_under_the_cursor() {
        let chest: Vec<usize> = (300..327).collect();
        let main: Vec<usize> = (9..36).collect();
        let mut both = main.clone();
        both.extend(0..9);
        assert_eq!(sort_section(Some(305), &chest, false, false), chest);
        assert_eq!(sort_section(Some(20), &chest, true, false), main);
        // The hotbar is the inventory's: its main part, unless the setting
        // takes the hotbar too.
        assert_eq!(sort_section(Some(3), &chest, false, false), main);
        assert_eq!(sort_section(Some(3), &chest, false, true), both);
        // Armour, offhand and crafting grid: the inventory.
        for slot in [500, 504, 100] {
            assert_eq!(sort_section(Some(slot), &[], false, false), main);
        }
        // Over no slot: the nearer part.
        assert_eq!(sort_section(None, &chest, true, false), chest);
        assert_eq!(sort_section(None, &chest, false, false), main);
        // No container to sort: always the inventory.
        assert_eq!(sort_section(None, &[], true, false), main);
    }

    #[test]
    fn left_click_takes_puts_tops_up_and_swaps() {
        let kinds = [kind_of("dirt"), kind_of("stone")];
        let dirt = |count| Some(Piece { kind: 0, count });
        let stone = |count| Some(Piece { kind: 1, count });
        let mut slots = vec![dirt(40), dirt(50), stone(3), None];
        let mut hand = None;
        left_click(&mut slots, &mut hand, &kinds, 0);
        assert_eq!((slots[0], hand), (None, dirt(40)));
        left_click(&mut slots, &mut hand, &kinds, 1);
        assert_eq!((slots[1], hand), (dirt(64), dirt(26)));
        left_click(&mut slots, &mut hand, &kinds, 2);
        assert_eq!((slots[2], hand), (dirt(26), stone(3)));
        left_click(&mut slots, &mut hand, &kinds, 3);
        assert_eq!((slots[3], hand), (stone(3), None));
        // A full stack on a partial one of its kind: their counts swap.
        let mut slots = vec![dirt(64), dirt(10)];
        left_click(&mut slots, &mut hand, &kinds, 0);
        left_click(&mut slots, &mut hand, &kinds, 1);
        left_click(&mut slots, &mut hand, &kinds, 0);
        assert_eq!((slots, hand), (vec![dirt(10), dirt(64)], None));
    }

    #[test]
    fn a_sorted_section_takes_no_click() {
        let mut menu = Fake::chest()
            .with(300, "stone", 64)
            .with(301, "dirt", 64)
            .with(302, "dirt", 3)
            .with(303, "pearl", 16);
        let section: Vec<usize> = (300..327).collect();
        let s = sorting(&mut menu, &section, SortOrder::Registry, false);
        assert!(s.done());
        sort(&mut menu, &section, SortOrder::Registry);
        assert!(menu.clicks.is_empty());
    }

    #[test]
    fn sorting_merges_then_orders() {
        let section: Vec<usize> = (300..309).collect();
        let mut menu = Fake::chest()
            .with(300, "pearl", 5)
            .with(301, "dirt", 30)
            .with(303, "stone", 2)
            .with(304, "dirt", 50)
            .with(305, "pearl", 15)
            .with(307, "dirt", 10)
            // Outside the section: never touched.
            .with(9, "dirt", 7);
        sort(&mut menu, &section, SortOrder::Registry);
        assert_eq!(
            contents(&menu, &section),
            vec![
                Some(("stone", 2)),
                Some(("dirt", 64)),
                Some(("dirt", 26)),
                Some(("pearl", 16)),
                Some(("pearl", 4)),
                None,
                None,
                None,
                None,
            ]
        );
        assert_eq!(menu.slot(9), Some(("dirt", 7)));
        assert!(
            menu.clicks
                .iter()
                .all(|(s, c)| section.contains(s) && *c == Click::Left)
        );
        assert_eq!(menu.carried, None);
    }

    #[test]
    fn two_stacks_out_of_order_swap_in_three_clicks() {
        let section: Vec<usize> = (9..36).collect();
        let mut menu = Fake::default().with(9, "pearl", 3).with(10, "stone", 8);
        sort(&mut menu, &section, SortOrder::Registry);
        assert_eq!(
            (menu.slot(9), menu.slot(10)),
            (Some(("stone", 8)), Some(("pearl", 3)))
        );
        assert_eq!(menu.clicks.len(), 3);
        // Into an empty slot: two clicks.
        let mut menu = Fake::default().with(12, "stone", 8);
        sort(&mut menu, &section, SortOrder::Registry);
        assert_eq!(menu.clicks, vec![(12, Click::Left), (9, Click::Left)]);
    }

    #[test]
    fn a_partial_stack_goes_after_the_full_ones() {
        let section: Vec<usize> = (300..303).collect();
        let mut menu = Fake::chest().with(300, "dirt", 20).with(301, "dirt", 64);
        sort(&mut menu, &section, SortOrder::Registry);
        assert_eq!(
            contents(&menu, &section),
            vec![Some(("dirt", 64)), Some(("dirt", 20)), None]
        );
        assert_eq!(menu.clicks.len(), 3);
    }

    #[test]
    fn different_components_never_merge() {
        let section: Vec<usize> = (300..306).collect();
        let mut menu = Fake::chest()
            .with(300, "sword:worn", 1)
            .with(301, "dirt", 10)
            .with(302, "dirt:named", 5)
            .with(303, "sword", 1)
            .with(304, "dirt", 10);
        sort(&mut menu, &section, SortOrder::Registry);
        // Dirt and the renamed dirt share the item: the name orders them;
        // the swords tie but for damage.
        assert_eq!(
            contents(&menu, &section),
            vec![
                Some(("dirt:named", 5)),
                Some(("dirt", 20)),
                Some(("sword", 1)),
                Some(("sword:worn", 1)),
                None,
                None,
            ]
        );
    }

    #[test]
    fn sort_orders() {
        let section: Vec<usize> = (300..305).collect();
        let start = || {
            Fake::chest()
                .with(300, "pearl", 16)
                .with(301, "pearl", 16)
                .with(302, "stone", 3)
                .with(303, "egg", 5)
                .with(304, "zinc", 1)
        };
        let cases = [
            (
                SortOrder::Registry,
                ["stone", "egg", "pearl", "pearl", "zinc"],
            ),
            (SortOrder::Name, ["egg", "pearl", "pearl", "stone", "zinc"]),
            (SortOrder::Count, ["pearl", "pearl", "egg", "stone", "zinc"]),
        ];
        for (order, want) in cases {
            let mut menu = start();
            sort(&mut menu, &section, order);
            let got: Vec<&str> = contents(&menu, &section)
                .iter()
                .flatten()
                .map(|s| s.0)
                .collect();
            assert_eq!(got, want, "{order:?}");
        }
    }

    #[test]
    fn bundles_stay_where_they_are() {
        let section: Vec<usize> = (300..306).collect();
        let mut menu = Fake::chest()
            .with(300, "pearl", 4)
            .with(301, "bundle", 1)
            .with(302, "stone", 9)
            .with(304, "bundle:red", 1)
            .with(305, "dirt", 2);
        // The fake menu fails the test on any click of a stack on a bundle.
        sort(&mut menu, &section, SortOrder::Registry);
        assert_eq!(
            contents(&menu, &section),
            vec![
                Some(("stone", 9)),
                Some(("bundle", 1)),
                Some(("dirt", 2)),
                Some(("pearl", 4)),
                Some(("bundle:red", 1)),
                None,
            ]
        );
    }

    #[test]
    fn nothing_moves_with_a_stack_in_hand_or_a_stack_too_big() {
        let section: Vec<usize> = (300..303).collect();
        let mut menu = Fake::chest().with(300, "pearl", 4).with(301, "stone", 9);
        let mut s = sorting(&mut menu, &section, SortOrder::Registry, false);
        menu.carried = Some(("dirt", 1));
        assert!(!s.run(&mut menu, 0.0));
        assert!(menu.clicks.is_empty());
        let mut big = Fake::chest().with(300, "pearl", 40).with(301, "stone", 9);
        assert!(sorting(&mut big, &section, SortOrder::Registry, false).done());
    }

    #[test]
    fn a_paced_sort_spreads_over_ticks_and_stops_when_things_change() {
        let section: Vec<usize> = (300..327).collect();
        let start = || {
            let mut menu = Fake::chest();
            for (i, slot) in section.iter().enumerate() {
                let item = ["pearl", "dirt", "stone", "egg"][i % 4];
                menu.slots.insert(*slot, (item, 1 + i as u32 % 5));
            }
            menu
        };
        let mut menu = start();
        let mut s = sorting(&mut menu, &section, SortOrder::Registry, true);
        let longest = s.steps.iter().map(|s| s.clicks.len()).max().unwrap_or(0);
        let mut runs = 0;
        let mut more = s.run(&mut menu, 0.0);
        while more {
            let before = menu.clicks.len();
            more = s.run(&mut menu, TICK_SECONDS);
            let made = menu.clicks.len() - before;
            assert!(made < SORT_CLICKS_PER_TICK as usize + longest, "{made}");
            runs += 1;
            assert!(runs < 1000);
        }
        assert!(runs > 2, "{runs}");
        let mut unpaced = start();
        sort(&mut unpaced, &section, SortOrder::Registry);
        assert_eq!(contents(&menu, &section), contents(&unpaced, &section));
        // The section changes between two steps: the sort stops, the hand
        // empty and every item still there.
        let mut menu = start();
        let mut s = sorting(&mut menu, &section, SortOrder::Registry, true);
        assert!(s.run(&mut menu, 0.0));
        let made = menu.clicks.len();
        menu.slots.insert(326, ("zinc", 1));
        let total = totals(&menu);
        assert!(!s.run(&mut menu, TICK_SECONDS));
        assert_eq!(menu.clicks.len(), made);
        assert!(s.done());
        assert_eq!((menu.carried, totals(&menu)), (None, total));
    }

    /// xorshift64*: a small, fixed random sequence for the property test.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    /// The order a sorted section must show, written out apart from
    /// `sorted`: (the order's keys, damage, the kind's first slot before the
    /// sort, then larger stacks first).
    fn sort_key(
        stack: Stack,
        order: SortOrder,
        totals: &BTreeMap<&str, u32>,
        first: &BTreeMap<&str, usize>,
    ) -> (u64, String, u64, u32, usize, std::cmp::Reverse<u32>) {
        let k = kind_of(stack.0);
        let most = u64::from(u32::MAX - totals[stack.0]);
        let name = k.name.to_lowercase();
        let (a, b, c) = match order {
            SortOrder::Registry => (k.order, name, most),
            SortOrder::Name => (0, format!("{name}\0{:020}", k.order), most),
            SortOrder::Count => (most, format!("{:020}\0{name}", k.order), 0),
        };
        (
            a,
            b,
            c,
            k.damage,
            first[stack.0],
            std::cmp::Reverse(stack.1),
        )
    }

    #[test]
    fn random_sections_end_sorted_with_every_item_kept() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let orders = [SortOrder::Registry, SortOrder::Name, SortOrder::Count];
        let mut clicks = 0;
        for round in 0..3000 {
            let order = orders[round % 3];
            // A chest (27 or 54 slots) or the inventory (main, or with the
            // hotbar), filled from empty to full, few kinds or many.
            let (mut menu, section): (Fake, Vec<usize>) = match round % 4 {
                0 => (Fake::chest(), (300..327).collect()),
                1 => (Fake::chest(), (300..354).collect()),
                2 => (Fake::default(), sort_section(Some(10), &[], false, false)),
                _ => (Fake::default(), sort_section(Some(1), &[], false, true)),
            };
            let fill = rng.below(101);
            let kinds = 1 + rng.below(ITEMS.len() as u64) as usize;
            for &slot in section.iter().chain(&[0, 300, 500]) {
                if rng.below(100) < fill {
                    let item = ITEMS[rng.below(kinds as u64) as usize];
                    let count = 1 + rng.below(u64::from(max_of(item))) as u32;
                    menu.slots.insert(slot, (item, count));
                }
            }
            let outside: Vec<(usize, Stack)> = menu
                .slots
                .iter()
                .filter(|(s, _)| !section.contains(s))
                .map(|(s, st)| (*s, *st))
                .collect();
            let before = contents(&menu, &section);
            let total = totals(&menu);
            let snap = snapshot(&mut menu, &section);
            let kind_list: Vec<Kind> = snap.stacks.iter().map(|s| kind_of(s.0)).collect();
            let steps = plan_sort(&snap.pieces, &kind_list, order);
            // Each step expects what the steps before it leave, and the
            // plan's own model of a click agrees with the fake menu's.
            let mut model = snap.pieces.clone();
            for step in &steps {
                assert_eq!(step.before, model, "round {round}");
                let mut hand = None;
                for &i in &step.clicks {
                    left_click(&mut model, &mut hand, &kind_list, i);
                }
                assert_eq!(hand, None, "round {round}");
            }
            assert_eq!(
                model,
                sorted(&snap.pieces, &kind_list, order),
                "round {round}"
            );
            let mut s = Sorting::new(section.clone(), snap.stacks.clone(), steps, false);
            assert!(!s.run(&mut menu, 0.0));
            clicks += menu.clicks.len();

            // Nothing gained, lost or left in the hand; only the section's
            // slots clicked; everything outside it untouched.
            assert_eq!(menu.carried, None, "round {round}");
            assert_eq!(totals(&menu), total, "round {round}");
            assert!(
                menu.clicks.iter().all(|(s, _)| section.contains(s)),
                "round {round}"
            );
            for (slot, stack) in &outside {
                assert_eq!(menu.slots.get(slot), Some(stack), "round {round}");
            }
            let after = contents(&menu, &section);
            // Bundles where they were; the other slots: stacks, then empty.
            let mut seen_empty = false;
            let mut sorted_stacks = Vec::new();
            for (was, now) in before.iter().zip(&after) {
                if bundle(*was) || bundle(*now) {
                    assert_eq!(was, now, "round {round}: a bundle moved");
                    continue;
                }
                match now {
                    None => seen_empty = true,
                    Some(stack) => {
                        assert!(!seen_empty, "round {round}: a stack after an empty slot");
                        sorted_stacks.push(*stack);
                    }
                }
            }
            // Merged: at most one stack of a kind not full.
            for item in ITEMS {
                let partial = sorted_stacks
                    .iter()
                    .filter(|s| s.0 == item && s.1 < max_of(item))
                    .count();
                assert!(partial <= 1, "round {round}: {item} not merged");
            }
            // In order, by keys computed apart from the plan's.
            let mut in_section: BTreeMap<&str, u32> = BTreeMap::new();
            let mut first: BTreeMap<&str, usize> = BTreeMap::new();
            for (i, s) in before.iter().enumerate().filter(|(_, s)| !bundle(**s)) {
                if let Some(s) = s {
                    *in_section.entry(s.0).or_default() += s.1;
                    first.entry(s.0).or_insert(i);
                }
            }
            let keys: Vec<_> = sorted_stacks
                .iter()
                .map(|s| sort_key(*s, order, &in_section, &first))
                .collect();
            assert!(
                keys.windows(2).all(|w| w[0] <= w[1]),
                "round {round} {order:?}: {sorted_stacks:?}"
            );
            // Sorting again takes no click.
            let again = sorting(&mut menu, &section, order, false);
            assert!(again.done(), "round {round}");
        }
        assert!(clicks > 0);
    }
}
