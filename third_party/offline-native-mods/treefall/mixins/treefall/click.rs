// Copyright (c) 2026 d3grando
// SPDX-License-Identifier: MIT
// native_hooks::Game::native_settings_click (a hook point): a click on
// TreeFall's settings page changes the setting, keeps it and applies it.
fn treefall_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    if mod_id == crate::game::treefall::ID {
        game.treefall_settings_click(button);
    }
}
