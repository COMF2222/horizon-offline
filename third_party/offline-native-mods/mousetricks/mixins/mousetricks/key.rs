// Copyright (c) 2026 d3grando
// SPDX-License-Identifier: MIT
// native_hooks::Game::native_key (a hook point): the sort key, over an
// inventory screen (keys reach it on any screen; repeats are left out).
fn mousetricks_on_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    game.mousetricks_key(key, pressed);
}
