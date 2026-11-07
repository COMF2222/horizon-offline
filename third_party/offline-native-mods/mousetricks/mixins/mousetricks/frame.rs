// Copyright (c) 2026 d3grando
// SPDX-License-Identifier: MIT
// native_hooks::Game::native_frame (a hook point): a drag left over from a
// closed screen ends; a paced sort goes on.
fn mousetricks_frame(game: &mut crate::game::Game<'_>, elapsed: std::time::Duration) {
    game.mousetricks_frame(elapsed);
}
