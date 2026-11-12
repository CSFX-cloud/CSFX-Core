use anyhow::{Context, Result};
use tokio::process::Command;
use tracing::info;

const TABLE_NAME: &str = "csfx";
const CHAIN_NAME: &str = "rg_isolation";
const NAT_TABLE_NAME: &str = "csfx_nat";
const NAT_CHAIN_NAME: &str = "node_port_dnat";
const SNAT_CHAIN_NAME: &str = "node_port_snat";
const MGMT_GUARD_CHAIN_NAME: &str = "guest_mgmt_guard";
const GUEST_BRIDGE_PATTERN: &str = "csfxrg*";
const MGMT_INTERFACE: &str = "wgmgmt0";
const MGMT_MESH_CIDR: &str = "10.100.0.0/16";
const MGMT_GUARD_SELECTORS: [&[&str]; 2] = [
    &["oifname", MGMT_INTERFACE],
    &["ip", "daddr", MGMT_MESH_CIDR],
];

pub async fn ensure_table_and_chain() -> Result<()> {
    run_nft(&["add", "table", "inet", TABLE_NAME]).await?;

    run_nft(&[
        "add",
        "chain",
        "inet",
        TABLE_NAME,
        CHAIN_NAME,
        "{ type filter hook forward priority 0 ; policy accept ; }",
    ])
    .await?;

    info!(
        table = TABLE_NAME,
        chain = CHAIN_NAME,
        "nftables RG isolation chain ready"
    );

    guard_management_mesh().await?;

    run_nft(&["add", "table", "ip", NAT_TABLE_NAME]).await?;

    run_nft(&[
        "add",
        "chain",
        "ip",
        NAT_TABLE_NAME,
        NAT_CHAIN_NAME,
        "{ type nat hook prerouting priority -100 ; policy accept ; }",
    ])
    .await?;

    run_nft(&[
        "add",
        "chain",
        "ip",
        NAT_TABLE_NAME,
        SNAT_CHAIN_NAME,
        "{ type nat hook postrouting priority 100 ; policy accept ; }",
    ])
    .await?;

    info!(
        table = NAT_TABLE_NAME,
        chains = "node_port_dnat,node_port_snat",
        "nftables node port nat chains ready"
    );

    Ok(())
}

async fn guard_management_mesh() -> Result<()> {
    run_nft(&[
        "add",
        "chain",
        "inet",
        TABLE_NAME,
        MGMT_GUARD_CHAIN_NAME,
        "{ type filter hook forward priority -10 ; policy accept ; }",
    ])
    .await?;
    run_nft(&["flush", "chain", "inet", TABLE_NAME, MGMT_GUARD_CHAIN_NAME]).await?;
    for selector in MGMT_GUARD_SELECTORS {
        let mut rule = vec![
            "add",
            "rule",
            "inet",
            TABLE_NAME,
            MGMT_GUARD_CHAIN_NAME,
            "iifname",
            GUEST_BRIDGE_PATTERN,
        ];
        rule.extend_from_slice(selector);
        rule.push("drop");
        run_nft(&rule).await?;
    }

    info!(
        chain = MGMT_GUARD_CHAIN_NAME,
        "nftables management mesh guard ready"
    );
    Ok(())
}

pub async fn isolate_bridge(bridge_name: &str, other_bridges: &[String]) -> Result<()> {
    for other in other_bridges {
        if other == bridge_name {
            continue;
        }

        add_drop_rule(bridge_name, other).await?;
        add_drop_rule(other, bridge_name).await?;
    }

    Ok(())
}

async fn add_drop_rule(from_bridge: &str, to_bridge: &str) -> Result<()> {
    run_nft(&[
        "add",
        "rule",
        "inet",
        TABLE_NAME,
        CHAIN_NAME,
        "iifname",
        from_bridge,
        "oifname",
        to_bridge,
        "drop",
    ])
    .await
}

pub async fn add_node_port_dnat(
    workload_id: &str,
    protocol: &str,
    node_port: u16,
    guest_ip: &str,
    container_port: u16,
) -> Result<()> {
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
        protocol,
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
    protocol: &str,
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
        protocol,
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
        &format!("\"{}-s3\"", bridge_name),
    ])
    .await
}

pub async fn allow_mesh_forwarding(bridge_name: &str, wg_iface: &str) -> Result<()> {
    let comment = format!("\"{}\"", mesh_comment(bridge_name));
    delete_rules_containing("ip", NAT_TABLE_NAME, SNAT_CHAIN_NAME, &comment).await?;
    run_nft(&[
        "insert",
        "rule",
        "ip",
        NAT_TABLE_NAME,
        SNAT_CHAIN_NAME,
        "iifname",
        bridge_name,
        "oifname",
        wg_iface,
        "accept",
        "comment",
        &comment,
    ])
    .await
}

fn mesh_comment(bridge_name: &str) -> String {
    format!("{bridge_name}-mesh")
}

pub async fn masquerade_bridge_traffic(bridge_name: &str) -> Result<()> {
    let comment = format!("\"{}\"", snat_comment(bridge_name));
    run_nft(&[
        "add",
        "rule",
        "ip",
        NAT_TABLE_NAME,
        SNAT_CHAIN_NAME,
        "iifname",
        bridge_name,
        "ct",
        "status",
        "dnat",
        "masquerade",
        "comment",
        &comment,
    ])
    .await?;
    run_nft(&[
        "add",
        "rule",
        "ip",
        NAT_TABLE_NAME,
        SNAT_CHAIN_NAME,
        "iifname",
        bridge_name,
        "oifname",
        "!=",
        bridge_name,
        "masquerade",
        "comment",
        &comment,
    ])
    .await
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

pub async fn remove_bridge_rules(bridge_name: &str) -> Result<()> {
    delete_rules_containing("inet", TABLE_NAME, CHAIN_NAME, bridge_name).await?;
    delete_rules_containing(
        "ip",
        NAT_TABLE_NAME,
        SNAT_CHAIN_NAME,
        &format!("\"{}\"", snat_comment(bridge_name)),
    )
    .await?;
    delete_rules_containing(
        "ip",
        NAT_TABLE_NAME,
        SNAT_CHAIN_NAME,
        &format!("\"{}\"", mesh_comment(bridge_name)),
    )
    .await
}

async fn delete_rules_containing(
    family: &str,
    table: &str,
    chain: &str,
    needle: &str,
) -> Result<()> {
    let output = Command::new("nft")
        .args(["-a", "list", "chain", family, table, chain])
        .output()
        .await
        .context("failed to list nftables rules")?;

    let listing = String::from_utf8_lossy(&output.stdout);
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

    #[tokio::test]
    async fn bridges_are_never_isolated_from_themselves() {
        isolate_bridge("csfxrg1", &["csfxrg1".to_string()])
            .await
            .unwrap();
        isolate_bridge("csfxrg1", &[]).await.unwrap();
    }

    use crate::testing::FakeTools;

    const SUCCEEDING: &str = "exit 0";
    const LISTING: &str = r#"case "$*" in *"list chain"*) printf '%s\n' 'iifname "br1" oifname "br2" drop # handle 4' 'iifname "br1" accept comment "br1-mesh" # handle 9' 'masquerade comment "br1-snat" # handle 11' 'dnat to 10.0.0.2:80 comment "w-1" # handle 15';; esac"#;

    #[tokio::test]
    async fn base_tables_and_chains_are_created_in_order() {
        let tools = FakeTools::install(&[("nft", SUCCEEDING)]);

        ensure_table_and_chain().await.unwrap();

        let calls = tools.calls("nft");
        assert_eq!(calls.len(), 9);
        assert_eq!(calls[0], "add table inet csfx");
        assert!(calls[1].starts_with("add chain inet csfx rg_isolation"));
        assert!(calls[2].starts_with("add chain inet csfx guest_mgmt_guard"));
        assert_eq!(calls[3], "flush chain inet csfx guest_mgmt_guard");
        assert_eq!(calls[6], "add table ip csfx_nat");
        assert!(calls[7].contains("node_port_dnat"));
        assert!(calls[8].contains("node_port_snat"));
    }

    #[tokio::test]
    async fn guests_are_blocked_from_the_management_mesh() {
        let tools = FakeTools::install(&[("nft", SUCCEEDING)]);

        ensure_table_and_chain().await.unwrap();

        let calls = tools.calls("nft");
        assert_eq!(
            calls[4],
            "add rule inet csfx guest_mgmt_guard iifname csfxrg* oifname wgmgmt0 drop"
        );
        assert_eq!(
            calls[5],
            "add rule inet csfx guest_mgmt_guard iifname csfxrg* ip daddr 10.100.0.0/16 drop"
        );
    }

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

    #[tokio::test]
    async fn isolation_adds_a_drop_rule_in_both_directions() {
        let tools = FakeTools::install(&[("nft", SUCCEEDING)]);

        isolate_bridge("br1", &["br2".to_string(), "br3".to_string()])
            .await
            .unwrap();

        let calls = tools.calls("nft");
        assert_eq!(calls.len(), 4);
        assert!(calls
            .contains(&"add rule inet csfx rg_isolation iifname br1 oifname br2 drop".to_string()));
        assert!(calls
            .contains(&"add rule inet csfx rg_isolation iifname br3 oifname br1 drop".to_string()));
    }

    #[tokio::test]
    async fn node_port_rules_forward_to_the_guest() {
        let tools = FakeTools::install(&[("nft", SUCCEEDING)]);

        add_node_port_dnat("w-1", "tcp", 30080, "10.0.0.2", 80)
            .await
            .unwrap();
        add_rg_port_dnat("w-1", "udp", "10.0.0.1", 5353, "10.0.0.2", 53)
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
    async fn masquerade_covers_dnat_and_outbound_traffic() {
        let tools = FakeTools::install(&[("nft", SUCCEEDING)]);

        masquerade_bridge_traffic("br1").await.unwrap();

        let calls = tools.calls("nft");
        assert_eq!(calls.len(), 2);
        assert!(calls[0].contains("ct status dnat masquerade"));
        assert!(calls[1].contains("oifname != br1 masquerade"));
        assert!(calls
            .iter()
            .all(|call| call.ends_with("comment \"br1-snat\"")));
    }

    #[tokio::test]
    async fn mesh_forwarding_replaces_the_previous_rule() {
        let tools = FakeTools::install(&[("nft", LISTING)]);

        allow_mesh_forwarding("br1", "wg0").await.unwrap();

        let calls = tools.calls("nft");
        assert!(calls[0].starts_with("-a list chain ip csfx_nat node_port_snat"));
        assert_eq!(calls[1], "delete rule ip csfx_nat node_port_snat handle 9");
        assert!(calls[2]
            .starts_with("insert rule ip csfx_nat node_port_snat iifname br1 oifname wg0 accept"));
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
    async fn bridge_removal_clears_isolation_snat_and_mesh_rules() {
        let tools = FakeTools::install(&[("nft", LISTING)]);

        remove_bridge_rules("br1").await.unwrap();

        let deletes: Vec<_> = tools
            .calls("nft")
            .into_iter()
            .filter(|call| call.starts_with("delete rule"))
            .collect();
        assert!(deletes.contains(&"delete rule inet csfx rg_isolation handle 4".to_string()));
        assert!(deletes.contains(&"delete rule inet csfx rg_isolation handle 9".to_string()));
        assert!(deletes.contains(&"delete rule ip csfx_nat node_port_snat handle 11".to_string()));
        assert!(deletes.contains(&"delete rule ip csfx_nat node_port_snat handle 9".to_string()));
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
        ensure_table_and_chain().await.unwrap();
        masquerade_bridge_traffic("csfxrglab0").await.unwrap();

        run_nft(&["flush", "chain", "inet", TABLE_NAME, MGMT_GUARD_CHAIN_NAME])
            .await
            .unwrap();
        assert!(lab.guest_reaches("10.100.0.9"));
        assert!(lab.guest_reaches("10.201.0.2"));

        ensure_table_and_chain().await.unwrap();
        isolate_bridge("csfxrglab0", &["csfxrglab1".to_string()])
            .await
            .unwrap();
        assert!(!lab.guest_reaches("10.100.0.9"));
        assert!(!lab.guest_reaches("10.201.0.2"));
    }
}
