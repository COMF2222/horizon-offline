// Copyright (c) 2026 d3grando
// SPDX-License-Identifier: MIT
// Game::break_block HEAD: fell the tree, or continue with the game's own break.
fn treefall_mining(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
) -> core::ops::ControlFlow<Result<()>> {
    match game.treefall_break_block(pos) {
        Some(result) => core::ops::ControlFlow::Break(result),
        None => core::ops::ControlFlow::Continue(()),
    }
}
