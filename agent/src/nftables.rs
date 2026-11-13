use crate::spec::PortProtocol;
use anyhow::{Context, Result};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::debug;

const TABLE_NAME: &str = "csfx";
const CHAIN_NAME: &str = "rg_isolation";
const BRIDGE_SET_NAME: &str = "rg_bridges";
const NAT_TABLE_NAME: &str = "csfx_nat";
const NAT_CHAIN_NAME: &str = "node_port_dnat";
const SNAT_CHAIN_NAME: &str = "node_port_snat";
const MGMT_GUARD_CHAIN_NAME: &str = "guest_mgmt_guard";
const GUEST_BRIDGE_PATTERN: &str = "csfxrg*";
const MGMT_GUARD_SELECTORS: [&str; 2] = ["oifname \"wgmgmt0\"", "ip daddr 10.100.0.0/16"];

pub struct BridgeRules {
    pub name: String,
    pub mesh_iface: Option<String>,
}

pub async fn apply_bridge_rules(bridges: &[BridgeRules]) -> Result<()> {
    apply_script(&ruleset(bridges)).await?;
    debug!(bridges = bridges.len(), "nftables bridge rules applied");
    Ok(())
}

fn ruleset(bridges: &[BridgeRules]) -> String {
    let mut lines = vec![
        format!("add table inet {TABLE_NAME}"),
        format!("add table ip {NAT_TABLE_NAME}"),
        format!(
            "add chain ip {NAT_TABLE_NAME} {NAT_CHAIN_NAME} {{ type nat hook prerouting priority -100 ; policy accept ; }}"
        ),
    ];
    lines.extend(isolation_rules(bridges));
    lines.extend(guard_rules());
    lines.extend(snat_rules(bridges));
    lines.join("\n")
}

fn isolation_rules(bridges: &[BridgeRules]) -> Vec<String> {
    let mut lines = vec![
        format!("add set inet {TABLE_NAME} {BRIDGE_SET_NAME} {{ type ifname ; }}"),
        format!("flush set inet {TABLE_NAME} {BRIDGE_SET_NAME}"),
    ];
    if !bridges.is_empty() {
        let elements: Vec<_> = bridges
            .iter()
            .map(|bridge| format!("\"{}\"", bridge.name))
            .collect();
        lines.push(format!(
            "add element inet {TABLE_NAME} {BRIDGE_SET_NAME} {{ {} }}",
            elements.join(", ")
        ));
    }
    lines.push(format!(
        "add chain inet {TABLE_NAME} {CHAIN_NAME} {{ type filter hook forward priority 0 ; policy accept ; }}"
    ));
    lines.push(format!("flush chain inet {TABLE_NAME} {CHAIN_NAME}"));
    for bridge in bridges {
        lines.push(format!(
            "add rule inet {TABLE_NAME} {CHAIN_NAME} iifname \"{0}\" oifname \"{0}\" accept",
            bridge.name
        ));
    }
    lines.push(format!(
        "add rule inet {TABLE_NAME} {CHAIN_NAME} iifname @{BRIDGE_SET_NAME} oifname @{BRIDGE_SET_NAME} drop"
    ));
    lines
}

fn guard_rules() -> Vec<String> {
    let mut lines = vec![
        format!(
            "add chain inet {TABLE_NAME} {MGMT_GUARD_CHAIN_NAME} {{ type filter hook forward priority -10 ; policy accept ; }}"
        ),
        format!("flush chain inet {TABLE_NAME} {MGMT_GUARD_CHAIN_NAME}"),
    ];
    for selector in MGMT_GUARD_SELECTORS {
        lines.push(format!(
            "add rule inet {TABLE_NAME} {MGMT_GUARD_CHAIN_NAME} iifname \"{GUEST_BRIDGE_PATTERN}\" {selector} drop"
        ));
    }
    lines
}

fn snat_rules(bridges: &[BridgeRules]) -> Vec<String> {
    let mut lines = vec![
        format!(
            "add chain ip {NAT_TABLE_NAME} {SNAT_CHAIN_NAME} {{ type nat hook postrouting priority 100 ; policy accept ; }}"
        ),
        format!("flush chain ip {NAT_TABLE_NAME} {SNAT_CHAIN_NAME}"),
    ];
    for bridge in bridges {
        if let Some(wg_iface) = &bridge.mesh_iface {
            lines.push(format!(
                "add rule ip {NAT_TABLE_NAME} {SNAT_CHAIN_NAME} iifname \"{}\" oifname \"{wg_iface}\" accept comment \"{}\"",
                bridge.name,
                mesh_comment(&bridge.name)
            ));
        }
    }
    for bridge in bridges {
        let comment = snat_comment(&bridge.name);
        lines.push(format!(
            "add rule ip {NAT_TABLE_NAME} {SNAT_CHAIN_NAME} iifname \"{0}\" ct status dnat masquerade comment \"{comment}\"",
            bridge.name
        ));
        lines.push(format!(
            "add rule ip {NAT_TABLE_NAME} {SNAT_CHAIN_NAME} iifname \"{0}\" oifname != \"{0}\" masquerade comment \"{comment}\"",
            bridge.name
        ));
    }
    lines
}

async fn apply_script(script: &str) -> Result<()> {
    let mut child = Command::new("nft")
        .args(["-f", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to execute nft")?;
    let mut stdin = child.stdin.take().context("nft stdin is unavailable")?;
    let written = stdin.write_all(script.as_bytes()).await;
    drop(stdin);
    let output = child
        .wait_with_output()
        .await
        .context("failed to wait for nft")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("nft ruleset failed stderr={}", stderr);
    }
    written.context("failed to send ruleset to nft")
}

fn ensure_node_port_in_range(node_port: u16) -> Result<()> {
    let bound = |name: &str, default: u16| {
        std::env::var(name)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
    };
    let (start, end) = (
        bound("NODE_PORT_RANGE_START", 30000),
        bound("NODE_PORT_RANGE_END", 32767),
    );
    anyhow::ensure!(
        (start..=end).contains(&node_port),
        "node port {node_port} is outside the allowed range {start}-{end}"
    );
    Ok(())
}

pub async fn add_node_port_dnat(
    workload_id: &str,
    protocol: PortProtocol,
    node_port: u16,
    guest_ip: &str,
    container_port: u16,
) -> Result<()> {
    ensure_node_port_in_range(node_port)?;
    run_nft(&[
        "add",
        "rule",
        "ip",
        NAT_TABLE_NAME,
        NAT_CHAIN_NAME,
        "fib",
        "daddr",
        "type",
        "local",
        protocol.as_str(),
        "dport",
        &node_port.to_string(),
        "dnat",
        "to",
        &format!("{}:{}", guest_ip, container_port),
        "comment",
        &format!("\"{}\"", workload_id),
    ])
    .await
}

pub async fn add_rg_port_dnat(
    workload_id: &str,
    protocol: PortProtocol,
    rg_gateway_ip: &str,
    rg_port: u16,
    guest_ip: &str,
    container_port: u16,
) -> Result<()> {
    run_nft(&[
        "add",
        "rule",
        "ip",
        NAT_TABLE_NAME,
        NAT_CHAIN_NAME,
        "ip",
        "daddr",
        rg_gateway_ip,
        protocol.as_str(),
        "dport",
        &rg_port.to_string(),
        "dnat",
        "to",
        &format!("{}:{}", guest_ip, container_port),
        "comment",
        &format!("\"{}\"", workload_id),
    ])
    .await
}

pub async fn dnat_bridge_port(bridge_name: &str, rg_gateway_ip: &str, port: u16) -> Result<()> {
    run_nft(&[
        "add",
        "rule",
        "ip",
        NAT_TABLE_NAME,
        NAT_CHAIN_NAME,
        "iifname",
        bridge_name,
        "ip",
        "daddr",
        rg_gateway_ip,
        "tcp",
        "dport",
        &port.to_string(),
        "dnat",
        "to",
        &format!("127.0.0.1:{}", port),
        "comment",
        &bridge_dnat_comment(bridge_name),
    ])
    .await
}

fn mesh_comment(bridge_name: &str) -> String {
    format!("{bridge_name}-mesh")
}

fn snat_comment(bridge_name: &str) -> String {
    format!("{bridge_name}-snat")
}

pub async fn remove_node_port_rules(workload_id: &str) -> Result<()> {
    delete_rules_containing(
        "ip",
        NAT_TABLE_NAME,
        NAT_CHAIN_NAME,
        &format!("\"{workload_id}\""),
    )
    .await
}

pub async fn ensure_bridge_dnat(bridge_name: &str, rg_gateway_ip: &str, port: u16) -> Result<()> {
    let comment = bridge_dnat_comment(bridge_name);
    let listing = list_chain("ip", NAT_TABLE_NAME, NAT_CHAIN_NAME).await?;
    if listing.contains(&comment) {
        return Ok(());
    }
    dnat_bridge_port(bridge_name, rg_gateway_ip, port).await
}

pub async fn remove_bridge_dnat(bridge_name: &str) -> Result<()> {
    delete_rules_containing(
        "ip",
        NAT_TABLE_NAME,
        NAT_CHAIN_NAME,
        &bridge_dnat_comment(bridge_name),
    )
    .await
}

fn bridge_dnat_comment(bridge_name: &str) -> String {
    format!("\"{bridge_name}-s3\"")
}

async fn list_chain(family: &str, table: &str, chain: &str) -> Result<String> {
    let output = Command::new("nft")
        .args(["-a", "list", "chain", family, table, chain])
        .output()
        .await
        .context("failed to list nftables rules")?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

async fn delete_rules_containing(
    family: &str,
    table: &str,
    chain: &str,
    needle: &str,
) -> Result<()> {
    let listing = list_chain(family, table, chain).await?;
    for handle in rule_handles(&listing, needle) {
        run_nft(&[
            "delete",
            "rule",
            family,
            table,
            chain,
            "handle",
            &handle.to_string(),
        ])
        .await?;
    }

    Ok(())
}

fn rule_handles(listing: &str, needle: &str) -> Vec<u32> {
    listing
        .lines()
        .filter(|line| line.contains(needle))
        .filter_map(|line| line.rsplit("handle ").next()?.trim().parse().ok())
        .collect()
}

async fn run_nft(args: &[&str]) -> Result<()> {
    let output = Command::new("nft")
        .args(args)
        .output()
        .await
        .context("failed to execute nft")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("File exists") {
            return Ok(());
        }
        anyhow::bail!("nft command failed args={:?} stderr={}", args, stderr);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_comments_are_derived_from_the_bridge_name() {
        assert_eq!(mesh_comment("csfxrg1"), "csfxrg1-mesh");
        assert_eq!(snat_comment("csfxrg1"), "csfxrg1-snat");
    }

    #[test]
    fn rule_handles_are_read_from_matching_lines_only() {
        let listing = "table ip csfx_nat {\n\tchain node_port_dnat {\n\t\tdnat to 10.0.0.2:80 comment \"w-1\" # handle 7\n\t\tdnat to 10.0.0.3:80 comment \"w-2\" # handle 9\n\t\tdnat to 10.0.0.4:80 comment \"w-1\" # handle 12\n\t}\n}";
        assert_eq!(rule_handles(listing, "\"w-1\""), vec![7, 12]);
        assert_eq!(rule_handles(listing, "\"w-2\""), vec![9]);
        assert!(rule_handles(listing, "\"w-3\"").is_empty());
        assert!(rule_handles("", "w-1").is_empty());
    }

    #[test]
    fn lines_without_a_numeric_handle_are_ignored() {
        assert!(rule_handles("rule comment \"w-1\" handle abc", "w-1").is_empty());
        assert!(rule_handles("rule comment \"w-1\"", "w-1").is_empty());
    }

    use crate::testing::FakeTools;

    const SUCCEEDING: &str = "exit 0";
    const LISTING_WITH_S3: &str = r#"case "$*" in *"list chain"*) printf '%s\n' 'iifname "br1" ip daddr 10.0.0.1 tcp dport 3900 dnat to 127.0.0.1:3900 comment "br1-s3" # handle 21';; esac"#;
    const LISTING: &str = r#"case "$*" in *"list chain"*) printf '%s\n' 'iifname "br1" oifname "br2" drop # handle 4' 'iifname "br1" accept comment "br1-mesh" # handle 9' 'masquerade comment "br1-snat" # handle 11' 'dnat to 10.0.0.2:80 comment "w-1" # handle 15';; esac"#;

    #[tokio::test]
    async fn existing_objects_are_not_an_error_but_other_failures_are() {
        {
            let _tools = FakeTools::install(&[("nft", "echo 'Error: File exists' >&2\nexit 1")]);
            run_nft(&["add", "table", "inet", "csfx"]).await.unwrap();
        }
        let _tools = FakeTools::install(&[("nft", "echo 'syntax error' >&2\nexit 1")]);
        let error = run_nft(&["add", "bogus"]).await.unwrap_err().to_string();
        assert!(error.contains("nft command failed"));
        assert!(error.contains("syntax error"));
    }

    fn bridge(name: &str, mesh_iface: Option<&str>) -> BridgeRules {
        BridgeRules {
            name: name.to_string(),
            mesh_iface: mesh_iface.map(str::to_string),
        }
    }

    const CAPTURING: &str = "cat >> \"$0.stdin\"";

    #[test]
    fn isolation_uses_one_drop_rule_over_a_bridge_set() {
        let rules = isolation_rules(&[
            bridge("br1", None),
            bridge("br2", None),
            bridge("br3", None),
        ]);

        assert!(rules.contains(
            &"add element inet csfx rg_bridges { \"br1\", \"br2\", \"br3\" }".to_string()
        ));
        let drops: Vec<_> = rules.iter().filter(|rule| rule.contains(" drop")).collect();
        assert_eq!(
            drops,
            vec!["add rule inet csfx rg_isolation iifname @rg_bridges oifname @rg_bridges drop"]
        );
        assert!(rules.contains(
            &"add rule inet csfx rg_isolation iifname \"br2\" oifname \"br2\" accept".to_string()
        ));
        let last_accept = rules.iter().rposition(|rule| rule.contains("accept"));
        assert!(last_accept < rules.iter().position(|rule| rule.contains(" drop")));
    }

    #[test]
    fn isolation_rule_count_grows_linearly_with_bridges() {
        let count = |bridges: usize| {
            let list: Vec<_> = (0..bridges)
                .map(|index| bridge(&format!("br{index}"), None))
                .collect();
            isolation_rules(&list).len()
        };

        assert_eq!(count(200) - count(100), 100);
        assert_eq!(count(200), 206);
    }

    #[test]
    fn an_empty_bridge_set_is_flushed_without_elements() {
        let rules = isolation_rules(&[]);

        assert!(rules.contains(&"flush set inet csfx rg_bridges".to_string()));
        assert!(!rules.iter().any(|rule| rule.contains("add element")));
    }

    #[test]
    fn the_bridge_set_is_declared_before_the_rule_that_uses_it() {
        let script = ruleset(&[bridge("br1", None)]);

        let declared = script.find("add set inet csfx rg_bridges").unwrap();
        assert!(declared < script.find("@rg_bridges").unwrap());
    }

    #[test]
    fn guests_are_blocked_from_the_management_mesh() {
        let rules = guard_rules();

        assert!(rules.contains(
            &"add rule inet csfx guest_mgmt_guard iifname \"csfxrg*\" oifname \"wgmgmt0\" drop"
                .to_string()
        ));
        assert!(rules.contains(
            &"add rule inet csfx guest_mgmt_guard iifname \"csfxrg*\" ip daddr 10.100.0.0/16 drop"
                .to_string()
        ));
    }

    #[test]
    fn mesh_exemptions_come_before_the_masquerade_rules() {
        let rules = snat_rules(&[bridge("br1", Some("wg1")), bridge("br2", None)]);

        let mesh = rules
            .iter()
            .position(|rule| rule.contains("accept comment"))
            .unwrap();
        let first_masquerade = rules
            .iter()
            .position(|rule| rule.contains("masquerade"))
            .unwrap();
        assert!(mesh < first_masquerade);
        assert_eq!(
            rules
                .iter()
                .filter(|rule| rule.contains("accept comment"))
                .count(),
            1
        );
        assert_eq!(
            rules
                .iter()
                .filter(|rule| rule.contains("masquerade"))
                .count(),
            4
        );
        assert!(rules
            .iter()
            .any(|rule| rule.contains("ct status dnat masquerade comment \"br1-snat\"")));
        assert!(rules
            .iter()
            .any(|rule| rule.contains("oifname != \"br2\" masquerade comment \"br2-snat\"")));
    }

    #[test]
    fn every_chain_is_flushed_before_it_is_rebuilt() {
        let script = ruleset(&[]);

        for chain in [
            "flush chain inet csfx rg_isolation",
            "flush chain inet csfx guest_mgmt_guard",
            "flush chain ip csfx_nat node_port_snat",
        ] {
            assert!(script.contains(chain), "{chain}");
        }
        assert!(!script.contains("flush chain ip csfx_nat node_port_dnat"));
    }

    #[tokio::test]
    async fn the_ruleset_is_applied_as_one_transaction() {
        let tools = FakeTools::install(&[("nft", CAPTURING)]);

        apply_bridge_rules(&[bridge("br1", None), bridge("br2", None)])
            .await
            .unwrap();

        assert_eq!(tools.calls("nft"), vec!["-f -".to_string()]);
        let script = tools.stdin("nft");
        assert!(script.contains("add table inet csfx"));
        assert!(script.contains("add element inet csfx rg_bridges { \"br1\", \"br2\" }"));
    }

    #[tokio::test]
    async fn a_rejected_ruleset_is_reported_with_the_nft_message() {
        let _tools =
            FakeTools::install(&[("nft", "cat > /dev/null\necho 'syntax error' >&2\nexit 1")]);

        let error = apply_bridge_rules(&[bridge("br1", None)])
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("nft ruleset failed"));
        assert!(error.contains("syntax error"));
    }

    #[tokio::test]
    async fn the_bridge_dnat_rule_is_only_added_when_missing() {
        {
            let tools = FakeTools::install(&[("nft", LISTING_WITH_S3)]);
            ensure_bridge_dnat("br1", "10.0.0.1", 3900).await.unwrap();
            assert_eq!(tools.calls("nft").len(), 1);
        }
        let tools = FakeTools::install(&[("nft", LISTING)]);
        ensure_bridge_dnat("br1", "10.0.0.1", 3900).await.unwrap();
        let calls = tools.calls("nft");
        assert_eq!(calls.len(), 2);
        assert!(calls[1].starts_with("add rule ip csfx_nat node_port_dnat iifname br1"));
    }

    #[tokio::test]
    async fn the_bridge_dnat_rule_is_removed_by_handle() {
        let tools = FakeTools::install(&[("nft", LISTING_WITH_S3)]);

        remove_bridge_dnat("br1").await.unwrap();

        assert!(tools
            .calls("nft")
            .contains(&"delete rule ip csfx_nat node_port_dnat handle 21".to_string()));
    }

    #[tokio::test]
    async fn node_port_outside_range_creates_no_rule() {
        let tools = FakeTools::install(&[("nft", SUCCEEDING)]);
        for port in [22, 7443] {
            assert!(
                add_node_port_dnat("w-1", PortProtocol::Tcp, port, "10.0.0.2", 80)
                    .await
                    .is_err()
            );
        }
        assert!(tools.calls("nft").is_empty());
    }

    #[tokio::test]
    async fn node_port_rules_forward_to_the_guest() {
        let tools = FakeTools::install(&[("nft", SUCCEEDING)]);

        add_node_port_dnat("w-1", PortProtocol::Tcp, 30080, "10.0.0.2", 80)
            .await
            .unwrap();
        add_rg_port_dnat("w-1", PortProtocol::Udp, "10.0.0.1", 5353, "10.0.0.2", 53)
            .await
            .unwrap();
        dnat_bridge_port("br1", "10.0.0.1", 3900).await.unwrap();

        let calls = tools.calls("nft");
        assert!(calls[0].contains("tcp dport 30080 dnat to 10.0.0.2:80 comment \"w-1\""));
        assert!(calls[1].contains("ip daddr 10.0.0.1 udp dport 5353 dnat to 10.0.0.2:53"));
        assert!(calls[2]
            .contains("iifname br1 ip daddr 10.0.0.1 tcp dport 3900 dnat to 127.0.0.1:3900"));
        assert!(calls[2].ends_with("comment \"br1-s3\""));
    }

    #[tokio::test]
    async fn workload_rules_are_removed_by_handle() {
        let tools = FakeTools::install(&[("nft", LISTING)]);

        remove_node_port_rules("w-1").await.unwrap();

        let calls = tools.calls("nft");
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1], "delete rule ip csfx_nat node_port_dnat handle 15");
    }

    #[tokio::test]
    async fn failed_rule_deletion_is_propagated() {
        let _tools = FakeTools::install(&[(
            "nft",
            "case \"$*\" in *delete*) echo denied >&2; exit 1;; esac\nprintf '%s\\n' 'x comment \"w-1\" # handle 3'",
        )]);

        assert!(remove_node_port_rules("w-1").await.is_err());
    }

    mod lab {
        use std::process::{Child, Command as Process, Stdio};

        const SETUP: &[&str] = &[
            "sysctl -qw net.ipv4.ip_forward=1",
            "ip netns add csfxlab-guest",
            "ip netns add csfxlab-peer",
            "ip netns add csfxlab-other",
            "ip link add csfxrglab0 type bridge",
            "ip addr add 10.200.0.1/24 dev csfxrglab0",
            "ip link set csfxrglab0 up",
            "ip link add labg0 type veth peer name labg1",
            "ip link set labg1 netns csfxlab-guest",
            "ip link set labg0 master csfxrglab0",
            "ip link set labg0 up",
            "ip -n csfxlab-guest addr add 10.200.0.2/24 dev labg1",
            "ip -n csfxlab-guest link set labg1 up",
            "ip -n csfxlab-guest route add default via 10.200.0.1",
            "ip link add csfxrglab1 type bridge",
            "ip addr add 10.201.0.1/24 dev csfxrglab1",
            "ip link set csfxrglab1 up",
            "ip link add labo0 type veth peer name labo1",
            "ip link set labo1 netns csfxlab-other",
            "ip link set labo0 master csfxrglab1",
            "ip link set labo0 up",
            "ip -n csfxlab-other addr add 10.201.0.2/24 dev labo1",
            "ip -n csfxlab-other link set labo1 up",
            "ip -n csfxlab-other route add default via 10.201.0.1",
            "ip link add wgmgmt0 type veth peer name labp1",
            "ip link set labp1 netns csfxlab-peer",
            "ip addr add 10.100.0.7/24 dev wgmgmt0",
            "ip link set wgmgmt0 up",
            "ip -n csfxlab-peer addr add 10.100.0.9/24 dev labp1",
            "ip -n csfxlab-peer link set labp1 up",
            "ip -n csfxlab-peer route add default via 10.100.0.7",
        ];

        const TEARDOWN: &[&str] = &[
            "nft delete table inet csfx",
            "nft delete table ip csfx_nat",
            "ip netns del csfxlab-guest",
            "ip netns del csfxlab-peer",
            "ip netns del csfxlab-other",
            "ip link del csfxrglab0",
            "ip link del csfxrglab1",
            "ip link del wgmgmt0",
        ];

        pub struct Lab {
            listeners: Vec<Child>,
        }

        fn shell(command: &str) -> bool {
            Process::new("sh")
                .args(["-c", command])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
        }

        const LISTENER: &str = "import socket, sys\n\
             server = socket.create_server((sys.argv[1], 8000))\n\
             while True:\n\
             \x20   server.accept()[0].close()";

        fn listen(namespace: &str, address: &str) -> Child {
            Process::new("ip")
                .args(["netns", "exec", namespace])
                .args(["python3", "-c", LISTENER, address])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap()
        }

        fn capture(command: &str) -> String {
            Process::new("sh")
                .args(["-c", command])
                .output()
                .map(|output| {
                    format!(
                        "{}{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    )
                })
                .unwrap_or_default()
        }

        impl Lab {
            pub fn build() -> Self {
                for command in SETUP {
                    assert!(shell(command), "lab setup failed: {command}");
                }
                let listeners = vec![
                    listen("csfxlab-peer", "10.100.0.9"),
                    listen("csfxlab-other", "10.201.0.2"),
                ];
                std::thread::sleep(std::time::Duration::from_secs(2));
                Lab { listeners }
            }

            pub fn diagnostics(&self, address: &str) -> String {
                let probe = format!(
                    "ip netns exec csfxlab-guest python3 -c \
                     \"import socket; socket.create_connection(('{address}', 8000), 2)\" 2>&1; \
                     nft list ruleset; iptables -S; ip -n csfxlab-guest route; ip route; \
                     sysctl net.ipv4.ip_forward net.bridge.bridge-nf-call-iptables \
                     net.ipv4.conf.all.rp_filter"
                );
                capture(&probe)
            }

            pub fn guest_reaches(&self, address: &str) -> bool {
                let probe = format!(
                    "ip netns exec csfxlab-guest python3 -c \
                     \"import socket; socket.create_connection(('{address}', 8000), 2)\""
                );
                shell(&probe)
            }
        }

        impl Drop for Lab {
            fn drop(&mut self) {
                for listener in &mut self.listeners {
                    let _ = listener.kill();
                }
                for command in TEARDOWN {
                    shell(command);
                }
            }
        }
    }

    #[tokio::test]
    async fn isolation_holds_in_a_network_namespace_lab() {
        if std::env::var("CSFX_ISOLATION_TEST").is_err() {
            return;
        }
        let lab = lab::Lab::build();
        let lone = [bridge("csfxrglab0", None)];
        let both = [bridge("csfxrglab0", None), bridge("csfxrglab1", None)];

        apply_bridge_rules(&lone).await.unwrap();
        run_nft(&["flush", "chain", "inet", TABLE_NAME, MGMT_GUARD_CHAIN_NAME])
            .await
            .unwrap();
        for address in ["10.100.0.9", "10.201.0.2"] {
            assert!(lab.guest_reaches(address), "{}", lab.diagnostics(address));
        }

        apply_bridge_rules(&both).await.unwrap();
        assert!(!lab.guest_reaches("10.100.0.9"));
        assert!(!lab.guest_reaches("10.201.0.2"));

        run_nft(&["delete", "table", "inet", TABLE_NAME])
            .await
            .unwrap();
        run_nft(&["delete", "table", "ip", NAT_TABLE_NAME])
            .await
            .unwrap();
        for address in ["10.100.0.9", "10.201.0.2"] {
            assert!(lab.guest_reaches(address), "{}", lab.diagnostics(address));
        }

        apply_bridge_rules(&both).await.unwrap();
        assert!(!lab.guest_reaches("10.100.0.9"));
        assert!(!lab.guest_reaches("10.201.0.2"));
    }
}
