// Copyright (c) 2026 d3grando
// SPDX-License-Identifier: MIT
// Game::pop_resource HEAD (`dropsAtStump`): while a tree is being felled, its
// drops pop at the stump instead of where each block was.
fn treefall_drops(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    stack: &mut horizon_client::items::ItemStack,
) -> core::ops::ControlFlow<()> {
    if game.treefall_pop_at_stump(pos, stack) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
