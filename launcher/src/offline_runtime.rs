//! The offline distribution carries its own game and matching preparer, never Minecraft files.
//! Each payload installs as an immutable version. Existing prepared resources are reused only
//! through the preparer's compatibility checks; official updates cannot replace this runtime.
use crate::{backend, i18n};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Read},
    path::{Component, Path},
};

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/offline-runtime.rs"));
}

const PREFIX: &str = "offline-bundled-";
pub const VERSION: &str = match option_env!("HORIZON_OFFLINE_VERSION") {
    Some(version) => version,
    None => "dev",
};
const MARKER: &str = ".offline-runtime.sha256";
const VERSION_MARKER: &str = ".offline-runtime.version";
const MODS_SUFFIX: &str = "-mods";
const MODS_PROGRAM: &str = "optional-mods/horizon-client.exe";
const MAX_FILE: u64 = 512 * 1024 * 1024;
const MAX_TOTAL: u64 = 1024 * 1024 * 1024;
const REQUIRED: &[&str] = &[
    "horizon-client.exe",
    "horizon-ctl.exe",
    "horizon-prepare.exe",
    "horizon-server.exe",
    "research-lock.json",
    "crates/horizon-prepare/src/lib.rs",
    "mods/horizon/src/test/java/dev/horizonlab/parity/PrepareWorker.java",
];

/// Source builds without a payload retain the original launcher's installation behavior.
pub fn available() -> bool {
    !embedded::ARCHIVE.is_empty()
}

/// Whether a version belongs to this distribution, including its previously installed payloads.
pub fn is_bundled(tag: &str) -> bool {
    tag.starts_with(PREFIX)
}

pub fn has_mods(tag: &str) -> bool {
    is_bundled(tag) && tag.ends_with(MODS_SUFFIX)
}

fn payload_group(tag: &str) -> &str {
    tag.strip_suffix(MODS_SUFFIX).unwrap_or(tag)
}

/// The display label for our immutable version, without its internal content hash.
pub fn label(tag: &str) -> Option<String> {
    is_bundled(tag).then(|| {
        let version = fs::read_to_string(backend::version_dir(tag).join(VERSION_MARKER))
            .ok()
            .filter(|version| !version.trim().is_empty())
            .unwrap_or_else(|| VERSION.to_string());
        if has_mods(tag) {
            i18n::trf("Offline {version} · Mods", &[("version", version.trim())])
        } else {
            i18n::trf("Offline {version}", &[("version", version.trim())])
        }
    })
}

/// Install before the CLI, MCP server, or window resolves a game version. A new executable carries
/// a new payload and selects it; explicitly imported/native builds remain the player's choice.
pub fn ensure_installed() -> Result<Option<String>, String> {
    if !available() {
        return Ok(None);
    }
    let result = (|| {
        let candidate = version_tag(embedded::SHA256)?;
        let installed_before = complete(&backend::version_dir(&candidate), embedded::SHA256);
        let tag = install_at(
            &backend::version_dir(""),
            embedded::ARCHIVE,
            embedded::SHA256,
        )?;
        if !installed_before {
            backend::reuse_prepared_for(&tag);
        }
        let mod_tag = format!("{tag}{MODS_SUFFIX}");
        let mods_before = complete(&backend::version_dir(&mod_tag), embedded::SHA256);
        let mods = contains_mods(embedded::ARCHIVE)
            .then(|| {
                install_variant_at(
                    &backend::version_dir(""),
                    embedded::ARCHIVE,
                    embedded::SHA256,
                    true,
                )
            })
            .transpose()?;
        if !mods_before {
            if let Some(mods) = &mods {
                backend::reuse_prepared_for(mods);
            }
        }
        let mut settings = backend::Settings::load();
        let preferred = if settings.selected.as_deref().is_some_and(has_mods) {
            mods.as_ref().unwrap_or(&tag)
        } else {
            &tag
        };
        if settings.selected.as_deref().is_none_or(|selected| {
            is_bundled(selected)
                || (!crate::native_mods::is_native(selected)
                    && !crate::build_import::is_imported(selected))
        }) && settings.selected.as_deref() != Some(preferred)
        {
            settings.selected = Some(preferred.clone());
            settings.save()?;
        }
        if !installed_before || (mods.is_some() && !mods_before) {
            prune_old_payloads(&tag, &settings);
        }
        Ok(Some(preferred.clone()))
    })();
    result.map_err(|error: String| {
        i18n::trf(
            "Could not install the bundled Horizon game: {error}",
            &[("error", &error)],
        )
    })
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn complete(dir: &Path, sha256: &str) -> bool {
    fs::read_to_string(dir.join(MARKER)).is_ok_and(|saved| saved == sha256)
        && dir.join(".installed").is_file()
        && dir.join(VERSION_MARKER).is_file()
        && REQUIRED.iter().all(|name| dir.join(name).is_file())
}

fn io_error(path: &Path, error: std::io::Error) -> String {
    format!("{}: {error}", path.display())
}

fn prune_old_payloads(current: &str, settings: &backend::Settings) {
    if !settings.auto_prune() {
        return;
    }
    let versions = backend::version_dir("");
    let Ok(entries) = fs::read_dir(&versions) else {
        return;
    };
    let mut installed = Vec::new();
    for entry in entries.flatten() {
        let Some(tag) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(hash) = fs::read_to_string(entry.path().join(MARKER)) else {
            continue;
        };
        if version_tag(&hash).ok().as_deref() != Some(payload_group(&tag)) {
            continue;
        }
        if let Ok(time) =
            fs::metadata(entry.path().join(".installed")).and_then(|metadata| metadata.modified())
        {
            installed.push((tag, time));
        }
    }
    let mut protected: Vec<String> = settings.selected.iter().cloned().collect();
    protected.extend(
        crate::instances::list()
            .into_iter()
            .filter_map(|item| item.version),
    );
    for tag in stale_payloads(current, installed, settings.keep_newest(), &protected) {
        if backend::in_use(&tag) {
            continue;
        }
        if let Err(error) = backend::remove_version(&tag) {
            eprintln!("Horizon Offline: could not remove unused runtime {tag}: {error}");
        }
    }
}

fn stale_payloads(
    current: &str,
    mut installed: Vec<(String, std::time::SystemTime)>,
    keep: usize,
    protected: &[String],
) -> Vec<String> {
    installed.sort_by(|(left, left_time), (right, right_time)| {
        (payload_group(right) == payload_group(current))
            .cmp(&(payload_group(left) == payload_group(current)))
            .then_with(|| right_time.cmp(left_time))
            .then_with(|| left.cmp(right))
    });
    let mut groups = Vec::new();
    for (tag, _) in &installed {
        let group = payload_group(tag);
        if !groups.contains(&group) && groups.len() < keep.max(1) {
            groups.push(group);
        }
    }
    let kept: Vec<String> = groups.into_iter().map(str::to_owned).collect();
    installed
        .into_iter()
        .map(|(tag, _)| tag)
        .filter(|tag| {
            !kept.iter().any(|group| group == payload_group(tag)) && !protected.contains(tag)
        })
        .collect()
}

fn install_at(versions: &Path, archive: &[u8], sha256: &str) -> Result<String, String> {
    install_variant_at(versions, archive, sha256, false)
}

fn contains_mods(archive: &[u8]) -> bool {
    let Ok(mut zip) = zip::ZipArchive::new(Cursor::new(archive)) else {
        return false;
    };
    let entry = zip.by_name(MODS_PROGRAM);
    entry.is_ok_and(|entry| !entry.is_dir())
}

fn install_variant_at(
    versions: &Path,
    archive: &[u8],
    sha256: &str,
    mods: bool,
) -> Result<String, String> {
    // Both the tag and staging folder are generated here, never from ZIP names or user input.
    let mut tag = version_tag(sha256)?;
    if mods {
        tag.push_str(MODS_SUFFIX);
    }
    let target = versions.join(&tag);
    if complete(&target, sha256) {
        return Ok(tag);
    }
    if digest(archive) != sha256 {
        return Err("Embedded runtime SHA-256 mismatch".into());
    }
    fs::create_dir_all(versions).map_err(|error| io_error(versions, error))?;
    let stage = versions.join(format!(".{tag}-{}-partial", std::process::id()));
    fs::create_dir_all(&stage).map_err(|error| io_error(&stage, error))?;
    let result = (|| {
        unpack(archive, &stage, mods)?;
        if !REQUIRED.iter().all(|name| stage.join(name).is_file()) {
            return Err("The embedded runtime is incomplete".into());
        }
        fs::write(stage.join(MARKER), sha256).map_err(|error| io_error(&stage, error))?;
        fs::write(stage.join(VERSION_MARKER), VERSION).map_err(|error| io_error(&stage, error))?;
        fs::write(stage.join(".installed"), "").map_err(|error| io_error(&stage, error))?;
        // Preserve privately prepared resources when repairing an interrupted installation.
        // No player worlds or settings live in a version folder.
        if target.exists() {
            let _ = fs::remove_file(target.join(".installed"));
            for entry in fs::read_dir(&stage).map_err(|error| io_error(&stage, error))? {
                let entry = entry.map_err(|error| io_error(&stage, error))?;
                if entry.file_name() == ".installed" {
                    continue;
                }
                copy_tree(&entry.path(), &target.join(entry.file_name()))?;
            }
            fs::write(target.join(".installed"), "").map_err(|error| io_error(&target, error))?;
        } else if let Err(error) = fs::rename(&stage, &target) {
            // Another launcher may have published this same immutable payload first.
            if !complete(&target, sha256) {
                return Err(io_error(&target, error));
            }
        }
        Ok(tag.clone())
    })();
    // `stage` is a direct child made from a constant prefix, a digest, and our process id.
    if stage.parent() == Some(versions) && stage.exists() {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}

fn version_tag(sha256: &str) -> Result<String, String> {
    if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Invalid embedded SHA-256".into());
    }
    Ok(format!("{PREFIX}{}", &sha256[..16]))
}

fn unpack(bytes: &[u8], stage: &Path, mods: bool) -> Result<(), String> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|error| error.to_string())?;
    let mut total = 0u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let mut relative = entry
            .enclosed_name()
            .ok_or_else(|| "Invalid embedded runtime path".to_string())?;
        if relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
            || relative
                .components()
                .any(|part| part.as_os_str() == ".local")
            || entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
            || relative
                .extension()
                .is_some_and(|extension| extension == "jar" || extension == "class")
        {
            return Err("The embedded runtime contains a forbidden file".into());
        }
        total = total
            .checked_add(entry.size())
            .ok_or("Embedded runtime is too large")?;
        if entry.size() > MAX_FILE || total > MAX_TOTAL {
            return Err("Embedded runtime is too large".into());
        }
        if relative.starts_with("optional-mods") {
            if mods && relative == Path::new(MODS_PROGRAM) {
                relative = Path::new("horizon-client.exe").to_path_buf();
            } else {
                continue;
            }
        } else if mods && relative == Path::new("horizon-client.exe") {
            continue;
        }
        let path = stage.join(&relative);
        if entry.is_dir() {
            fs::create_dir_all(&path).map_err(|error| io_error(&path, error))?;
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
        }
        let mut file = fs::File::create(&path).map_err(|error| io_error(&path, error))?;
        let copied = std::io::copy(&mut entry.by_ref().take(MAX_FILE + 1), &mut file)
            .map_err(|error| io_error(&path, error))?;
        if copied > MAX_FILE {
            return Err("Embedded runtime is too large".into());
        }
    }
    Ok(())
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    if from.is_dir() {
        fs::create_dir_all(to).map_err(|error| io_error(to, error))?;
        for entry in fs::read_dir(from).map_err(|error| io_error(from, error))? {
            let entry = entry.map_err(|error| io_error(from, error))?;
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        }
    } else {
        fs::copy(from, to).map_err(|error| io_error(to, error))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn payload(extra: Option<&str>) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for name in REQUIRED.iter().copied().chain(extra) {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"original test fixture").unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    fn folder() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "horizon-offline-runtime-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn installs_once_and_preserves_private_preparation_when_repairing() {
        let root = folder();
        let archive = payload(None);
        let hash = digest(&archive);
        let tag = install_at(&root, &archive, &hash).unwrap();
        let version = root.join(&tag);
        let prepared = version.join(".local/horizon-client/26.3/private-data");
        fs::create_dir_all(prepared.parent().unwrap()).unwrap();
        fs::write(&prepared, b"private prepared test data").unwrap();
        assert_eq!(
            install_at(&root, b"not read on the fast path", &hash).unwrap(),
            tag
        );
        fs::remove_file(version.join("horizon-client.exe")).unwrap();
        install_at(&root, &archive, &hash).unwrap();
        assert!(version.join("horizon-client.exe").is_file());
        assert_eq!(fs::read(prepared).unwrap(), b"private prepared test data");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn optional_mods_install_as_a_separate_game_and_repair_without_losing_resources() {
        let root = folder();
        let archive = payload(Some(MODS_PROGRAM));
        assert!(contains_mods(&archive));
        assert!(!contains_mods(&payload(None)));
        let hash = digest(&archive);
        let vanilla = install_at(&root, &archive, &hash).unwrap();
        let modded = install_variant_at(&root, &archive, &hash, true).unwrap();
        assert_ne!(vanilla, modded);
        assert!(!root.join(&vanilla).join(MODS_PROGRAM).exists());
        for tag in [&vanilla, &modded] {
            assert!(complete(&root.join(tag), &hash));
            assert_eq!(
                fs::read_to_string(root.join(tag).join(VERSION_MARKER)).unwrap(),
                VERSION
            );
        }
        let prepared = root.join(&modded).join(".local/private-resources");
        fs::create_dir_all(prepared.parent().unwrap()).unwrap();
        fs::write(&prepared, b"keep prepared resources").unwrap();
        fs::remove_file(root.join(&modded).join("horizon-client.exe")).unwrap();
        install_variant_at(&root, &archive, &hash, true).unwrap();
        assert_eq!(fs::read(prepared).unwrap(), b"keep prepared resources");
        assert!(root.join(vanilla).join("horizon-client.exe").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_optional_game_uses_only_its_own_executable_regardless_of_zip_order() {
        let root = folder();
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        // The modded program comes first: a later vanilla entry must not overwrite it.
        for (name, contents) in std::iter::once((MODS_PROGRAM, b"modded game".as_slice())).chain(
            REQUIRED
                .iter()
                .map(|name| (*name, b"vanilla game".as_slice())),
        ) {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(contents).unwrap();
        }
        let archive = zip.finish().unwrap().into_inner();
        let hash = digest(&archive);
        let vanilla = install_at(&root, &archive, &hash).unwrap();
        let modded = install_variant_at(&root, &archive, &hash, true).unwrap();
        assert_eq!(
            fs::read(root.join(vanilla).join("horizon-client.exe")).unwrap(),
            b"vanilla game"
        );
        assert_eq!(
            fs::read(root.join(modded).join("horizon-client.exe")).unwrap(),
            b"modded game"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_corrupt_or_escaping_payload_without_publishing_a_version() {
        let root = folder();
        let archive = payload(None);
        assert!(install_at(&root, b"corrupt", &digest(&archive)).is_err());
        let forbidden = payload(Some("../escaped-file"));
        assert!(install_at(&root, &forbidden, &digest(&forbidden)).is_err());
        let minecraft = payload(Some("minecraft.jar"));
        assert!(install_at(&root, &minecraft, &digest(&minecraft)).is_err());
        assert!(fs::read_dir(&root).unwrap().all(|entry| !entry
            .unwrap()
            .path()
            .join(".installed")
            .exists()));
        assert!(!root.parent().unwrap().join("escaped-file").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cleanup_keeps_current_previous_and_explicitly_pinned_versions() {
        let installed = [
            ("current", 10),
            ("previous", 40),
            ("pinned", 30),
            ("old", 20),
        ]
        .into_iter()
        .map(|(tag, seconds)| {
            (
                tag.into(),
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds),
            )
        })
        .collect();
        assert_eq!(
            stale_payloads("current", installed, 2, &["pinned".into()]),
            ["old"]
        );
    }

    #[test]
    fn cleanup_retains_both_variants_of_each_kept_release() {
        let installed = [
            "current",
            "current-mods",
            "previous",
            "previous-mods",
            "old",
            "old-mods",
        ]
        .into_iter()
        .enumerate()
        .map(|(i, tag)| {
            (
                tag.into(),
                std::time::UNIX_EPOCH + std::time::Duration::from_secs((10 - i) as u64),
            )
        })
        .collect();
        let mut stale = stale_payloads("current-mods", installed, 2, &[]);
        stale.sort();
        assert_eq!(stale, ["old", "old-mods"]);
    }
}
