use anyhow::{Context, Result};
use libcontainer::container::builder::ContainerBuilder;
use libcontainer::syscall::syscall::SyscallType;
use nix::mount::{MsFlags, mount};
use nix::pty::openpty;
use nix::sys::reboot::{RebootMode, reboot};
use nix::sys::wait::{WaitStatus, waitpid};
use nix::unistd::Pid;
use serde::Deserialize;
use std::os::fd::{AsRawFd, OwnedFd};
use tokio::io::unix::AsyncFd;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{Duration, timeout};
use tokio_vsock::{VMADDR_CID_ANY, VsockAddr, VsockListener};
use tracing::{debug, error, info, warn};

const LOG_PORT: u32 = 10001;
const EXEC_PORT: u32 = 10002;
const LOG_BACKLOG_CAPACITY: usize = 65536;
const CONTAINER_BUNDLE_PATH: &str = "/csfx-bundle";
const CONTAINER_ROOTFS_DIR: &str = "rootfs";
const CONTAINER_STATE_ROOT: &str = "/run/csfx";
const CONTAINER_ID: &str = "workload";
const MMDS_ADDR: &str = "169.254.169.254:80";
const MMDS_BOOTSTRAP_IP: std::net::Ipv4Addr = std::net::Ipv4Addr::new(169, 254, 169, 2);
const MMDS_BOOTSTRAP_NETMASK: std::net::Ipv4Addr = std::net::Ipv4Addr::new(255, 255, 0, 0);
const MMDS_TIMEOUT: Duration = Duration::from_secs(5);
const RESOLV_CONF_PATH: &str = "/etc/resolv.conf";

#[derive(Debug, Deserialize)]
struct MmdsVolume {
    device: String,
    mount_path: String,
}

#[derive(Debug, Deserialize)]
struct MmdsNetwork {
    ip: String,
    prefix: String,
    gateway: Option<String>,
    dns: Option<String>,
    search: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct MmdsData {
    #[serde(default)]
    volumes: Vec<MmdsVolume>,
    network: Option<MmdsNetwork>,
    #[serde(default)]
    env: std::collections::HashMap<String, String>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    info!("csfx-guest-init starting");

    if let Err(e) = run().await {
        error!(error = ?e, "csfx-guest-init failed");
    }

    power_off();
}

async fn run() -> Result<()> {
    debug!(
        stage = "mount_pseudo_filesystems",
        "Mounting pseudo filesystems"
    );
    mount_pseudo_filesystems();

    debug!(
        stage = "bring_up_interface",
        iface = "eth0",
        "Bringing up network interface"
    );
    bring_up_interface("eth0").context("Failed to bring up network interface")?;

    debug!(
        stage = "assign_bootstrap_address",
        iface = "eth0",
        "Assigning mmds bootstrap address"
    );
    assign_bootstrap_address("eth0").context("Failed to assign mmds bootstrap address")?;

    add_host_route(std::net::Ipv4Addr::new(169, 254, 169, 254), "eth0")
        .context("Failed to add mmds route")?;
    info!("mmds route added");

    debug!(stage = "fetch_mmds_data", "Fetching mmds data");
    let mmds_data = fetch_mmds_data()
        .await
        .context("Failed to fetch mmds data")?;
    debug!(
        stage = "mmds_data_fetched",
        has_network = mmds_data.network.is_some(),
        volume_count = mmds_data.volumes.len(),
        env_count = mmds_data.env.len(),
        "mmds data fetched"
    );

    if let Some(network) = &mmds_data.network {
        debug!(stage = "configure_network", ip = %network.ip, prefix = %network.prefix, dns = ?network.dns, "Configuring guest network");
        configure_network("eth0", network).context("Failed to configure guest network")?;
    }

    for volume in &mmds_data.volumes {
        debug!(stage = "mount_volume", device = %volume.device, mount_path = %volume.mount_path, "Mounting volume");
        mount_volume(volume).with_context(|| {
            format!(
                "Failed to mount volume device={} mount_path={}",
                volume.device, volume.mount_path
            )
        })?;
    }

    let log_listener = VsockListener::bind(VsockAddr::new(VMADDR_CID_ANY, LOG_PORT))
        .context("Failed to bind vsock log port")?;
    let exec_listener = VsockListener::bind(VsockAddr::new(VMADDR_CID_ANY, EXEC_PORT))
        .context("Failed to bind vsock exec port")?;
    debug!(
        stage = "vsock_listeners_bound",
        log_port = LOG_PORT,
        exec_port = EXEC_PORT,
        "vsock listeners bound"
    );

    debug!(
        stage = "start_container",
        "Starting container via libcontainer"
    );
    let (stdout_read, stderr_read, container_pid, _stdout_write, _stderr_write) =
        start_container(&mmds_data.env).await?;
    debug!(
        stage = "container_started",
        container_pid = container_pid.as_raw(),
        "Container process started"
    );

    let log_task = tokio::spawn(stream_logs(log_listener, stdout_read, stderr_read));
    tokio::spawn(serve_exec(exec_listener));

    let exit_status = tokio::task::spawn_blocking(move || {
        loop {
            match waitpid(container_pid, None) {
                Ok(WaitStatus::Exited(pid, code)) if pid == container_pid => {
                    return Ok(WaitStatus::Exited(pid, code));
                }
                Ok(WaitStatus::Signaled(pid, sig, core)) if pid == container_pid => {
                    return Ok(WaitStatus::Signaled(pid, sig, core));
                }
                Ok(_) => continue,
                Err(nix::errno::Errno::EINTR) => continue,
                Err(e) => return Err(e),
            }
        }
    })
    .await
    .context("Container wait task panicked")?
    .context("Failed to wait on container process")?;
    info!(status = ?exit_status, "entrypoint exited");

    let _ = log_task.await;

    if !matches!(exit_status, WaitStatus::Exited(_, 0)) {
        dump_failure_diagnostics();
    }

    Ok(())
}

fn apply_extra_env(extra_env: &std::collections::HashMap<String, String>) -> Result<()> {
    if extra_env.is_empty() {
        return Ok(());
    }

    let config_path = std::path::Path::new(CONTAINER_BUNDLE_PATH).join("config.json");
    let mut spec = oci_spec::runtime::Spec::load(&config_path)
        .context("Failed to load runtime config.json")?;

    let process = spec
        .process_mut()
        .as_mut()
        .context("Runtime config has no process section")?;
    let mut env = process.env().clone().unwrap_or_default();
    for (key, value) in extra_env {
        env.push(format!("{}={}", key, value));
    }
    process.set_env(Some(env));

    spec.save(&config_path)
        .context("Failed to save runtime config.json")
}

async fn start_container(
    extra_env: &std::collections::HashMap<String, String>,
) -> Result<(AsyncFd<OwnedFd>, AsyncFd<OwnedFd>, Pid, OwnedFd, OwnedFd)> {
    apply_extra_env(extra_env).context("Failed to apply mmds env to container config")?;
    write_container_etc_hosts();

    let (stdout_read, stdout_write) =
        nix::unistd::pipe().context("Failed to create stdout pipe")?;
    let (stderr_read, stderr_write) =
        nix::unistd::pipe().context("Failed to create stderr pipe")?;

    set_nonblocking(&stdout_read).context("Failed to set stdout pipe non-blocking")?;
    set_nonblocking(&stderr_read).context("Failed to set stderr pipe non-blocking")?;

    let stdout_write_container = stdout_write
        .try_clone()
        .context("Failed to clone stdout pipe for container")?;
    let stderr_write_container = stderr_write
        .try_clone()
        .context("Failed to clone stderr pipe for container")?;

    let container_pid = tokio::task::spawn_blocking(move || -> Result<Pid> {
        let mut container = ContainerBuilder::new(CONTAINER_ID.to_string(), SyscallType::default())
            .with_root_path(CONTAINER_STATE_ROOT)
            .context("Invalid container state root path")?
            .with_stdout(stdout_write_container)
            .with_stderr(stderr_write_container)
            .as_init(CONTAINER_BUNDLE_PATH)
            .with_systemd(false)
            .with_detach(true)
            .build()
            .context("Failed to build container")?;

        container.start().context("Failed to start container")?;

        let libcontainer_pid = container
            .pid()
            .context("Container has no pid after start")?;
        Ok(Pid::from_raw(libcontainer_pid.as_raw()))
    })
    .await
    .context("Container build task panicked")??;

    Ok((
        AsyncFd::new(stdout_read).context("Failed to register stdout pipe")?,
        AsyncFd::new(stderr_read).context("Failed to register stderr pipe")?,
        container_pid,
        stdout_write,
        stderr_write,
    ))
}

fn dump_failure_diagnostics() {
    let config_path = std::path::Path::new(CONTAINER_BUNDLE_PATH).join("config.json");
    match std::fs::read_to_string(&config_path) {
        Ok(contents) => warn!(config = %contents, "debug: container config.json on failure"),
        Err(e) => {
            warn!(error = ?e, path = ?config_path, "debug: failed to read config.json on failure")
        }
    }

    match std::fs::read_to_string("/proc/self/mountinfo") {
        Ok(contents) => warn!(mountinfo = %contents, "debug: guest mountinfo on failure"),
        Err(e) => warn!(error = ?e, "debug: failed to read mountinfo on failure"),
    }

    let rootfs_path = std::path::Path::new(CONTAINER_BUNDLE_PATH).join(CONTAINER_ROOTFS_DIR);
    let conf_d_path = rootfs_path.join("etc/nginx/conf.d");
    match std::fs::metadata(&conf_d_path) {
        Ok(meta) => {
            use std::os::unix::fs::MetadataExt;
            warn!(
                path = ?conf_d_path,
                mode = format!("{:o}", meta.mode()),
                uid = meta.uid(),
                gid = meta.gid(),
                "debug: conf.d metadata on failure"
            );
        }
        Err(e) => warn!(error = ?e, path = ?conf_d_path, "debug: failed to stat conf.d on failure"),
    }
}

fn bring_up_interface(name: &str) -> Result<()> {
    let socket_fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if socket_fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }

    let mut request: libc::ifreq = unsafe { std::mem::zeroed() };
    for (dest, src) in request.ifr_name.iter_mut().zip(name.as_bytes()) {
        *dest = *src as libc::c_char;
    }

    let result = unsafe {
        if libc::ioctl(socket_fd, libc::SIOCGIFFLAGS as _, &mut request) < 0 {
            libc::close(socket_fd);
            return Err(std::io::Error::last_os_error().into());
        }

        request.ifr_ifru.ifru_flags |= (libc::IFF_UP | libc::IFF_RUNNING) as libc::c_short;

        let set_result = libc::ioctl(socket_fd, libc::SIOCSIFFLAGS as _, &request);
        libc::close(socket_fd);
        set_result
    };

    if result < 0 {
        return Err(std::io::Error::last_os_error().into());
    }

    Ok(())
}

fn assign_bootstrap_address(iface: &str) -> Result<()> {
    let socket_fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if socket_fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }

    let result = (|| -> Result<()> {
        set_ifreq_addr(socket_fd, iface, libc::SIOCSIFADDR, MMDS_BOOTSTRAP_IP)
            .context("SIOCSIFADDR failed")?;
        set_ifreq_addr(
            socket_fd,
            iface,
            libc::SIOCSIFNETMASK,
            MMDS_BOOTSTRAP_NETMASK,
        )
        .context("SIOCSIFNETMASK failed")?;
        Ok(())
    })();

    unsafe {
        libc::close(socket_fd);
    }
    result?;

    info!(iface = %iface, ip = %MMDS_BOOTSTRAP_IP, "mmds bootstrap address assigned");
    Ok(())
}

fn configure_network(iface: &str, network: &MmdsNetwork) -> Result<()> {
    let ip: std::net::Ipv4Addr = network.ip.parse().context("invalid guest ip")?;
    let prefix: u32 = network.prefix.parse().context("invalid guest prefix")?;
    let netmask = prefix_to_netmask(prefix)?;

    let socket_fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if socket_fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }

    let result = (|| -> Result<()> {
        set_ifreq_addr(socket_fd, iface, libc::SIOCSIFADDR, ip)?;
        set_ifreq_addr(socket_fd, iface, libc::SIOCSIFNETMASK, netmask)?;
        Ok(())
    })();

    unsafe {
        libc::close(socket_fd);
    }
    result?;

    if let Some(gateway) = &network.gateway {
        let gateway: std::net::Ipv4Addr = gateway.parse().context("invalid gateway ip")?;
        add_default_route(gateway)?;
    }

    if let Some(dns) = &network.dns {
        write_resolv_conf(dns, network.search.as_deref())?;
    }

    info!(iface = %iface, ip = %ip, prefix = prefix, "Guest network configured");
    Ok(())
}

fn write_container_etc_hosts() {
    let path = std::path::Path::new(CONTAINER_BUNDLE_PATH)
        .join(CONTAINER_ROOTFS_DIR)
        .join("etc/hosts");
    let contents = "127.0.0.1\tlocalhost\n::1\tlocalhost\n";
    if let Err(e) = std::fs::write(&path, contents) {
        warn!(path = ?path, error = %e, "Failed to write container /etc/hosts");
    }
}

fn write_resolv_conf(dns: &str, search: Option<&str>) -> Result<()> {
    let contents = match search {
        Some(search) => format!("nameserver {}\nsearch {}\n", dns, search),
        None => format!("nameserver {}\n", dns),
    };

    std::fs::create_dir_all("/etc").context("Failed to create /etc")?;
    std::fs::write(RESOLV_CONF_PATH, &contents).context("Failed to write guest resolv.conf")?;

    let container_resolv_conf = std::path::Path::new(CONTAINER_BUNDLE_PATH)
        .join(CONTAINER_ROOTFS_DIR)
        .join("etc/resolv.conf");
    std::fs::create_dir_all(container_resolv_conf.parent().unwrap())
        .context("Failed to create container /etc")?;
    std::fs::write(&container_resolv_conf, &contents).context("Failed to write resolv.conf")
}

fn prefix_to_netmask(prefix: u32) -> Result<std::net::Ipv4Addr> {
    if prefix > 32 {
        anyhow::bail!("invalid prefix length {}", prefix);
    }
    let bits: u32 = if prefix == 0 {
        0
    } else {
        !0u32 << (32 - prefix)
    };
    Ok(std::net::Ipv4Addr::from(bits))
}

fn set_ifreq_addr(
    socket_fd: libc::c_int,
    iface: &str,
    request_code: libc::c_ulong,
    addr: std::net::Ipv4Addr,
) -> Result<()> {
    let mut request: libc::ifreq = unsafe { std::mem::zeroed() };
    for (dest, src) in request.ifr_name.iter_mut().zip(iface.as_bytes()) {
        *dest = *src as libc::c_char;
    }

    let sockaddr = ipv4_to_sockaddr(addr);
    unsafe {
        std::ptr::copy_nonoverlapping(
            &sockaddr as *const libc::sockaddr_in as *const u8,
            &mut request.ifr_ifru.ifru_addr as *mut libc::sockaddr as *mut u8,
            std::mem::size_of::<libc::sockaddr_in>(),
        );

        if libc::ioctl(socket_fd, request_code as _, &request) < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }

    Ok(())
}

fn add_host_route(dest: std::net::Ipv4Addr, iface: &str) -> Result<()> {
    let socket_fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if socket_fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }

    let mut iface_name = [0 as libc::c_char; libc::IFNAMSIZ];
    for (dst, src) in iface_name.iter_mut().zip(iface.as_bytes()) {
        *dst = *src as libc::c_char;
    }

    let mut route: libc::rtentry = unsafe { std::mem::zeroed() };
    route.rt_dst = ipv4_to_sockaddr_storage(dest);
    route.rt_genmask = ipv4_to_sockaddr_storage(std::net::Ipv4Addr::new(255, 255, 255, 255));
    route.rt_flags = (libc::RTF_UP | libc::RTF_HOST) as libc::c_ushort;
    route.rt_dev = iface_name.as_mut_ptr();

    let result = unsafe { libc::ioctl(socket_fd, libc::SIOCADDRT as _, &route) };
    unsafe {
        libc::close(socket_fd);
    }

    if result < 0 {
        return Err(std::io::Error::last_os_error().into());
    }

    Ok(())
}

fn add_default_route(gateway: std::net::Ipv4Addr) -> Result<()> {
    let socket_fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if socket_fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }

    let mut route: libc::rtentry = unsafe { std::mem::zeroed() };
    route.rt_dst = ipv4_to_sockaddr_storage(std::net::Ipv4Addr::UNSPECIFIED);
    route.rt_genmask = ipv4_to_sockaddr_storage(std::net::Ipv4Addr::UNSPECIFIED);
    route.rt_gateway = ipv4_to_sockaddr_storage(gateway);
    route.rt_flags = (libc::RTF_UP | libc::RTF_GATEWAY) as libc::c_ushort;

    let result = unsafe { libc::ioctl(socket_fd, libc::SIOCADDRT as _, &route) };
    unsafe {
        libc::close(socket_fd);
    }

    if result < 0 {
        return Err(std::io::Error::last_os_error().into());
    }

    Ok(())
}

fn ipv4_to_sockaddr(addr: std::net::Ipv4Addr) -> libc::sockaddr_in {
    libc::sockaddr_in {
        sin_family: libc::AF_INET as libc::sa_family_t,
        sin_port: 0,
        sin_addr: libc::in_addr {
            s_addr: u32::from_ne_bytes(addr.octets()),
        },
        sin_zero: [0; 8],
    }
}

fn ipv4_to_sockaddr_storage(addr: std::net::Ipv4Addr) -> libc::sockaddr {
    let sockaddr_in = ipv4_to_sockaddr(addr);
    let mut sockaddr: libc::sockaddr = unsafe { std::mem::zeroed() };
    unsafe {
        std::ptr::copy_nonoverlapping(
            &sockaddr_in as *const libc::sockaddr_in as *const u8,
            &mut sockaddr as *mut libc::sockaddr as *mut u8,
            std::mem::size_of::<libc::sockaddr_in>(),
        );
    }
    sockaddr
}

async fn fetch_mmds_data() -> Result<MmdsData> {
    timeout(MMDS_TIMEOUT, fetch_mmds_data_inner())
        .await
        .context("Timed out fetching mmds data")?
}

async fn fetch_mmds_data_inner() -> Result<MmdsData> {
    let mut stream = TcpStream::connect(MMDS_ADDR)
        .await
        .context("Failed to connect to mmds")?;

    let request = "GET / HTTP/1.1\r\nHost: 169.254.169.254\r\nAccept: application/json\r\nConnection: close\r\n\r\n";
    stream
        .write_all(request.as_bytes())
        .await
        .context("Failed to write mmds request")?;

    let body = read_http_body(&mut stream).await?;
    serde_json::from_str(&body).context("Failed to parse mmds json")
}

async fn read_http_body(stream: &mut TcpStream) -> Result<String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        if let Some(pos) = find_header_end(&buf) {
            break pos;
        }
        let n = stream
            .read(&mut chunk)
            .await
            .context("Failed to read mmds response headers")?;
        anyhow::ensure!(n > 0, "mmds connection closed before headers completed");
        buf.extend_from_slice(&chunk[..n]);
    };

    let header_text = String::from_utf8_lossy(&buf[..header_end]);
    let content_length: usize = header_text
        .lines()
        .find_map(|line| {
            line.to_lowercase()
                .strip_prefix("content-length:")
                .map(str::trim)
                .map(str::to_string)
        })
        .context("mmds response missing content-length")?
        .parse()
        .context("mmds response has invalid content-length")?;

    let body_start = header_end + 4;
    while buf.len() < body_start + content_length {
        let n = stream
            .read(&mut chunk)
            .await
            .context("Failed to read mmds response body")?;
        anyhow::ensure!(n > 0, "mmds connection closed before body completed");
        buf.extend_from_slice(&chunk[..n]);
    }

    Ok(String::from_utf8_lossy(&buf[body_start..body_start + content_length]).into_owned())
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|window| window == b"\r\n\r\n")
}

fn mount_volume(volume: &MmdsVolume) -> Result<()> {
    let container_relative_path = volume.mount_path.trim_start_matches('/');
    let host_mount_path = std::path::Path::new(CONTAINER_BUNDLE_PATH)
        .join(CONTAINER_ROOTFS_DIR)
        .join(container_relative_path);

    std::fs::create_dir_all(&host_mount_path)
        .with_context(|| format!("Failed to create mount path {:?}", host_mount_path))?;

    for fstype in ["ext4", "xfs", "btrfs"] {
        if mount(
            Some(volume.device.as_str()),
            &host_mount_path,
            Some(fstype),
            MsFlags::empty(),
            None::<&str>,
        )
        .is_ok()
        {
            info!(device = %volume.device, mount_path = ?host_mount_path, fstype = %fstype, "Volume mounted");
            return Ok(());
        }
    }

    anyhow::bail!(
        "failed to mount device {} with any known filesystem type",
        volume.device
    )
}

fn mount_pseudo_filesystems() {
    let mounts: &[(&str, &str, &str, MsFlags, Option<&str>)] = &[
        (
            "proc",
            "/proc",
            "proc",
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
            None,
        ),
        (
            "sysfs",
            "/sys",
            "sysfs",
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
            None,
        ),
        ("devtmpfs", "/dev", "devtmpfs", MsFlags::MS_NOSUID, None),
        (
            "devpts",
            "/dev/pts",
            "devpts",
            MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC,
            Some("mode=0620,ptmxmode=0666,gid=5"),
        ),
        (
            "cgroup2",
            "/sys/fs/cgroup",
            "cgroup2",
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
            None,
        ),
    ];

    for (source, target, fstype, flags, data) in mounts {
        std::fs::create_dir_all(target).ok();
        if let Err(e) = mount(Some(*source), *target, Some(*fstype), *flags, *data) {
            if e == nix::errno::Errno::EBUSY {
                continue;
            }
            warn!(target = %target, error = %e, "Failed to mount pseudo filesystem");
        }
    }
}

fn power_off() -> ! {
    info!("powering off");
    match reboot(RebootMode::RB_POWER_OFF) {
        Ok(_) => unreachable!("reboot syscall does not return on success"),
        Err(e) => {
            error!(error = %e, "power off failed");
            std::process::exit(1);
        }
    }
}

fn push_to_backlog(backlog: &mut std::collections::VecDeque<u8>, chunk: &[u8]) {
    if chunk.len() >= LOG_BACKLOG_CAPACITY {
        backlog.clear();
        backlog.extend(&chunk[chunk.len() - LOG_BACKLOG_CAPACITY..]);
        return;
    }

    let overflow = (backlog.len() + chunk.len()).saturating_sub(LOG_BACKLOG_CAPACITY);
    backlog.drain(..overflow);
    backlog.extend(chunk);
}

async fn accept_log_client(
    listener: &VsockListener,
    backlog: &std::collections::VecDeque<u8>,
) -> Option<tokio_vsock::VsockStream> {
    let (mut conn, addr) = match listener.accept().await {
        Ok(accepted) => accepted,
        Err(e) => {
            error!(error = %e, "Failed to accept log connection");
            return None;
        }
    };
    info!(target: "workload.logstream", peer = ?addr, "log client accepted");

    let (front, back) = backlog.as_slices();
    if let Err(e) = conn.write_all(front).await.and(conn.write_all(back).await) {
        info!(target: "workload.logstream", error = %e, "failed replaying log backlog to new client");
        return None;
    }

    Some(conn)
}

async fn stream_logs(listener: VsockListener, stdout: AsyncFd<OwnedFd>, stderr: AsyncFd<OwnedFd>) {
    let mut client: Option<tokio_vsock::VsockStream> = None;
    let mut backlog: std::collections::VecDeque<u8> =
        std::collections::VecDeque::with_capacity(LOG_BACKLOG_CAPACITY);

    let mut stdout_buf = [0u8; 4096];
    let mut stderr_buf = [0u8; 4096];
    let mut stdout_open = true;
    let mut stderr_open = true;

    while stdout_open || stderr_open {
        tokio::select! {
            accepted = accept_log_client(&listener, &backlog) => {
                if accepted.is_some() {
                    client = accepted;
                }
            }
            result = read_pty(&stdout, &mut stdout_buf), if stdout_open => {
                match result {
                    Ok(0) => stdout_open = false,
                    Ok(n) => {
                        info!(target: "workload.stdout", "{}", String::from_utf8_lossy(&stdout_buf[..n]).trim_end());
                        push_to_backlog(&mut backlog, &stdout_buf[..n]);
                        if let Some(stream) = client.as_mut() {
                            info!(target: "workload.logstream", bytes = n, "writing stdout chunk to log client");
                            match stream.write_all(&stdout_buf[..n]).await {
                                Ok(()) => info!(target: "workload.logstream", "stdout chunk written to log client"),
                                Err(e) => {
                                    info!(target: "workload.logstream", error = %e, "failed writing stdout chunk to log client");
                                    client = None;
                                }
                            }
                        } else {
                            info!(target: "workload.logstream", "no log client connected, dropping stdout chunk");
                        }
                    }
                    Err(_) => stdout_open = false,
                }
            }
            result = read_pty(&stderr, &mut stderr_buf), if stderr_open => {
                match result {
                    Ok(0) => stderr_open = false,
                    Ok(n) => {
                        info!(target: "workload.stderr", "{}", String::from_utf8_lossy(&stderr_buf[..n]).trim_end());
                        push_to_backlog(&mut backlog, &stderr_buf[..n]);
                        if let Some(stream) = client.as_mut() {
                            info!(target: "workload.logstream", bytes = n, "writing stderr chunk to log client");
                            match stream.write_all(&stderr_buf[..n]).await {
                                Ok(()) => info!(target: "workload.logstream", "stderr chunk written to log client"),
                                Err(e) => {
                                    info!(target: "workload.logstream", error = %e, "failed writing stderr chunk to log client");
                                    client = None;
                                }
                            }
                        } else {
                            info!(target: "workload.logstream", "no log client connected, dropping stderr chunk");
                        }
                    }
                    Err(_) => stderr_open = false,
                }
            }
        }
    }
}

async fn serve_exec(listener: VsockListener) {
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                error!(error = %e, "Failed to accept exec connection");
                continue;
            }
        };

        tokio::spawn(handle_exec_session(stream));
    }
}

async fn handle_exec_session(stream: tokio_vsock::VsockStream) {
    info!("exec session starting");

    let pty = match openpty(None, None) {
        Ok(pty) => pty,
        Err(e) => {
            error!(error = ?e, "Failed to open pty");
            return;
        }
    };

    let slave = pty.slave;
    let child_pid = match exec_shell_in_container(slave).await {
        Ok(pid) => pid,
        Err(e) => {
            error!(error = ?e, "Failed to spawn exec shell");
            return;
        }
    };
    info!(pid = child_pid.as_raw(), "exec shell started in container");

    if let Err(e) = set_nonblocking(&pty.master) {
        error!(error = ?e, "Failed to set pty master nonblocking");
        reap_exec_child(child_pid).await;
        return;
    }

    match AsyncFd::new(pty.master) {
        Ok(master) => pipe_pty_to_vsock(master, stream, child_pid).await,
        Err(e) => {
            error!(error = ?e, "Failed to register pty master with async runtime");
            reap_exec_child(child_pid).await;
        }
    }

    info!("exec session ended");
}

fn set_nonblocking(fd: &OwnedFd) -> Result<()> {
    let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
    if flags < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let result = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) };
    if result < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

async fn exec_shell_in_container(slave: OwnedFd) -> Result<Pid> {
    tokio::task::spawn_blocking(move || -> Result<Pid> {
        let stdin = slave
            .try_clone()
            .context("Failed to clone pty slave for stdin")?;
        let stdout = slave
            .try_clone()
            .context("Failed to clone pty slave for stdout")?;

        let libcontainer_pid =
            ContainerBuilder::new(CONTAINER_ID.to_string(), SyscallType::default())
                .with_root_path(CONTAINER_STATE_ROOT)
                .context("Invalid container state root path")?
                .with_stdin(stdin)
                .with_stdout(stdout)
                .with_stderr(slave)
                .as_tenant()
                .with_container_args(vec!["sh".to_string()])
                .with_detach(true)
                .build()
                .context("Failed to exec into container")?;

        Ok(Pid::from_raw(libcontainer_pid.as_raw()))
    })
    .await
    .context("Exec task panicked")?
}

async fn reap_exec_child(pid: Pid) {
    let _ = nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGKILL);
    let _ = tokio::task::spawn_blocking(move || waitpid(pid, None)).await;
}

async fn pipe_pty_to_vsock(
    master: AsyncFd<OwnedFd>,
    stream: tokio_vsock::VsockStream,
    child_pid: Pid,
) {
    let (mut vsock_read, mut vsock_write) = stream.into_split();

    let output_task = async {
        let mut buf = [0u8; 4096];
        loop {
            match read_pty(&master, &mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if vsock_write.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    };

    let input_task = async {
        let mut buf = [0u8; 4096];
        loop {
            match vsock_read.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if write_pty(&master, &buf[..n]).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    };

    tokio::select! {
        _ = output_task => {}
        _ = input_task => {}
    }

    reap_exec_child(child_pid).await;
}

async fn read_pty(master: &AsyncFd<OwnedFd>, buf: &mut [u8]) -> std::io::Result<usize> {
    loop {
        let mut guard = master.readable().await?;
        match guard.try_io(|fd| {
            let n = unsafe {
                libc::read(
                    fd.as_raw_fd(),
                    buf.as_mut_ptr() as *mut libc::c_void,
                    buf.len(),
                )
            };
            if n < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(n as usize)
            }
        }) {
            Ok(result) => return result,
            Err(_would_block) => continue,
        }
    }
}

async fn write_pty(master: &AsyncFd<OwnedFd>, buf: &[u8]) -> std::io::Result<()> {
    let mut written = 0;
    while written < buf.len() {
        let mut guard = master.writable().await?;
        match guard.try_io(|fd| {
            let n = unsafe {
                libc::write(
                    fd.as_raw_fd(),
                    buf[written..].as_ptr() as *const libc::c_void,
                    buf.len() - written,
                )
            };
            if n < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(n as usize)
            }
        }) {
            Ok(Ok(n)) => written += n,
            Ok(Err(e)) => return Err(e),
            Err(_would_block) => continue,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sockaddr_holds_octets_in_network_order() {
        let sockaddr = ipv4_to_sockaddr(std::net::Ipv4Addr::new(169, 254, 169, 2));
        assert_eq!(sockaddr.sin_addr.s_addr.to_ne_bytes(), [169, 254, 169, 2]);
        assert_eq!(sockaddr.sin_family, libc::AF_INET as libc::sa_family_t);

        let netmask = ipv4_to_sockaddr(prefix_to_netmask(16).unwrap());
        assert_eq!(netmask.sin_addr.s_addr.to_ne_bytes(), [255, 255, 0, 0]);
    }
}
