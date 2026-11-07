// Copyright (c) 2026 d3grando
// SPDX-License-Identifier: MIT
// tweaks::Game::sort_mouse_press HEAD (pinned): a mouse press on any screen,
// before the game handles it. Mouse Tricks notes the drag a left or right
// press may start (that press goes on to the game); a middle click on an
// empty spot (or the sort key bound to a mouse button) sorts, and the game
// takes the press as handled, as it does for its own sort button.
fn mousetricks_on_press(
    game: &mut crate::game::Game<'_>,
    button: winit::event::MouseButton,
) -> core::ops::ControlFlow<bool> {
    if game.mousetricks_press(button) {
        core::ops::ControlFlow::Break(true)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
