// Copyright (c) 2026 d3grando
// SPDX-License-Identifier: MIT
// native_hooks::Game::native_mods (a hook point): Mouse Tricks' row in Options > Mods.
fn mousetricks_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::mousetricks as mt;
    mods.push(crate::game::native_hooks::NativeMod {
        id: mt::ID.into(),
        title: format!("Mouse Tricks {}", mt::VERSION),
        description: mt::DESCRIPTION.into(),
        keys: mt::KEYS.iter().map(|k| k.map(String::from)).collect(),
    });
}
