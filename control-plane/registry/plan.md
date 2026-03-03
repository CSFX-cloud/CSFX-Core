# Orchestrator System - Specification for Implementation

## 1. REGISTRY SERVICE - Functionality Specification

### Overview

The Registry is a Rust library (NOT a server) that manages node lifecycle. It's used by the API Gateway as shared state.

### Architecture

```
Agent → API Gateway (Axum Server) → Registry (Library) → Database (PostgreSQL + etcd)
```

### Core Responsibilities

#### 1.1 Node Registration

When an agent starts, it registers with the control plane:

Input:

- hostname (string)
- ip_address (string)
- capabilities (cpu_cores, total_ram_mb, total_disk_gb, features)

Process:

1. Check if node with same hostname already exists in database
2. If exists: treat as re-registration, update existing record
3. If new: create new node record with UUID
4. Set initial status to "healthy"
5. Set available_cpu = total_cpu, available_ram = total_ram
6. Set last_heartbeat = current timestamp
7. Save to PostgreSQL (persistent storage)
8. Save to etcd (fast lookup cache)
9. Publish NodeRegistered event

Output:

- node_id (UUID)
- Created timestamp

#### 1.2 Heartbeat Processing

Every 30 seconds, agents send heartbeat with current metrics:

Input:

- node_id (UUID)
- metrics (cpu_usage_percent, ram_used_mb, ram_available_mb, disk_usage)
- running_vms (list of VM IDs and their resource usage)

Process:

1. Update last_heartbeat timestamp
2. Update available_cpu and available_ram based on metrics
3. If heartbeat not received for >60 seconds: mark node as "offline"
4. Update in PostgreSQL
5. Update in etcd cache
6. If status changed: publish NodeStatusChanged event

#### 1.3 Node Queries

Provide fast lookup for scheduler and other services:

Functions needed:

- get_node(node_id) → Returns single node
- get_healthy_nodes() → Returns all nodes with status="healthy"
- get_nodes_by_resource_group(rg_id) → Returns nodes running VMs from specific RG
- get_node_capacity(node_id) → Returns available CPU/RAM/Disk

All queries check etcd cache first (fast), fall back to PostgreSQL if not found.

#### 1.4 Node Lifecycle States

```
States: healthy, degraded, offline, quarantined

Transitions:
- healthy → degraded: High resource usage (>90% CPU or RAM)
- healthy → offline: No heartbeat for >60 seconds
- healthy → quarantined: Security event detected
- offline → healthy: Heartbeat received again
- quarantined → healthy: Admin manually releases after remediation
```

### Data Storage Strategy

PostgreSQL (Source of Truth):

- All node records
- Audit log of all changes
- Historical data

etcd (Cache):

- Current node state only
- Fast reads for scheduler
- TTL: 5 minutes (auto-refresh on heartbeat)

---

## 2. SECURITY - Compromised Node Isolation

### 5-Layer Defense Model

#### Layer 1: Certificate-Based Identity

Mechanism:

- Each node gets unique mTLS certificate on registration
- Certificate CN (Common Name) = node_id
- Certificate TTL: 24 hours
- Auto-rotation: Every 12 hours agent requests new certificate
- Certificate Revocation List (CRL) maintained in database

When node compromised:

1. Security system revokes certificate immediately
2. Certificate added to CRL
3. CRL distributed to all nodes within 5 seconds
4. Compromised node's API calls rejected (mTLS fails)
5. Time to isolation: <5 seconds

Implementation:

- PKI service generates certificates
- API Gateway validates certificate on every request
- Agent auto-rotates certificate every 12 hours
- OCSP (Online Certificate Status Protocol) for real-time validation

#### Layer 2: Network Segmentation

Default Policy: DENY ALL

Allowed traffic:

- Control Plane ↔ Node: TCP 8443 (mTLS)
- Node ↔ Node: TCP 6789 (Ceph), TCP 49152-49200 (VM migration)
- Node ↔ Node: UDP 4789 (VXLAN overlay)
- Node → Internet: TCP 80/443 (package repos only, via proxy)

Denied traffic:

- Node → Other Resource Groups (enforced by SDN)
- Node → Management Network (except API Gateway)
- Node → Internet (except allowed)

Firewall rules enforced at:

- Node level (nftables/eBPF)
- Network switch level (ACLs)
- SDN level (VXLAN isolation)

Quarantine mode:

- ALL traffic dropped except forensics port
- Forensics port: SSH on isolated VLAN for investigation
- No restoration of traffic until admin approval

#### Layer 3: Least Privilege

What nodes CAN do:

- Report metrics via heartbeat
- Request VM specifications for assigned VMs only
- Mount assigned RBD volumes only
- Join assigned VXLAN networks only
- Write status updates for their own VMs

What nodes CANNOT do:

- Schedule workloads (only Control Plane can)
- Access data from other nodes
- Read secrets (must request from Vault per-VM)
- Modify cluster configuration
- Access control plane databases
- Revoke other nodes' certificates

Implementation:

- RBAC: Each node certificate mapped to limited service account
- Service account permissions: read assigned VMs, write own status only
- Immutable infrastructure: NixOS with read-only /nix/store
- No persistent secrets on nodes
- Secrets from Vault with 15-minute TTL, rotated on every VM start
- SELinux/AppArmor profiles enforcing process isolation
- Seccomp filters limiting syscalls

#### Layer 4: Continuous Monitoring

Host-based Intrusion Detection (HIDS):

- Falco: Runtime security monitoring, syscall analysis
- AIDE/Tripwire: File integrity checks every hour
- Auditd: Kernel-level audit logging

Network-based Intrusion Detection (NIDS):

- Suricata: Deep packet inspection, protocol analysis
- Zeek: Network traffic analysis
- Both signature-based and anomaly-based detection

Behavioral Analytics:

- Baseline normal behavior for each node (first 7 days)
- ML models detect deviations: unusual API calls, CPU spikes, off-hours activity
- Threat intelligence feeds for known malicious IPs
- Correlation across multiple signals

SIEM (Security Information and Event Management):

- Centralized logging (ELK or Splunk)
- Real-time correlation of events
- Automated alerting on suspicious patterns

Detection Triggers (any 3 = auto-quarantine):

- Unexpected API calls to unauthorized endpoints
- Certificate validation failures
- Unusual network traffic patterns (port scans, beaconing)
- Failed authentication attempts spike (>10 in 1 minute)
- File integrity violations (/etc modified, unexpected binaries)
- Kernel module loading (not via package manager)
- Privilege escalation attempts
- Communication with unknown/blacklisted IPs

Automated Response Pipeline:

1. Detect: HIDS/NIDS alerts → SIEM
2. Analyze: Correlation engine scores threat (0-100)
3. Quarantine: Score ≥75 triggers auto-quarantine
4. Notify: Alert security team via PagerDuty/Slack
5. Forensics: Capture memory dump, disk snapshot
6. MTTR Target: <2 minutes from detection to quarantine

#### Layer 5: Data Isolation

Volume Encryption:

- Each VM has dedicated Ceph RBD volume
- LUKS/dm-crypt encryption at rest
- Encryption keys stored in HashiCorp Vault (NOT on nodes)
- Key hierarchy: Master key → Volume-specific key

Key Management:

- Node requests volume key from Vault when VM starts
- Vault validates: node cert + VM-to-node assignment
- Vault provides time-limited key (5-minute TTL)
- Node mounts volume, key discarded from memory after mount
- Key rotation: Every 90 days automatically

Encryption Layers:

- At rest: LUKS on RBD volumes
- In transit: mTLS for all network communication
- In memory: VM memory encrypted (if hardware supports)

Isolation guarantees:

- Node can only mount volumes assigned to its VMs
- Control Plane validates assignment before providing key
- Compromised node cannot decrypt other nodes' volumes
- Even with physical disk access, data is encrypted

### Compromise Scenario - Full Timeline

Example: Node 3 is compromised

T+0s: Attacker gains root access on Node 3

- Compromise occurs via kernel exploit

T+2s: Falco detects privilege escalation

- Alert: "Unexpected process running as root"
- Score: +30

T+3s: AIDE detects /etc/passwd modification

- Alert: "Critical system file modified"
- Score: +35 (total: 65)

T+4s: Auditd logs suspicious kernel module load

- Alert: "Non-standard kernel module loaded"
- Score: +15 (total: 80)

T+5s: SIEM correlation threshold reached (score ≥75)

- Automatic quarantine initiated

T+5.1s: Certificate revocation

- Node 3's certificate added to CRL
- CRL pushed to all nodes
- All future API calls from Node 3 rejected

T+5.5s: Network isolation

- Firewall rules updated: DENY ALL for Node 3
- SDN controller removes Node 3 from all VXLANs
- Node 3 network-isolated

T+6s: Scheduler notified

- Node 3 removed from available nodes
- VMs marked for migration

T+10s: Failover Controller starts VM migration

- Gets list of VMs on Node 3
- Selects target nodes (Node 1, Node 2)
- Sends migration commands

T+15s: VMs migrating

- VM-A: RBD volume re-attached on Node 1
- VM-B: RBD volume re-attached on Node 2
- VMs start with same IPs (VXLAN seamless)

T+45s: All VMs migrated successfully

- System fully operational
- Node 3 completely isolated

Impact Assessment:

- Other nodes: ✅ Unaffected (cert revocation)
- Other Resource Groups: ✅ Completely isolated (SDN)
- User VMs: ✅ Migrated safely (RBD volumes intact)
- Data: ✅ No loss (encrypted, keys in Vault)
- Attacker: ❌ Trapped, cannot pivot, cannot read data, cannot reach Control Plane

---

## 3. BOOTSTRAPPING - First Cluster Setup

### The Challenge

Chicken-and-egg problem:

- Control Plane needs to run as containers
- Containers need to be orchestrated
- But there's no orchestrator yet

### Solution: Three-Phase Bootstrap

#### Phase 1: Manual Bootstrap (One-Time Setup)

Prerequisites:

- 3 physical servers with NixOS installed
- Network connectivity between servers
- ISO built with agent binary

Steps:

1. Boot first 3 nodes from NixOS ISO
2. Agent installed automatically during install
3. Agent starts in "bootstrap mode" (no Control Plane available yet)

Bootstrap mode behavior:

- Agent listens on local port 9999 (bootstrap API)
- Accepts commands from localhost only
- Can create containers without Control Plane
- Does NOT send heartbeats (no Control Plane yet)

#### Phase 2: Deploy Control Plane

On Node 1, run bootstrap script:

```
./deployments/bootstrap/bootstrap.sh
```

Script does:

1. Create docker-compose.yml with all control plane services
2. Start containers on Node 1:
   - api-gateway (port 8443)
   - scheduler
   - failover-controller
   - postgres (port 5432)
   - etcd (port 2379)
3. Wait for services to be healthy (health checks)
4. Initialize database schema (run migrations)
5. Create initial admin user

Node 1 now has:

- NixOS (host OS)
- Agent (running in bootstrap mode)
- Control Plane containers (running via docker-compose)

#### Phase 3: Agent Takeover

Once Control Plane is running:

1. Agent detects Control Plane availability:
   - Try to connect to https://localhost:8443/health
   - If successful: switch from "bootstrap mode" to "normal mode"

2. Agent registers itself:
   - POST /api/v1/nodes/register
   - Receives node_id and certificate
   - Saves to disk

3. Agent discovers Control Plane containers:
   - Lists running containers
   - Tags them with label: system=control-plane
   - Registers them in database as "protected"

4. Repeat for Node 2 and Node 3:
   - SSH to Node 2, run: docker-compose up (subset of services)
   - SSH to Node 3, run: docker-compose up (subset of services)
   - Agents register themselves
   - Cluster now has HA

Final state:

- Node 1: Agent + api-gateway + scheduler + postgres
- Node 2: Agent + api-gateway + failover-controller + etcd
- Node 3: Agent + api-gateway + sdn-controller + etcd

### Control Plane Distribution

Recommended distribution for 3 control plane nodes:

Node 1 (Primary):

- api-gateway
- scheduler
- postgres (primary)
- etcd (member 1)

Node 2 (Secondary):

- api-gateway
- failover-controller
- postgres (replica)
- etcd (member 2)

Node 3 (Secondary):

- api-gateway
- sdn-controller
- volume-manager
- etcd (member 3)

Load balancing:

- api-gateway on all 3 nodes
- DNS/HAProxy points to all 3 IPs
- If one fails, traffic routes to others

### Bootstrap Protection

Once bootstrapped, prevent accidental deletion:

1. Container labels:
   - All control plane containers: label "system=control-plane"
   - Agent checks label before stopping container
   - Refuses to stop system containers

2. Database flag:
   - Nodes table has "is_control_plane" boolean
   - Scheduler never schedules user VMs on control plane nodes
   - Prevents resource exhaustion

3. Firewall protection:
   - Control plane nodes have stricter firewall
   - Only accept connections from known nodes
   - Rate limiting on API endpoints

### Recovery Scenarios

If Node 1 (with postgres primary) dies:

1. Postgres replica on Node 2 promoted to primary
2. etcd quorum maintained (2/3 nodes)
3. api-gateway still available (Node 2, Node 3)
4. System continues operating
5. When Node 1 returns: postgres rejoins as replica

If all control plane nodes die:

1. Worker nodes continue running existing VMs (autonomous)
2. No new VMs can be scheduled (no scheduler)
3. Bring up any control plane node
4. Once postgres+etcd restored: full functionality returns
5. State recovered from database (persistent)

---

## 4. DATA FLOW EXAMPLES

### Example 1: Agent Registration

1. Agent starts on worker-node-1
2. Agent loads config: control_plane_url = "https://api-gateway:8443"
3. Agent sends POST /api/v1/nodes/register with hostname, IP, capabilities
4. API Gateway receives request
5. API Gateway calls registry.register_node(hostname, ip, capabilities)
6. Registry checks PostgreSQL for existing node with same hostname
7. Not found, creates new node with UUID, status="healthy"
8. Registry saves to PostgreSQL
9. Registry saves to etcd with key "nodes/{uuid}"
10. Registry returns node object to API Gateway
11. API Gateway generates certificate for node
12. API Gateway returns node_id + certificate
13. Agent saves node_id to /var/lib/orchestrator/node-id
14. Agent saves certificate to /var/lib/orchestrator/cert.pem
15. Agent switches to normal mode, starts heartbeat loop

### Example 2: Heartbeat Processing

1. Agent (every 30s) collects metrics: CPU, RAM, disk
2. Agent sends POST /nodes/{node_id}/heartbeat with metrics
3. API Gateway receives request
4. API Gateway validates mTLS certificate (CN matches node_id)
5. API Gateway calls registry.update_heartbeat(node_id, metrics)
6. Registry updates PostgreSQL: last_heartbeat = now(), available_cpu/ram from metrics
7. Registry updates etcd cache
8. Registry returns success
9. API Gateway returns 200 OK
10. Agent continues loop

### Example 3: Node Goes Offline

1. Node 5 network failure (or crash)
2. Heartbeat stops arriving
3. After 60 seconds, health checker task runs (background task in registry)
4. Health checker sees last_heartbeat is 65 seconds old
5. Health checker updates node status = "offline"
6. Health checker publishes NodeOffline event
7. Failover Controller receives event
8. Failover Controller queries: which VMs on Node 5?
9. Failover Controller triggers migration for each VM
10. Scheduler finds new nodes for VMs
11. Agents on new nodes receive start_vm commands
12. VMs start on new nodes with same RBD volumes
13. Network (VXLAN) updated to point to new nodes
14. VMs running again, user traffic restored

---

## 5. CONFIGURATION

### Registry Configuration

No separate config needed - Registry is a library used by API Gateway.

API Gateway config (/etc/orchestrator/api-gateway.toml):

```
[database]
url = "postgres://orchestrator:password@postgres:5432/orchestrator"
max_connections = 100

[etcd]
endpoints = ["http://etcd:2379"]
timeout_seconds = 5

[security]
ca_cert_path = "/etc/orchestrator/ca.pem"
server_cert_path = "/etc/orchestrator/server.pem"
server_key_path = "/etc/orchestrator/server-key.pem"

[registry]
heartbeat_timeout_seconds = 60
health_check_interval_seconds = 10
```

### Agent Configuration

/etc/orchestrator/agent.toml:

```
[control_plane]
url = "https://api-gateway:8443"
heartbeat_interval_seconds = 30
command_poll_interval_seconds = 5

[node]
hostname = "worker-node-1"  # auto-detect if not set
ip_address = "192.168.1.10"  # auto-detect if not set

[security]
cert_path = "/var/lib/orchestrator/cert.pem"
key_path = "/var/lib/orchestrator/key.pem"
ca_path = "/etc/orchestrator/ca.pem"
cert_rotation_hours = 12

[executor]
firecracker_binary = "/usr/bin/firecracker"
kernel_path = "/var/lib/vmlinux"
```

---

## 6. IMPLEMENTATION PRIORITIES

Phase 1 - Core Registry (Week 1):

- Node registration function
- Heartbeat update function
- Node query functions (get_node, get_healthy_nodes)
- PostgreSQL storage
- etcd cache

Phase 2 - Security Basics (Week 2):

- mTLS certificate generation
- Certificate validation in API Gateway
- Basic RBAC (node can only access own data)

Phase 3 - Monitoring (Week 3):

- Health checker background task
- Status transitions (healthy → offline)
- Event publishing (NodeRegistered, NodeOffline)

Phase 4 - Advanced Security (Week 4):

- Certificate rotation
- CRL (Certificate Revocation List)
- Quarantine mode

Phase 5 - Bootstrap (Week 5):

- Bootstrap script
- Agent bootstrap mode
- Control plane container protection

This specification should be sufficient to implement the registry service, security layers, and bootstrap process without needing code examples.
