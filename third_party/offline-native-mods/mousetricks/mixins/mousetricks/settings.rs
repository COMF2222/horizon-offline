// Copyright (c) 2026 d3grando
// SPDX-License-Identifier: MIT
// native_hooks::Game::native_settings (a hook point): Mouse Tricks' settings page.
fn mousetricks_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::mousetricks::ID {
        buttons.extend(game.mousetricks_settings());
    }
}
