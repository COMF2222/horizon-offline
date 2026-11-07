//! Updates of the offline distribution, whose executable also carries its matching game.
//! Only our release channel and dedicated publishing keys can authorize replacements.
use crate::backend::{self, Release};
use horizon_release_sign::{PublicKey, SIGNATURE, SUMS};
use semver::Version;

pub const REPO: &str = "COMF2222/horizon-offline";
pub const TAG_PREFIX: &str = "offline-v";
pub const ASSET: &str = "horizon-offline-windows-x64.zip";
const KEYS: &str = include_str!("../../packaging/offline/release-keys.txt");

pub fn trusted_keys() -> Vec<PublicKey> {
    horizon_release_sign::parse_keys(KEYS).unwrap_or_default()
}

/// No unsigned, unsupported-platform, or developer distribution replaces itself.
pub fn configured() -> bool {
    cfg!(all(
        feature = "offline",
        target_os = "windows",
        target_arch = "x86_64"
    )) && stable_version(crate::offline_runtime::VERSION).is_some()
        && !trusted_keys().is_empty()
}

fn stable_version(version: &str) -> Option<Version> {
    Version::parse(version)
        .ok()
        .filter(|version| version.pre.is_empty() && version.build.is_empty())
}

pub fn tag_version(tag: &str) -> Option<&str> {
    let version = tag.strip_prefix(TAG_PREFIX)?;
    stable_version(version).map(|_| version)
}

pub fn asset_url(tag: &str, name: &str) -> String {
    format!("https://github.com/{REPO}/releases/download/{tag}/{name}")
}

/// Choose by distribution semver, never publication time or the upstream launcher's number.
pub fn newer_release(list: &serde_json::Value, current: &str) -> Option<Release> {
    let current = stable_version(current)?;
    list.as_array()?
        .iter()
        .filter_map(|release| {
            if release["draft"].as_bool().unwrap_or(true)
                || release["prerelease"].as_bool().unwrap_or(true)
            {
                return None;
            }
            let tag = release["tag_name"].as_str()?;
            let version = stable_version(tag_version(tag)?)?;
            if version <= current {
                return None;
            }
            let expected = asset_url(tag, ASSET);
            let asset = release["assets"].as_array()?.iter().find(|asset| {
                asset["name"].as_str() == Some(ASSET)
                    && asset["browser_download_url"].as_str() == Some(expected.as_str())
                    && asset["state"].as_str() == Some("uploaded")
            })?;
            let published = release["published_at"].as_str().unwrap_or_default();
            Some((
                version,
                Release {
                    tag: tag.into(),
                    name: release["name"].as_str().unwrap_or(tag).into(),
                    body: release["body"].as_str().unwrap_or_default().into(),
                    date: published.get(..10).unwrap_or_default().into(),
                    time: published.get(11..16).unwrap_or_default().into(),
                    asset: Some((expected, asset["size"].as_u64().unwrap_or(0))),
                    prerelease: false,
                    nightly: None,
                },
            ))
        })
        .max_by(|(left, _), (right, _)| left.cmp(right))
        .map(|(_, release)| release)
}

/// Uses the shared ETag/disk cache; cached older metadata never offers a downgrade.
pub fn check() -> Result<Option<Release>, String> {
    if !configured() {
        return Ok(None);
    }
    let list = crate::github::json(&format!(
        "https://api.github.com/repos/{REPO}/releases?per_page=100"
    ))?;
    Ok(newer_release(&list, crate::offline_runtime::VERSION))
}

/// Signed metadata is fetched only from this release's fixed GitHub location.
pub fn digest(release: &Release) -> Result<String, String> {
    digest_with(release, &trusted_keys(), backend::fetch_small)
}

fn digest_with(
    release: &Release,
    keys: &[PublicKey],
    fetch: impl Fn(&str) -> Result<Option<Vec<u8>>, String>,
) -> Result<String, String> {
    if tag_version(&release.tag).is_none()
        || release.nightly.is_some()
        || release.prerelease
        || release.asset.as_ref().map(|(url, _)| url.as_str())
            != Some(asset_url(&release.tag, ASSET).as_str())
    {
        return Err(crate::i18n::tr("This update is not from Horizon Offline.").to_string());
    }
    let sums = fetch(&asset_url(&release.tag, SUMS))?;
    let signature = fetch(&asset_url(&release.tag, SIGNATURE))?
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
    horizon_release_sign::signed_digest(
        &release.tag,
        ASSET,
        sums.as_deref(),
        signature.as_deref(),
        keys,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_release_sign::{sha256_hex, sums_file, SigningKey};
    use serde_json::{json, Value};

    fn release(tag: &str) -> Value {
        json!({
            "tag_name": tag, "name": tag, "body": "", "draft": false,
            "prerelease": false, "published_at": "2099-01-01T10:20:30Z",
            "assets": [{"name": ASSET, "browser_download_url": asset_url(tag, ASSET),
                        "state": "uploaded", "size": 100}]
        })
    }

    #[test]
    fn updates_follow_our_semver_even_when_metadata_arrives_out_of_order() {
        let list = json!([
            release("offline-v0.2.1"),
            release("offline-v0.3.0"),
            release("offline-v0.2.5")
        ]);
        assert_eq!(newer_release(&list, "0.2.0").unwrap().tag, "offline-v0.3.0");
        assert!(newer_release(&list, "0.3.0").is_none());
        assert!(newer_release(&list, "1.0.0").is_none());
        assert!(newer_release(&list, "dev").is_none());
    }

    #[test]
    fn official_draft_prerelease_and_foreign_downloads_are_not_our_updates() {
        let mut draft = release("offline-v9.0.0");
        draft["draft"] = json!(true);
        let mut prerelease = release("offline-v8.0.0");
        prerelease["prerelease"] = json!(true);
        let mut foreign = release("offline-v7.0.0");
        foreign["assets"][0]["browser_download_url"] = json!("https://example.org/launcher.zip");
        let mut missing = release("offline-v6.0.0");
        missing["assets"][0]["state"] = json!("new");
        let list = json!([
            release("launcher@99.0.0"),
            draft,
            prerelease,
            foreign,
            missing,
            release("offline-v5.0.0-rc.1"),
            release("offline-v4.0.0+extra")
        ]);
        assert!(newer_release(&list, "0.2.0").is_none());
    }

    #[test]
    fn dedicated_publisher_key_and_exact_tag_must_authorize_the_archive() {
        let (key, _) = SigningKey::generate().unwrap();
        let (other, _) = SigningKey::generate().unwrap();
        let sums = sums_file(&[(ASSET.into(), sha256_hex(b"offline launcher"))]);
        let signature = key.sign("offline-v0.3.0", sums.as_bytes());
        let fetch = |url: &str| {
            assert!(
                url.starts_with("https://github.com/COMF2222/horizon-offline/releases/download/")
            );
            Ok(Some(if url.ends_with(SIGNATURE) {
                signature.as_bytes().to_vec()
            } else {
                sums.as_bytes().to_vec()
            }))
        };
        let offered = newer_release(&json!([release("offline-v0.3.0")]), "0.2.0").unwrap();
        let hash = digest_with(&offered, &[key.public()], fetch).unwrap();
        assert_eq!(hash, sha256_hex(b"offline launcher"));
        assert!(
            horizon_release_sign::check(&offered.tag, &hash, &sha256_hex(b"tampered")).is_err()
        );
        assert!(digest_with(&offered, &[other.public()], fetch).is_err());
        let replayed = newer_release(&json!([release("offline-v0.4.0")]), "0.2.0").unwrap();
        assert!(digest_with(&replayed, &[key.public()], fetch).is_err());
        assert!(digest_with(&offered, &[key.public()], |_| Ok(None)).is_err());
    }

    #[test]
    fn forged_source_is_refused_before_any_network_request() {
        let mut offered = newer_release(&json!([release("offline-v0.3.0")]), "0.2.0").unwrap();
        offered.asset = Some(("https://example.org/malware.zip".into(), 0));
        assert!(digest_with(&offered, &[], |_| panic!(
            "foreign source must never be fetched"
        ))
        .is_err());
    }
}
