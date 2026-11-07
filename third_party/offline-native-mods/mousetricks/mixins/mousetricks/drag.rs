// Copyright (c) 2026 d3grando
// SPDX-License-Identifier: MIT
// Game::inventory_drag_move HEAD (pinned): a move that belongs to Mouse
// Tricks' drag is its own; any other goes on to the game's quick craft drag.
fn mousetricks_on_drag(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if game.mousetricks_drag_move() {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
