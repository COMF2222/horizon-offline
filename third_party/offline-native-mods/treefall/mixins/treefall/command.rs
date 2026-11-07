// Copyright (c) 2026 d3grando
// SPDX-License-Identifier: MIT
// native_hooks::Game::native_command (a hook point): run `/treefall`, leave
// everything else to the game (and to other mods).
fn treefall_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    if !*ran && game.treefall_command(command) {
        *ran = true;
    }
}
