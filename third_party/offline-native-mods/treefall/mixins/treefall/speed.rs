// Copyright (c) 2026 d3grando
// SPDX-License-Identifier: MIT
// hand::Game::destroy_progress HEAD (`breakSpeed: "scaled"`): a log that will
// fell a tree takes longer to cut; anything else keeps the game's own speed.
fn treefall_speed(game: &mut crate::game::Game<'_>, pos: [i32; 3]) -> core::ops::ControlFlow<f32> {
    match game.treefall_destroy_progress(pos) {
        Some(progress) => core::ops::ControlFlow::Break(progress),
        None => core::ops::ControlFlow::Continue(()),
    }
}
