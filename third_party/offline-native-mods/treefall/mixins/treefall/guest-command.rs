// Copyright (c) 2026 d3grando
// SPDX-License-Identifier: MIT
// lan_actions::Game::guest_command HEAD: queue a LAN guest's `/treefall` so it
// runs as that guest (their own on/off state), whatever their permission level.
fn treefall_guest_command(
    game: &mut crate::game::Game<'_>,
    host: &mut crate::game::lan::LanHost,
    conn: u32,
    command: &str,
) -> core::ops::ControlFlow<()> {
    if game.treefall_guest_command(host, conn, command) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
