# TreeFall

A **native** Horizon mod (horizon-mixins source mixin): cut one log of a natural
tree with an axe and the whole tree comes down. Every log is broken through the
game's ordinary mining path, so drops, statistics, hunger and axe wear are
exactly as if you had chopped each log yourself (all of that is configurable,
in game: Options > Mods > TreeFall > Settings).

In the spirit of FallingTree; not affiliated.

> **Native mods are not sandboxed.** Unlike `.hmod` (Wasm) mods, a native mod is
> Rust source compiled into your own Horizon executable and runs with your user
> rights. Read the source (it is small: `mixins/treefall/`) before building.

## Behaviour

Breaking a block fells a tree when all of this holds:

- the block is a **log**: in the game's `minecraft:logs` block tag (`*_log`,
  `*_wood`, nether `*_stem`/`*_hyphae`, also modded logs added to the tag), but
  **not stripped** (stripped logs are almost always building material); with
  `mushrooms`, also `minecraft:mushroom_stem`;
- its wood is allowed by `woods`;
- the main hand holds what `tool` asks for (by default an **axe**, the
  `minecraft:axes` item tag);
- you are in **survival or adventure** (adventure still obeys `can_break` for
  every log), in single player or hosting; never in creative/spectator or as a
  client of another computer's server;
- the `sneak` rule allows it (by default: **not sneaking**; sneak to chop a
  single log);
- TreeFall is on for you (`/treefall`), in its settings (`enabled`) and in
  Options > Mods.

The tree is every log of the **same wood** (`oak_log` and `oak_wood` count as
one wood, birch next to it does not) connected through the 26 surrounding
blocks, **at or above** the cut block. Logs below the cut stay.

It is felled only if it is a **real tree**: at least `minLeaves` (default **5**)
natural leaves (leaves with `persistent=false`, i.e. grown, not placed by a
player) touch the logs. Nether stems count their crown instead (nether/warped
wart blocks, shroomlight), huge mushrooms their red/brown mushroom-block cap. A
log cabin, a log pillar or a wall decorated with placed leaves is therefore
broken one log at a time, as usual. Groups larger than `maxLogs` (default
**256**; a 2x2 jungle giant is ~130) are not felled at all rather than partially.

The tree comes down **from the top**, and the cut block breaks **last**, so if
felling stops part way, what is left is a stump standing on the ground, the cut
log included, which you can chop again. Felling stops:

- before the tool would drop below `keepDurability` points (default 1: the axe
  is never broken by felling; the cut log's own wear is counted in),
- if the held item changes, or a log fails to break.

If not even one log above the cut can come down (a worn-out axe), the cut log
breaks alone, as in vanilla.

Leaves are **left to decay naturally** (vanilla). With `breakLeaves` they are
broken right away instead (drops as if broken with the held tool: saplings,
sticks, apples, Fortune applies; no tool wear), but only the leaves vanilla
would let decay: within 6 steps of the felled logs and not within 6 steps of any
other log (leaves next to unloaded chunks are left alone). Nether crowns and
mushroom caps never decay and are never broken.

A single log with nothing else to fell is left to the game (or to Veinminer).

## Commands

| Command | Effect |
|---|---|
| `/treefall` or `/treefall status` | Show whether it is on for you |
| `/treefall on` / `off` / `toggle` | Turn it on/off for you (per player, until the game closes) |
| `/treefall reload` | Re-read `config.json` (permission level 2) |

LAN guests can use the same commands, even without command rights; each player
has their own on/off state.

## Settings

**Options > Mods** lists TreeFall with its **Settings** button. The row's own
button turns the mod off (it then does nothing at all; the launcher's next build
leaves it out) and on again. The settings page has one button per setting; a
click moves it to its next value, and it applies at once, for every player of
the world, no restart:

| Button | Key | Values a click cycles through |
|---|---|---|
| Felling: ON | `enabled` | ON, OFF |
| Tool: axe | `tool` | axe, any tool, anything (bare hands too) |
| Sneaking: cuts a single log | `sneak` | cuts a single log, needed to fell, makes no difference |
| Largest tree: 256 logs | `maxLogs` | 16, 32, 64, 128, 256, 512, 1024, 2048, 4096 |
| Leaves needed: 5 | `minLeaves` | 0, 1, 3, 5, 10, 20, 50, 100 |
| Break the leaves: OFF | `breakLeaves` | ON, OFF |
| Cut time: as one log | `breakSpeed` | as one log, grows with the tree |
| Extra time per log: 0.5x | `speedFactor` | 0, 0.1, 0.25, 0.5, 0.75, 1, 2, 5, 10 (shown when the cut time grows) |
| Longest cut: 20x one log | `maxSlowdown` | 1, 2, 3, 5, 10, 20, 50, 100 (shown when the cut time grows) |
| Tool wear: every log | `durability` | every log, once per tree, logs x factor, none |
| Wear factor: 0.5 per log | `durabilityFactor` | 0, 0.1, 0.25, 0.5, 0.75, 1, 2, 5, 10 (shown for logs x factor) |
| Keep durability: 1 | `keepDurability` | 0, 1, 2, 5, 10, 25, 50, 100 |
| Hunger: every log | `exhaustion` | every log, cut log only |
| Drops at the stump: OFF | `dropsAtStump` | ON, OFF |
| Replant a sapling: OFF | `replant` | ON, OFF |
| Huge mushrooms: OFF | `mushrooms` | ON, OFF |
| Woods: all (config.json, click to reload) | `woods` | edited in the file; a click reads the file again |
| Reset to defaults (woods kept) | | every setting above back to its default |

The page shows a setting's current value; a value set by hand in the file
between two steps goes to the next step up. If the file is broken, the page
shows its error (click to read the file again) and a reset.

## Configuration file

The settings page writes, and the mod reads, one JSON file in the mod's
settings folder, beside the game's `options.txt`:

```
horizon-mods/.horizon-mod-data/treefall/config.json
```

(the folder of the instance that plays the native build: the launcher's
"Native mods" instance for a folder built from the command line). Edit it by
hand for the `woods` lists, which the page cannot hold, then click the Woods
button or run `/treefall reload`. Every key is optional; unknown keys or
out-of-range values turn the mod off (the page shows why, and it prints
`TreeFall disabled: ...` to stderr) until the file is fixed or reset. A missing
file is the defaults:

```json
{
  "enabled": true,
  "sneak": "disables",
  "tool": "axe",
  "maxLogs": 256,
  "minLeaves": 5,
  "breakLeaves": false,
  "breakSpeed": "single",
  "speedFactor": 0.5,
  "maxSlowdown": 20.0,
  "durability": "perLog",
  "durabilityFactor": 0.5,
  "keepDurability": 1,
  "dropsAtStump": false,
  "replant": false,
  "exhaustion": "perLog",
  "woods": { "allow": [], "deny": [] },
  "mushrooms": false
}
```

0.2.0 read the file named by `HORIZON_TREEFALL_CONFIG`; 0.3.0 no longer reads
that variable (one place for the settings, the one the page writes). Copy an
old file to `config.json` above to keep it.

| Key | Default | Values |
|---|---|---|
| `enabled` | `true` | Global switch |
| `tool` | `"axe"` | `"axe"` (`minecraft:axes`), `"anyTool"` (any item with a tool component: pickaxes, shovels, hoes, swords, shears...), `"any"` (bare hands too) |
| `sneak` | `"disables"` | `"disables"`: sneak to cut one log. `"enables"`: only sneaking fells. `"ignored"` |
| `maxLogs` | `256` | Largest tree, 1..=4096; bigger log groups are not felled |
| `minLeaves` | `5` | Natural leaves (crown/cap blocks for stems/mushrooms) touching the logs, 0..=1024. `0` fells **any** log structure, cabins included |
| `breakLeaves` | `false` | Also break the leaves that would decay |
| `breakSpeed` | `"single"` | `"single"`: the cut takes as long as one log. `"scaled"`: the cut log takes `1 + (logs - 1) * speedFactor` times longer, at most `maxSlowdown` times |
| `speedFactor` | `0.5` | 0..=10, for `"scaled"` (a 6-log oak: 3.5x) |
| `maxSlowdown` | `20` | 1..=100, for `"scaled"` |
| `durability` | `"perLog"` | `"perLog"`: the game's wear for every log (Unbreaking applies). `"once"`: one use per tree. `"multiplier"`: `ceil(logs * durabilityFactor)` uses per tree. `"none"`: no wear, not even for the cut log |
| `durabilityFactor` | `0.5` | 0..=10, for `"multiplier"` |
| `keepDurability` | `1` | 0..=100000. Stop before the tool would have fewer points left. `0` lets felling use the tool up |
| `exhaustion` | `"perLog"` | `"perLog"`: the game's hunger cost per block broken. `"once"`: only the cut log's |
| `dropsAtStump` | `false` | Pop all drops at the cut block instead of where each block was |
| `replant` | `false` | After a complete felling, plant a sapling of that wood (`*_sapling`, `mangrove_propagule`, `crimson_fungus`/`warped_fungus`) from your inventory at the cut block, if the block below can hold it (the game's own `canSurvive`) |
| `woods` | all | `allow`/`deny` lists of wood keys: `"oak"`, `"dark_oak"`, `"crimson"`, `"minecraft:warped"`, `"mushroom"`, modded `"ns:wood"`... An empty `allow` allows all; `deny` wins |
| `mushrooms` | `false` | Fell huge mushrooms: `mushroom_stem` with a red/brown mushroom-block cap |

Why 5 leaves: the smallest vanilla trees (a 4-log oak, an acacia, an azalea
tree) touch 20+ natural leaves around their top log, while leaves that happen to
grow against a wall rarely reach 5.

Why `breakSpeed` defaults to `"single"`: it is today's behaviour. With
`"scaled"`, in single player or when you host, your game and the world's server
both slow the cut, so it behaves like a slower block: let go of attack and the
cut stops. A LAN guest's own client still predicts the normal (single-log)
time; the host then holds the block back until the slowed time is up, so for
guests the log can briefly reappear before the tree falls. A guest who mines a
log instantly (Efficiency + Haste) is not slowed.

Other notes: `replant` plants one sapling at the cut block, so 2x2 trees (dark
oak, pale oak, mega spruce/jungle) need the other three planted by hand, and
huge mushrooms are not replanted. The `/treefall` on/off toggles are kept in
memory, not saved.

## Compatibility

Horizon's native mods can use **hook points** (`game/native_hooks.rs`,
`NATIVE_HOOKS_API`): functions kept stable from release to release, so a mod that
uses only them builds on any later Horizon. TreeFall uses them for everything they
cover:

- `native_mods`, `native_settings`, `native_settings_click`: the row in Options >
  Mods and the settings page; `native_setting`/`native_set_setting`/`native_on`
  for the file and the on/off;
- `native_command`: `/treefall`;
- `native_tick`: the window passes Options > Mods' on/off to single player's
  server thread.

There are no hook points for the rest, so TreeFall hooks these game functions
directly and is **pinned** to one Horizon commit:

| Hook | Why it is not a hook point |
|---|---|
| `Game::break_block` | Felling replaces the break of a log; there is no mining hook point |
| `hand::Game::destroy_progress` | `breakSpeed: "scaled"` slows the cut; no mining-speed hook point |
| `Game::pop_resource` | `dropsAtStump` moves drops; no drops hook point |
| `lan_actions::Game::guest_command` | lets a LAN guest without command rights run `/treefall`; `native_command` only sees commands that are allowed to run |

Pinned: Horizon `142388e1c136e622652ec7505af15d8ad195e041` (main, with the hook
points and the settings page, horizon-rust/horizon#3345 and #3346), horizon-mixins
`0d6b103c70633d8fd9443d8d0acd3aadaacedb06` (the revision Horizon's own native mods
use), game 26.3. `horizon-native.json` therefore names no `hooks` version: the
launcher builds TreeFall on exactly that commit. Once hook points cover mining,
the pin can go.

Other native mods: the official ones (Zoom, FPS Counter, Keystrokes, Minimap) use
only hook points and the same horizon-mixins, so a launcher that has
horizon-rust/horizon#3323 builds them on TreeFall's commit, together with it
(checked: all five planned together with `horizon-mixins plan`, 27 injections).

### Veinminer

[Veinminer](https://github.com/levzzz5154/horizon-veinminer) also mines logs by
default. Veinminer 0.3.0 pins Horizon `83f15fa` and horizon-mixins `0500ff2`, so it
does **not** build together with TreeFall 0.3.0 (TreeFall 0.2.0 does). When both
pin the same commits again, TreeFall's hooks have priority 200, above Veinminer's
100; same-site HEAD hooks run highest first and the first that cancels skips the
rest:

- a natural tree cut with the configured tool is felled by TreeFall only;
- anything TreeFall declines (sneaking, wrong tool, a log cabin, too few
  leaves, a denied wood) goes on to Veinminer, which may vein-mine those logs
  by its own rules.

## Build and try

TreeFall is a source bundle; the launcher compiles it into a custom game:

```sh
horizon-launcher native-build <this folder> --version v0.2.97 --trust-native-code
```

`--version` is the installed release whose game files the build reuses; the
code is the pinned commit's whatever the release. `<this folder>` may also be a
folder holding several native mods.

## Development

`mixins/treefall/`:

- `horizon-mixin.toml`: two support modules and nine HEAD hooks (above).
- `runtime.rs`: all decisions (tree finding, leaf thresholds, decaying leaves,
  wear, speed, config, commands, the settings page's buttons) as pure code over a
  small `Blocks`/`Host` trait, with unit tests. It is both this repository's
  Cargo library and the injected `crate::game::treefall` module.
- `native.rs`: the adapter to the game (`crate::game::treefall_native`), and the
  state the window and single player's server thread share.
- `mining.rs`, `speed.rs`, `drops.rs`, `guest-command.rs`, `command.rs`,
  `tick.rs`, `list.rs`, `settings.rs`, `click.rs`: the hook helpers.

```sh
cargo test
cargo clippy -- -D warnings
python tools/pack_hnmod.py   # dist/treefall-<version>.hnmod
```

Original code; no code from other tree-felling mods was used. MIT licensed.
