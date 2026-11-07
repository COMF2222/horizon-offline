Download **Horizon-Offline.exe** below and open it on Windows x64.

The executable carries our patched Horizon game and its matching preparer. On first launch, press **Play**: it downloads the pinned Minecraft files and the required Java runtime directly from Mojang, verifies their checksums and prepares the resources locally. You do not need to install Java, Python, Rust or another launcher.

After preparation, single-player works without Microsoft sign-in or an internet connection. Pick a name in Settings → Account to keep a local profile. Online-mode servers still require a valid Microsoft account.

**Mouse Tricks and TreeFall are included as an optional, precompiled profile.** In the version selector beside **Play**, choose **Offline … · Mods**, then press **Play**. No separate ZIP, JAR, Rust SDK or mod installation is needed. The plain **Offline …** profile remains available. Both profiles receive the same authenticated updates, and the selected variant is kept after an update. See [mod instructions and controls](https://github.com/COMF2222/horizon-offline/blob/main/docs/offline-mods.md).

Mouse Tricks: open an inventory or chest, point at the section to sort and press **R** with an empty cursor. TreeFall: use an axe on a tree in Survival; hold **Shift** for normal single-block breaking. Both mods appear in the game's **Options → Mods** menu. These are MIT-licensed mods by [d3grando](https://github.com/d3grando), adapted to this Horizon source version.

Local PNG skins are shared with friends in an offline LAN/VPN world when everyone uses this updated Horizon Offline build. Select a local nickname and skin in **Settings → Account** before starting the game. Classic and slim models work; older or vanilla clients keep their default skin behavior.

Future launcher and bundled-game updates come from this repository. The updater accepts only newer stable releases authenticated by our pinned Ed25519 publisher key and verified SHA-256 checksums. Worlds and profiles stay in the same data folders. An upstream release is checked every two hours; a conservative source merge, tests, a Windows build and a three-client LAN check must succeed before a new update is published. A conflict or failed check leaves the existing release active.

The first setup requires internet access. Minecraft JARs, assets and prepared Minecraft resources are not included in this executable or this release. Horizon's original code is from [horizon-rust/horizon](https://github.com/horizon-rust/horizon); our source commit and upstream base are recorded with the build. `horizon-offline-windows-x64.zip` is the authenticated updater package; players can keep downloading **Horizon-Offline.exe** directly.

Скачайте **Horizon-Offline.exe** и откройте его. Нажмите **Играть**: нужные файлы Minecraft и Java загрузятся и подготовятся автоматически. Затем можно играть офлайн без Microsoft, а в общем мире по LAN/VPN друзья с этой же сборкой увидят выбранные PNG-скины. Проверенные обновления лаунчера и встроенной игры приходят из этого репозитория; профили и миры сохраняются.

Для модов выберите **«Офлайн … · Моды»** в списке версий рядом с кнопкой **Играть**. Mouse Tricks и TreeFall уже собраны внутри `.exe`: отдельно ничего устанавливать не нужно. **R** сортирует инвентарь или сундук под курсором; TreeFall срубает дерево топором в выживании, **Shift** оставляет обычное разрушение одного блока. Настройки находятся в **Параметры → Моды**. Выбор сборки с модами сохраняется при обновлениях.
