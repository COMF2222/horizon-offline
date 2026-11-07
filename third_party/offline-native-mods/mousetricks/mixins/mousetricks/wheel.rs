// Copyright (c) 2026 d3grando
// SPDX-License-Identifier: MIT
// tweaks::Game::tweak_scroll HEAD (pinned): the wheel over an inventory
// screen (after a bundle under the cursor had its turn). Break(true) when
// Mouse Tricks used it, else the game's own handling goes on.
fn mousetricks_on_wheel(
    game: &mut crate::game::Game<'_>,
    delta: f32,
    slot: Option<usize>,
) -> core::ops::ControlFlow<bool> {
    if game.mousetricks_scroll(delta, slot) {
        core::ops::ControlFlow::Break(true)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
