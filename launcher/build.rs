//! Embeds Horizon's icon (`packaging/icons/horizon.ico`) in the Windows executables, as icon
//! resource 1: Explorer and the task bar show it, and the window takes it as its icon.
fn main() {
    embed_offline_runtime();
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let ico = format!("{dir}/../packaging/icons/horizon.ico");
    println!("cargo:rerun-if-changed={ico}");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon(&ico);
        if let Err(e) = resource.compile() {
            panic!("embedding {ico}: {e}");
        }
    }
}

/// Embed only our runtime and preparer sources, staged by offline-release.yml.
fn embed_offline_runtime() {
    use sha2::{Digest, Sha256};
    use std::{env, fs, path::PathBuf};
    println!("cargo:rerun-if-env-changed=HORIZON_OFFLINE_RUNTIME_ARCHIVE");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));
    let archive = env::var_os("HORIZON_OFFLINE_RUNTIME_ARCHIVE")
        .filter(|_| env::var_os("CARGO_FEATURE_OFFLINE").is_some());
    let source = match archive {
        Some(path) => {
            let path = PathBuf::from(path);
            println!("cargo:rerun-if-changed={}", path.display());
            let bytes = fs::read(&path).expect("read the staged offline runtime");
            let digest = format!("{:x}", Sha256::digest(&bytes));
            fs::write(out.join("offline-runtime.zip"), bytes).expect("stage the offline runtime");
            format!(
                "pub const ARCHIVE: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/offline-runtime.zip\"));\npub const SHA256: &str = {digest:?};\n"
            )
        }
        None => "pub const ARCHIVE: &[u8] = &[];\npub const SHA256: &str = \"\";\n".into(),
    };
    fs::write(out.join("offline-runtime.rs"), source).expect("write offline runtime metadata");
}
