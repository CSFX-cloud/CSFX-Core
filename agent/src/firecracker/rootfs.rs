use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use oci_client::client::{ClientConfig, ClientProtocol};
use oci_client::config::ConfigFile;
use oci_client::secrets::RegistryAuth;
use oci_client::{Client, Reference};
use oci_spec::runtime::{
    get_default_namespaces, Capabilities, Capability, LinuxBuilder, LinuxCapabilitiesBuilder,
    LinuxNamespaceType, ProcessBuilder, RootBuilder, Spec, SpecBuilder, User, UserBuilder,
};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::process::Command;
use tracing::{debug, info};

use super::site_registry::{SiteRegistry, SITE_IMAGE_HOST};

const DOCKER_HUB_REGISTRY: &str = "docker.io";
const CONTAINER_BUNDLE_DIR: &str = "csfx-bundle";
const CONTAINER_ROOTFS_DIR: &str = "rootfs";

const ROOTFS_CACHE_DIR: &str = "/var/lib/csfx-agent/rootfs";
const ROOTFS_SIZE_MB: u64 = 2048;
const MAX_CONCURRENT_LAYER_DOWNLOADS: usize = 8;
const DIGEST_CACHE_TTL: Duration = Duration::from_secs(120);
const GUEST_INIT_BINARY_PATH: &str = "/var/lib/csfx-agent/csfx-guest-init";
const GZIP_LAYER_MEDIA_TYPES: &[&str] = &[
    "application/vnd.oci.image.layer.v1.tar+gzip",
    "application/vnd.docker.image.rootfs.diff.tar.gzip",
];
const WHITEOUT_PREFIX: &str = ".wh.";
const WHITEOUT_OPAQUE_MARKER: &str = ".wh..wh..opq";

fn build_lock(cache_key: &str) -> Arc<tokio::sync::Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> = OnceLock::new();
    let mut locks = LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    locks.entry(cache_key.to_string()).or_default().clone()
}

fn digest_cache() -> &'static Mutex<HashMap<String, (String, Instant)>> {
    static CACHE: OnceLock<Mutex<HashMap<String, (String, Instant)>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

fn cached_digest(reference: &str) -> Option<String> {
    let cache = digest_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (digest, stored_at) = cache.get(reference)?;
    (stored_at.elapsed() < DIGEST_CACHE_TTL).then(|| digest.clone())
}

fn store_digest(reference: &str, digest: &str) {
    digest_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(reference.to_string(), (digest.to_string(), Instant::now()));
}

fn resolve_reference(
    image: &str,
    mirror: Option<&str>,
    site_registry: Option<&SiteRegistry>,
) -> Result<Reference> {
    let reference: Reference = image.parse().context("Invalid image reference")?;
    if reference.registry() == SITE_IMAGE_HOST {
        let site_registry =
            site_registry.context("Site registry endpoint is not configured on this node")?;
        return site_registry.resolve(&reference);
    }
    let Some(mirror) = mirror else {
        return Ok(reference);
    };
    if reference.registry() != DOCKER_HUB_REGISTRY {
        return Ok(reference);
    }

    let tag_or_digest = match (reference.tag(), reference.digest()) {
        (_, Some(digest)) => format!("@{digest}"),
        (Some(tag), None) => format!(":{tag}"),
        (None, None) => String::new(),
    };
    let mirrored = format!("{mirror}/{}{tag_or_digest}", reference.repository());
    mirrored.parse().context("Invalid mirrored image reference")
}

fn auth_for(reference: &Reference, site_registry: Option<&SiteRegistry>) -> RegistryAuth {
    match site_registry {
        Some(site) if reference.registry() == site.endpoint() => site.auth(),
        _ => RegistryAuth::Anonymous,
    }
}

pub struct RootfsBuilder {
    client: Client,
    registry_mirror: Option<String>,
    site_registry: Option<SiteRegistry>,
}

impl RootfsBuilder {
    pub fn new(site_registry: Option<SiteRegistry>) -> Self {
        let registry_mirror = std::env::var("CSFX_REGISTRY_MIRROR")
            .ok()
            .filter(|value| !value.is_empty());

        let insecure_hosts: Vec<String> = registry_mirror
            .iter()
            .cloned()
            .chain(site_registry.iter().map(|site| site.endpoint().to_string()))
            .collect();
        let protocol = if insecure_hosts.is_empty() {
            ClientProtocol::Https
        } else {
            ClientProtocol::HttpsExcept(insecure_hosts)
        };

        Self {
            client: Client::new(ClientConfig {
                protocol,
                max_concurrent_download: MAX_CONCURRENT_LAYER_DOWNLOADS,
                ..Default::default()
            }),
            registry_mirror,
            site_registry,
        }
    }

    async fn resolve_digest(&self, reference: &Reference, auth: &RegistryAuth) -> Result<String> {
        if let Some(digest) = reference.digest() {
            return Ok(digest.to_string());
        }
        let key = reference.whole();
        if let Some(digest) = cached_digest(&key) {
            return Ok(digest);
        }
        let (_, digest) = self
            .client
            .pull_manifest(reference, auth)
            .await
            .context("Failed to resolve image manifest")?;
        store_digest(&key, &digest);
        Ok(digest)
    }

    pub async fn ensure_rootfs(&self, image: &str) -> Result<PathBuf> {
        let reference = resolve_reference(
            image,
            self.registry_mirror.as_deref(),
            self.site_registry.as_ref(),
        )?;
        let auth = auth_for(&reference, self.site_registry.as_ref());
        let digest = self.resolve_digest(&reference, &auth).await?;

        let guest_init_version = guest_init_binary_version().await?;
        let cache_key = format!(
            "{}-{}",
            digest.trim_start_matches("sha256:"),
            guest_init_version
        );
        let image_path = Path::new(ROOTFS_CACHE_DIR).join(format!("{}.ext4", cache_key));

        if image_path.exists() {
            info!(image = %image, digest = %digest, path = ?image_path, "Rootfs cache hit");
            return Ok(image_path);
        }

        let lock = build_lock(&cache_key);
        let _guard = lock.lock().await;
        if image_path.exists() {
            info!(image = %image, digest = %digest, path = ?image_path, "Rootfs cache hit after wait");
            return Ok(image_path);
        }

        info!(image = %image, digest = %digest, "Building rootfs image");

        debug!(image = %image, digest = %digest, stage = "pull_layers", "Pulling image layers");
        let image_data = self
            .client
            .pull(&reference, &auth, GZIP_LAYER_MEDIA_TYPES.to_vec())
            .await
            .context("Failed to pull image layers")?;
        debug!(
            image = %image,
            stage = "pull_layers",
            layer_count = image_data.layers.len(),
            "Pulled image layers"
        );

        let extract_dir = Path::new(ROOTFS_CACHE_DIR).join(format!("{}.extract", cache_key));
        if extract_dir.exists() {
            debug!(path = ?extract_dir, stage = "extract_prepare", "Removing stale extraction directory");
            tokio::fs::remove_dir_all(&extract_dir)
                .await
                .context("Failed to clean up stale rootfs extraction directory")?;
        }
        tokio::fs::create_dir_all(&extract_dir)
            .await
            .context("Failed to create rootfs extraction directory")?;

        let build_result = self.build_extracted_rootfs(&image_data, &extract_dir).await;

        if build_result.is_err() {
            debug!(path = ?extract_dir, stage = "extract_cleanup", "Removing extraction directory after failure");
            tokio::fs::remove_dir_all(&extract_dir)
                .await
                .context("Failed to clean up rootfs extraction directory after failure")?;
        }
        build_result?;

        let tmp_image_path = Path::new(ROOTFS_CACHE_DIR).join(format!("{}.ext4.tmp", cache_key));
        debug!(path = ?tmp_image_path, stage = "build_ext4", "Building ext4 image");
        build_ext4_image(&extract_dir, &tmp_image_path).await?;

        tokio::fs::remove_dir_all(&extract_dir)
            .await
            .context("Failed to clean up rootfs extraction directory")?;

        tokio::fs::rename(&tmp_image_path, &image_path)
            .await
            .context("Failed to finalize rootfs image")?;

        info!(image = %image, path = ?image_path, "Rootfs image ready");

        Ok(image_path)
    }

    async fn build_extracted_rootfs(
        &self,
        image_data: &oci_client::client::ImageData,
        extract_dir: &Path,
    ) -> Result<()> {
        let bundle_dir = extract_dir.join(CONTAINER_BUNDLE_DIR);
        let rootfs_dir = bundle_dir.join(CONTAINER_ROOTFS_DIR);
        tokio::fs::create_dir_all(&rootfs_dir)
            .await
            .context("Failed to create container rootfs directory")?;

        for (index, layer) in image_data.layers.iter().enumerate() {
            debug!(
                stage = "extract_layer",
                layer_index = index,
                layer_size = layer.data.len(),
                media_type = %layer.media_type,
                "Extracting image layer"
            );
            extract_layer(&layer.data, &rootfs_dir)
                .await
                .context("Failed to extract image layer")?;
        }

        debug!(
            stage = "write_runtime_config",
            "Writing OCI runtime config.json"
        );
        write_runtime_config(&image_data.config.data, &bundle_dir).await?;

        debug!(stage = "install_guest_init", "Installing guest-init binary");
        install_guest_init(extract_dir).await?;

        debug!(
            stage = "create_root_dirs",
            "Creating standard guest root directories"
        );
        create_guest_root_dirs(extract_dir).await?;

        Ok(())
    }
}

async fn extract_layer(gzip_data: &[u8], dest: &Path) -> Result<()> {
    let dest = dest.to_path_buf();
    let gzip_data = gzip_data.to_vec();

    tokio::task::spawn_blocking(move || extract_layer_blocking(&gzip_data, &dest))
        .await
        .context("Layer extraction task panicked")?
}

fn extract_layer_blocking(gzip_data: &[u8], dest: &Path) -> Result<()> {
    let decoder = GzDecoder::new(gzip_data);
    let mut archive = tar::Archive::new(decoder);
    archive.set_preserve_ownerships(true);
    archive.set_preserve_permissions(true);

    let entries = archive.entries().context("Failed to read layer entries")?;
    for entry in entries {
        let mut entry = entry.context("Failed to read layer entry")?;
        let path = entry.path().context("Invalid entry path")?.into_owned();

        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name == WHITEOUT_OPAQUE_MARKER {
                clear_opaque_directory(dest, &path)?;
                continue;
            }
            if let Some(deleted) = name.strip_prefix(WHITEOUT_PREFIX) {
                apply_whiteout(dest, &path, deleted)?;
                continue;
            }
        }

        let unpacked = entry
            .unpack_in(dest)
            .with_context(|| format!("Failed to unpack {:?}", path))?;

        if !unpacked {
            anyhow::bail!("Refused to unpack unsafe layer entry path {:?}", path);
        }
    }

    Ok(())
}

fn resolve_directory_within(dest: &Path, relative: &Path) -> Result<PathBuf> {
    let mut resolved = dest.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            if component == Component::CurDir {
                continue;
            }
            anyhow::bail!("Refused layer whiteout path {:?}", relative);
        };
        resolved.push(name);
        match std::fs::symlink_metadata(&resolved) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                anyhow::bail!("Refused layer whiteout path through symlink {:?}", relative)
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Failed to inspect whiteout path {:?}", resolved))
            }
        }
    }
    Ok(resolved)
}

fn clear_opaque_directory(dest: &Path, whiteout_path: &Path) -> Result<()> {
    let Some(parent) = whiteout_path.parent() else {
        return Ok(());
    };
    let target_dir = resolve_directory_within(dest, parent)?;
    if target_dir.is_dir() {
        std::fs::remove_dir_all(&target_dir)
            .with_context(|| format!("Failed to clear opaque directory {:?}", target_dir))?;
        std::fs::create_dir_all(&target_dir)
            .with_context(|| format!("Failed to recreate opaque directory {:?}", target_dir))?;
    }
    Ok(())
}

fn apply_whiteout(dest: &Path, whiteout_path: &Path, deleted_name: &str) -> Result<()> {
    let mut deleted = Path::new(deleted_name).components();
    let (Some(Component::Normal(name)), None) = (deleted.next(), deleted.next()) else {
        anyhow::bail!("Refused layer whiteout name {:?}", deleted_name);
    };
    let parent = whiteout_path.parent().unwrap_or_else(|| Path::new(""));
    let target = resolve_directory_within(dest, parent)?.join(name);

    let Ok(metadata) = std::fs::symlink_metadata(&target) else {
        return Ok(());
    };
    if metadata.is_dir() {
        let _ = std::fs::remove_dir_all(&target);
    } else {
        let _ = std::fs::remove_file(&target);
    }

    Ok(())
}

async fn write_runtime_config(config_data: &[u8], extract_dir: &Path) -> Result<()> {
    let config_file: ConfigFile =
        serde_json::from_slice(config_data).context("Failed to parse image config")?;
    let config = config_file.config.unwrap_or_default();

    let spec = build_runtime_spec(&config)?;
    let config_path = extract_dir.join("config.json");

    tokio::task::spawn_blocking(move || spec.save(&config_path))
        .await
        .context("Runtime spec save task panicked")?
        .context("Failed to write runtime config.json")
}

fn build_runtime_spec(config: &oci_client::config::Config) -> Result<Spec> {
    let mut argv: Vec<String> = Vec::new();
    argv.extend(config.entrypoint.clone().unwrap_or_default());
    argv.extend(config.cmd.clone().unwrap_or_default());
    if argv.is_empty() {
        argv.push("sh".to_string());
    }

    let mut env = config.env.clone().unwrap_or_default();
    if !env.iter().any(|entry| entry.starts_with("PATH=")) {
        env.push("PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string());
    }

    let cwd = config
        .working_dir
        .clone()
        .filter(|dir| !dir.is_empty())
        .unwrap_or_else(|| "/".to_string());

    let user = parse_oci_user(config.user.as_deref());
    debug!(
        stage = "build_runtime_spec",
        image_user = ?config.user,
        resolved_uid = user.uid(),
        resolved_gid = user.gid(),
        argv = ?argv,
        cwd = %cwd,
        "Resolved container process identity"
    );

    let capabilities = LinuxCapabilitiesBuilder::default()
        .bounding(default_container_capabilities())
        .effective(default_container_capabilities())
        .permitted(default_container_capabilities())
        .build()
        .context("Failed to build runtime capabilities spec")?;

    let mut process = ProcessBuilder::default()
        .terminal(false)
        .user(user)
        .args(argv)
        .env(env)
        .cwd(cwd)
        .capabilities(capabilities)
        .build()
        .context("Failed to build runtime process spec")?;
    process.set_no_new_privileges(None);

    let root = RootBuilder::default()
        .path(CONTAINER_ROOTFS_DIR)
        .readonly(false)
        .build()
        .context("Failed to build runtime root spec")?;

    let namespaces: Vec<_> = get_default_namespaces()
        .into_iter()
        .filter(|ns| ns.typ() != LinuxNamespaceType::Network)
        .collect();

    let linux = LinuxBuilder::default()
        .namespaces(namespaces)
        .build()
        .context("Failed to build runtime linux spec")?;

    SpecBuilder::default()
        .process(process)
        .root(root)
        .linux(linux)
        .hostname("csfx".to_string())
        .build()
        .context("Failed to build runtime spec")
}

fn default_container_capabilities() -> Capabilities {
    [
        Capability::AuditWrite,
        Capability::Chown,
        Capability::DacOverride,
        Capability::Fowner,
        Capability::Fsetid,
        Capability::Kill,
        Capability::Mknod,
        Capability::NetBindService,
        Capability::NetRaw,
        Capability::Setfcap,
        Capability::Setgid,
        Capability::Setpcap,
        Capability::Setuid,
        Capability::SysChroot,
    ]
    .into_iter()
    .collect()
}

fn parse_oci_user(user: Option<&str>) -> User {
    let Some(user) = user else {
        return UserBuilder::default().uid(0u32).gid(0u32).build().unwrap();
    };

    let (uid_part, gid_part) = match user.split_once(':') {
        Some((uid, gid)) => (uid, Some(gid)),
        None => (user, None),
    };

    let uid: u32 = uid_part.parse().unwrap_or(0);
    let gid: u32 = gid_part.and_then(|g| g.parse().ok()).unwrap_or(0);

    UserBuilder::default().uid(uid).gid(gid).build().unwrap()
}

async fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = tokio::fs::metadata(path).await?.permissions();
    permissions.set_mode(0o755);
    tokio::fs::set_permissions(path, permissions).await?;

    Ok(())
}

async fn guest_init_binary_version() -> Result<String> {
    match tokio::fs::read_link(GUEST_INIT_BINARY_PATH).await {
        Ok(target) => Ok(sanitize_cache_key_component(&target.to_string_lossy())),
        Err(_) => {
            let metadata = tokio::fs::metadata(GUEST_INIT_BINARY_PATH)
                .await
                .context("Failed to stat guest-init binary")?;
            let modified = metadata
                .modified()
                .context("Failed to read guest-init binary mtime")?;
            let since_epoch = modified
                .duration_since(std::time::UNIX_EPOCH)
                .context("Guest-init binary mtime before unix epoch")?;
            Ok(since_epoch.as_secs().to_string())
        }
    }
}

fn sanitize_cache_key_component(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

async fn install_guest_init(extract_dir: &Path) -> Result<()> {
    let dest = extract_dir.join("sbin").join("csfx-guest-init");
    tokio::fs::create_dir_all(extract_dir.join("sbin"))
        .await
        .context("Failed to create sbin directory")?;

    tokio::fs::copy(GUEST_INIT_BINARY_PATH, &dest)
        .await
        .context("Failed to copy guest-init binary into rootfs")?;

    set_executable(&dest)
        .await
        .context("Failed to make guest-init binary executable")?;

    Ok(())
}

async fn create_guest_root_dirs(extract_dir: &Path) -> Result<()> {
    for dir in ["dev", "proc", "sys", "etc"] {
        tokio::fs::create_dir_all(extract_dir.join(dir))
            .await
            .with_context(|| format!("Failed to create guest root directory {}", dir))?;
    }
    Ok(())
}

async fn build_ext4_image(source_dir: &Path, image_path: &Path) -> Result<()> {
    let size_arg = format!("{}M", ROOTFS_SIZE_MB);

    let status = Command::new("truncate")
        .args(["-s", &size_arg])
        .arg(image_path)
        .status()
        .await
        .context("Failed to execute truncate")?;

    if !status.success() {
        anyhow::bail!("truncate failed for rootfs image {:?}", image_path);
    }

    let status = Command::new("mkfs.ext4")
        .args([
            "-F",
            "-E",
            "lazy_itable_init=1,lazy_journal_init=1,nodiscard",
            "-d",
        ])
        .arg(source_dir)
        .arg(image_path)
        .status()
        .await
        .context("Failed to execute mkfs.ext4")?;

    if !status.success() {
        anyhow::bail!("mkfs.ext4 failed for rootfs image {:?}", image_path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn docker_hub_images_are_routed_through_the_mirror() {
        let reference = resolve_reference("nginx:1.27", Some("localhost:5050"), None).unwrap();
        assert_eq!(reference.registry(), "localhost:5050");
        assert_eq!(reference.repository(), "library/nginx");
        assert_eq!(reference.tag(), Some("1.27"));
    }

    #[test]
    fn site_images_resolve_to_the_site_registry_even_with_a_mirror() {
        let site = SiteRegistry::from_gateway("https://10.0.0.5:8000", "key").unwrap();
        let image = "sites.csfx.invalid/abc:12345678";
        let reference = resolve_reference(image, Some("localhost:5050"), Some(&site)).unwrap();
        assert_eq!(reference.whole(), "10.0.0.5:5060/abc:12345678");
        assert!(matches!(
            auth_for(&reference, Some(&site)),
            RegistryAuth::Basic(user, _) if user == "agent"
        ));
        assert!(resolve_reference(image, None, None).is_err());
    }

    #[test]
    fn credentials_are_never_sent_to_other_registries() {
        let site = SiteRegistry::from_gateway("https://10.0.0.5:8000", "key").unwrap();
        let other: Reference = "ghcr.io/org/app:1".parse().unwrap();
        assert!(matches!(
            auth_for(&other, Some(&site)),
            RegistryAuth::Anonymous
        ));
    }

    #[test]
    fn other_registries_bypass_the_mirror() {
        let image = "ghcr.io/csfx-cloud/csfx-site-builder:1";
        let reference = resolve_reference(image, Some("localhost:5050"), None).unwrap();
        assert_eq!(reference.registry(), "ghcr.io");
        assert_eq!(reference.whole(), image);
    }

    fn layer_header(path: &str, entry_type: tar::EntryType, size: u64, mode: u32) -> tar::Header {
        let mut header = tar::Header::new_gnu();
        header.set_path(path).unwrap();
        header.set_entry_type(entry_type);
        header.set_size(size);
        header.set_mode(mode);
        header.set_uid(u64::from(unsafe { libc::geteuid() }));
        header.set_gid(u64::from(unsafe { libc::getegid() }));
        header.set_mtime(0);
        header.set_cksum();
        header
    }

    fn build_gzip_layer(entries: &[(&str, &[u8], bool)]) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        for (path, contents, is_dir) in entries {
            if *is_dir {
                let header = layer_header(path, tar::EntryType::Directory, 0, 0o755);
                builder.append(&header, std::io::empty()).unwrap();
            } else {
                let header =
                    layer_header(path, tar::EntryType::Regular, contents.len() as u64, 0o644);
                builder.append(&header, *contents).unwrap();
            }
        }
        let tar_bytes = builder.into_inner().unwrap();

        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(&tar_bytes).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn whiteout_removes_earlier_layer_file() {
        let tmp = std::env::temp_dir().join(format!("csfx-rootfs-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(tmp.join("a")).unwrap();

        let layer1 = build_gzip_layer(&[("a/b.txt", b"hello", false)]);
        let layer2 = build_gzip_layer(&[("a/.wh.b.txt", b"", false)]);

        extract_layer_blocking(&layer1, &tmp).unwrap();
        assert!(tmp.join("a/b.txt").exists());

        extract_layer_blocking(&layer2, &tmp).unwrap();
        assert!(!tmp.join("a/b.txt").exists());

        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn oci_users_resolve_numeric_ids() {
        assert_eq!(parse_oci_user(None).uid(), 0);
        assert_eq!(parse_oci_user(Some("1000")).uid(), 1000);
        assert_eq!(parse_oci_user(Some("1000")).gid(), 0);
        let both = parse_oci_user(Some("1000:2000"));
        assert_eq!((both.uid(), both.gid()), (1000, 2000));
        assert_eq!(parse_oci_user(Some("nginx")).uid(), 0);
    }

    #[test]
    fn cache_key_components_are_sanitised() {
        assert_eq!(sanitize_cache_key_component("a/b:c.d"), "a-b-c-d");
        assert_eq!(sanitize_cache_key_component("abc123"), "abc123");
    }

    #[test]
    fn runtime_spec_uses_entrypoint_cmd_and_defaults() {
        let config = oci_client::config::Config {
            entrypoint: Some(vec!["/entry".to_string()]),
            cmd: Some(vec!["--flag".to_string()]),
            user: Some("5:6".to_string()),
            working_dir: Some("/srv".to_string()),
            ..Default::default()
        };
        let spec = build_runtime_spec(&config).unwrap();
        let process = spec.process().as_ref().unwrap();
        assert_eq!(
            process.args().as_ref().unwrap(),
            &vec!["/entry".to_string(), "--flag".to_string()]
        );
        assert_eq!(process.cwd().to_str(), Some("/srv"));
        assert_eq!(process.user().uid(), 5);
        assert!(process
            .env()
            .as_ref()
            .unwrap()
            .iter()
            .any(|entry| entry.starts_with("PATH=")));
        assert_eq!(spec.hostname().as_deref(), Some("csfx"));
    }

    #[test]
    fn empty_image_config_falls_back_to_a_shell_in_root() {
        let config = oci_client::config::Config {
            env: Some(vec!["PATH=/custom".to_string()]),
            working_dir: Some(String::new()),
            ..Default::default()
        };
        let spec = build_runtime_spec(&config).unwrap();
        let process = spec.process().as_ref().unwrap();
        assert_eq!(process.args().as_ref().unwrap(), &vec!["sh".to_string()]);
        assert_eq!(process.cwd().to_str(), Some("/"));
        assert_eq!(
            process.env().as_ref().unwrap(),
            &vec!["PATH=/custom".to_string()]
        );
        assert!(default_container_capabilities().contains(&Capability::NetBindService));
    }

    #[test]
    fn opaque_whiteouts_clear_the_directory() {
        let tmp = std::env::temp_dir().join(format!("csfx-rootfs-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(tmp.join("a/sub")).unwrap();
        std::fs::write(tmp.join("a/old.txt"), b"x").unwrap();

        let layer = build_gzip_layer(&[("a/.wh..wh..opq", b"", false), ("a/new.txt", b"y", false)]);
        extract_layer_blocking(&layer, &tmp).unwrap();

        assert!(!tmp.join("a/old.txt").exists());
        assert!(!tmp.join("a/sub").exists());
        assert!(tmp.join("a/new.txt").exists());
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn whiteouts_also_remove_directories() {
        let tmp = std::env::temp_dir().join(format!("csfx-rootfs-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(tmp.join("a/dir")).unwrap();
        apply_whiteout(&tmp, Path::new("a/.wh.dir"), "dir").unwrap();
        assert!(!tmp.join("a/dir").exists());
        clear_opaque_directory(&tmp, Path::new("missing/.wh..wh..opq")).unwrap();
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn unsafe_layer_entries_are_refused() {
        let tmp = std::env::temp_dir().join(format!("csfx-rootfs-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        assert!(extract_layer_blocking(b"not gzip", &tmp).is_err());
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[tokio::test]
    async fn layers_extract_asynchronously_and_config_is_written() {
        let tmp = std::env::temp_dir().join(format!("csfx-rootfs-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        let layer = build_gzip_layer(&[("d", b"", true), ("d/f", b"z", false)]);
        extract_layer(&layer, &tmp).await.unwrap();
        assert_eq!(std::fs::read(tmp.join("d/f")).unwrap(), b"z");

        let config = br#"{"architecture":"amd64","os":"linux","rootfs":{"type":"layers","diff_ids":[]},"config":{"Cmd":["run"]}}"#;
        write_runtime_config(config, &tmp).await.unwrap();
        assert!(tmp.join("config.json").exists());
        assert!(write_runtime_config(b"{", &tmp).await.is_err());
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn digests_are_cached_per_reference() {
        assert!(cached_digest("ref-never-stored").is_none());
        store_digest("ref-stored", "sha256:abc");
        assert_eq!(cached_digest("ref-stored").as_deref(), Some("sha256:abc"));
        assert!(Arc::ptr_eq(&build_lock("k"), &build_lock("k")));
        assert!(!Arc::ptr_eq(&build_lock("k"), &build_lock("other")));
    }
}
