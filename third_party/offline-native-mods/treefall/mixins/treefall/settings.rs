// Copyright (c) 2026 d3grando
// SPDX-License-Identifier: MIT
// native_hooks::Game::native_settings (a hook point): TreeFall's settings page.
fn treefall_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::treefall::ID {
        buttons.extend(game.treefall_settings());
    }
}
