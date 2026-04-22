# CSFX Updater — Architekturplan

## Aktueller Stand (vollständig analysiert)

### CI/CD Pipeline

**GitHub Actions Workflows:**
- `release-please.yml`: Läuft auf `main` — erstellt automatisch GitHub Releases via Conventional Commits, bumped `Cargo.toml` workspace version, aktuell bei `0.2.2`
- `docker-build.yml`: Trigggert nach erfolgreichem Release-Please-Run **oder** `workflow_dispatch` **oder** `push` auf `develop`
  - Matrix-Build: 6 Services × 2 Architekturen (amd64 + arm64) via native GitHub Runners (`ubuntu-latest` + `ubuntu-24.04-arm`)
  - Build-Strategie: `push-by-digest` → separater `manifest`-Job erstellt Multi-Arch-Manifest
  - Images landen auf `ghcr.io/<org>/csfx-ce-<service>:<version>` + `:latest`
  - Dockerfile: `control-plane/Dockerfile.prod.shared` mit `cargo-chef` für Layer-Caching
  - `build-binaries`-Job: baut `csfx-updater` und `csfx-agent` als statische musl-Binaries (amd64 + arm64)
  - `attach-binaries-release`-Job: uploaded Binaries + SHA256-Dateien zum GitHub Release
- `prerelease.yml`: Identischer Flow für `develop`-Branch → Pre-release mit `<version>-alpha.<commit-count>` Tag
- `lint.yml`: `cargo clippy -D warnings` + `cargo fmt --check` + `cargo audit` auf PRs und `main`
- `renovate.yml`: automatische Dependency-Updates (vermutlich)

**Dockerfile-Struktur (`Dockerfile.prod.shared`):**
- Stage 1 (`planner`): `cargo chef prepare` — generiert `recipe.json`
- Stage 2 (`builder`): `cargo chef cook` (Dependency-Cache) + `cargo build --profile docker-release --bin <SERVICE_BIN> --bin csfx-migrate`
- Stage 3 (`runtime`): `debian:bookworm-slim`, beide Binaries (`/app/service` + `/csfx-migrate`) kopiert
- Build-Arg `CSFX_BUILD_VERSION` wird an den Build übergeben (für `build.rs`)

**`Dockerfile.csfx-updater`:**
- Separates Dockerfile nur für `csfx-updater`, exportiert Binary via `FROM scratch AS export`
- Wird nicht vom CI verwendet — CI baut `csfx-updater` als musl-Binary direkt via `cargo build`
- Dieses Dockerfile ist totes Deployment-Artefakt, das nicht mehr zum CI-Flow passt

### Runtime-Komponenten

**`csfx-updater` Binary** (`control-plane/csfx-updater/`):
- Pollt etcd alle N Sekunden auf `/csfx/config/desired_cp_version`
- Validiert Semver-Format, setzt `/csfx/config/last_update_result` als Statusindikator
- Lädt GHCR-Token verschlüsselt aus etcd (AES-256-GCM via `secret.rs`)
- Führt `docker compose pull` → Digest-Verify → `docker compose up -d` aus
- Digest-Verify: GHCR Registry API (remote) vs. `docker image inspect` (lokal) — aber `local_digest()` macht intern nochmal `docker pull`
- Wartet 15s pauschal, prüft dann `docker compose ps` auf unhealthy Services
- Downloadet `csfx-agent` und `csfx-updater` Binaries von GitHub Releases, verifiziert SHA256, swappt atomar via `rename(2)`
- Startet Units via `sudo systemctl restart <unit>`

**Shell-Fallback** (`deployments/systemd/csfx-updater.sh`):
- Identische Logik in Bash: etcd-Poll via curl + jq, docker-compose-Flow, Digest-Verify
- Kein Binary-Download, kein Self-Update
- Kein Health-Check nach up (nur `sleep 15` + `jq`-Filter)

**Systemd-Unit** (`deployments/systemd/csfx-updater.service`):
- `ExecStart` zeigt auf `csfx-updater.sh` (Shell-Script), nicht auf das Rust-Binary
- Fehlende Env-Var: `SECRET_ENCRYPTION_KEY` (vom Rust-Binary required, im Shell-Script nicht gebraucht)
- `ETCD_ENDPOINT` (Singular) statt `ETCD_ENDPOINTS` (Liste, wie Config erwartet)
- Kein Hardening: kein `ProtectSystem`, kein `NoNewPrivileges`, kein `CapabilityBoundingSet`
- User `csfx-updater` ist in Gruppe `docker` — kann alle Container auf dem Host steuern

---

## Probleme und Schwachstellen

### P1 — systemd-Unit startet Shell-Script statt Rust-Binary
`ExecStart=/opt/csfx/csfx-updater.sh` — das Rust-Binary wird gebaut, deployed, aber nie gestartet.
Das Secret-Handling (AES-256-GCM), das persistente etcd-RESULT_KEY-Schreiben und die SHA256-Verify laufen damit in Prod nie. Die Shell-Version hat keine Verschlüsselung und kein Binary-Download.

### P2 — sudo ohne sudoers-Regel bricht in Prod
`restart_unit()` ruft `sudo systemctl restart <unit>` auf. Der User `csfx-updater` hat keine sudoers-Regel — jeder Update-Cycle schlägt beim systemctl-Call fehl, ohne Rollback.

### P3 — Kein Rollback
Wenn `health_check()` einen unhealthy Service meldet, wird `RESULT_KEY` auf `failed` gesetzt und der Cycle endet. Die Services laufen weiterhin mit dem neuen (kaputten) Image. Kein `docker compose up -d` mit dem vorherigen Tag.

### P4 — Self-Update-Race
`update_self_binary()` downloaded das neue Binary und macht `systemctl restart csfx-updater`. Der eigene Prozess wird gekillt bevor er `RESULT_KEY = success` schreiben kann — jeder Self-Update-Cycle hinterlässt `in_progress` in etcd.

### P5 — `last_applied` nur im RAM
Nach Crash oder Restart versucht der Updater sofort wieder dieselbe Version zu applyen. Bei einem kaputten Setup → endloser Retry-Loop.

### P6 — 15s Sleep ist nicht deterministisch
`health_check()` wartet pauschal 15 Sekunden. Bei großen Images oder langsamen Nodes reicht das nicht. Bei schnellen Nodes ist es Verschwendung.

### P7 — Kein Distributed Lock
Wenn zwei Master-Nodes gleichzeitig denselben `desired_cp_version`-Key sehen, laufen beide gleichzeitig `docker compose up -d`. Kein Lock in etcd.

### P8 — Reines Polling, keine etcd-Watches
Der Updater reconnected zu etcd jede Poll-Iteration und macht ein synchrones GET. Ein etcd-Watch wäre reaktiver und ressourcenschonender.

### P9 — `local_digest()` macht internen zweiten `docker pull`
In `verify_images()` wird `docker pull --quiet` in `local_digest()` aufgerufen — obwohl `pull()` das Image bereits wenige Sekunden vorher gezogen hat. Verdoppelt die Download-Zeit.

### P10 — Agent-Binary-Update inkompatibel mit NixOS
`update_agent_binary()` schreibt nach `/usr/local/bin/csfx-agent` und startet `csfx-daemon` neu. Auf NixOS überlebt das Binary keinen `nixos-rebuild switch` — die systemd-Unit zeigt auf einen Nix-Store-Pfad, nicht auf `/usr/local/bin`. Der Ansatz funktioniert nur auf nicht-NixOS-Systemen.

### P11 — `Dockerfile.csfx-updater` ist orphaned
Das separate Dockerfile baut `csfx-updater` als statisches Binary, exportiert es via `FROM scratch`. Der CI-Flow (`docker-build.yml`) nutzt es nicht — er baut `csfx-updater` direkt via `cargo build --target musl`. Das Dockerfile ist toter Code und führt zu Verwirrung bei der Frage welcher Build-Pfad der kanonische ist.

### P12 — `update-versions.sh` referenziert `backend/Cargo.toml` das nicht existiert
Das Script in `.github/scripts/update-versions.sh` patcht `backend/Cargo.toml`. Das Projekt heißt aber `CSFX-Core` mit `Cargo.toml` im Root als Workspace. `backend/` existiert nicht. Das Script ist toter Code aus einem früheren Projekt-Layout.

### P13 — `csfx-updater` im selben `Dockerfile.prod.shared` wie Services
Der `build`-Job in `docker-build.yml` baut alle 6 Services mit `Dockerfile.prod.shared`. `csfx-updater` hat ein eigenes `Dockerfile.csfx-updater`. Der `build-binaries`-Job baut `csfx-updater` als musl-Binary. Drei verschiedene Build-Pfade für dasselbe Binary — unklar welcher kanonisch ist.

---

## Zielarchitektur

### Schicht 1 — Control Plane Updates (Docker-basiert)

```
GitHub Release v1.2.3
  → CI baut Images + musl-Binaries
  → Images auf ghcr.io/<org>/csfx-ce-<service>:1.2.3
  → Binaries als Release-Assets (csfx-agent-amd64, csfx-updater-amd64 etc.)
  → Admin setzt etcd: /csfx/config/desired_cp_version = "1.2.3"

etcd-Watch (kein Poll) triggert csfx-updater:
  1. acquire_lock (etcd Lease, 60s TTL) — verhindert parallele Updates
  2. pull images (alle 6 Services parallel via goroutines/tasks)
  3. verify digests (remote GHCR API vs lokaler docker inspect, KEIN zweiter pull)
  4. docker compose up -d --remove-orphans
  5. wait_healthy (Retry-Loop, 5s Interval, konfigurierbarer Timeout)
     → bei timeout: docker compose up -d mit PREV_VERSION (Rollback)
  6. release_lock
  7. put applied_cp_version = version, put last_update_result = success

bei Fehler in Schritt 4/5:
  8. docker compose up -d mit applied_cp_version (Rollback)
  9. put last_update_result = rolled_back
```

**etcd-Keys:**
```
/csfx/config/desired_cp_version     → Zielversion (Admin schreibt diesen Key)
/csfx/config/applied_cp_version     → zuletzt erfolgreich gerollte Version (persistentes last_applied)
/csfx/config/last_update_result     → in_progress | success | failed | rolled_back
/csfx/config/update_paused          → true/false (bereits implementiert)
/csfx/config/update_lock            → Distributed Lock (etcd Lease)
/csfx/config/ghcr_token             → AES-256-GCM verschlüsseltes Token (bereits implementiert)
/csfx/config/desired_agent_version  → Zielversion für csfx-agent (Registry liest, Heartbeat trägt aus)
```

### Schicht 2 — Agent-Updates

**NixOS-Nodes (Primärpfad):**
```
Registry liest desired_agent_version aus etcd
  → Heartbeat-Response: { desired_version: "1.2.3" }
  → Agent vergleicht mit env!("CARGO_PKG_VERSION") aus build.rs
  → wenn neuer: schreibe /var/lib/csfx-daemon/desired_version
  → triggere systemctl start csfx-agent-update.service (PolicyKit-Regel)
  → Oneshot-Unit führt nixos-rebuild switch aus
  → systemd startet csfx-daemon nach rebuild neu (neues Binary aus Nix-Store)
```

**Nicht-NixOS-Fallback:**
```
Agent:
  1. Download Binary in tmpfile (/var/lib/csfx-daemon/csfx-agent.new)
  2. verifiziere SHA256 gegen Release-Asset
  3. chmod 0o750
  4. rename(2) → atomarer swap nach /var/lib/csfx-daemon/csfx-agent
  5. exec() sich selbst (in-place restart, kein PID-Wechsel)
     bei exec()-Fehler: systemctl restart csfx-daemon via D-Bus (kein sudo)
```

Der `csfx-updater` ist nicht zuständig für Agent-Updates. Er schreibt nur `/csfx/config/desired_agent_version`. Die Verteilung läuft ausschließlich über den Heartbeat-Mechanismus.

### Schicht 3 — Self-Update des Updaters

Empfehlung: `csfx-updater` Self-Update entfernen.

Begründung: `csfx-updater` ist kein Service der laufend upgedatet werden muss. Er wird beim Aufsetzen eines neuen Nodes deployed (via NixOS-Modul oder Ansible). Neue Versionen des Updaters kommen mit dem nächsten Node-Provisioning. Der Self-Update-Race (P4) entfällt komplett.

Falls Self-Update doch gewünscht: `success` + `applied_cp_version` in etcd schreiben, **dann** Binary tauschen + Unit neustarten. Die neue Instanz liest `applied_cp_version` beim Start und überspringt die Version.

---

## Konkrete Änderungen (priorisiert)

### 1 — systemd-Unit auf Rust-Binary umstellen [blocking]
`ExecStart` von `csfx-updater.sh` auf `/usr/local/bin/csfx-updater` ändern.
`ETCD_ENDPOINT` → `ETCD_ENDPOINTS` (kommaseparierte Liste).
`SECRET_ENCRYPTION_KEY` als Env-Var ergänzen (aus `/opt/csfx/.env`).

### 2 — Persistentes `applied_version` in etcd [blocking]
Beim Start: `etcd.get(APPLIED_VERSION_KEY)` als initialen `last_applied`.
`APPLIED_VERSION_KEY` nach erfolgreichem Update schreiben.
Eliminiert idempotenten Retry-Loop nach Restart.

### 3 — Rollback-Logik in `updater.rs`
Vor Update: `prev_version = etcd.get(APPLIED_VERSION_KEY)`.
Nach fehlgeschlagenem health_check: `compose(cfg, &prev, docker_config_dir, &["up", "-d"])`.
`RESULT_KEY = "rolled_back"`.

### 4 — Health-Check: Retry-Loop statt pauschaler Sleep
```rust
let timeout = Duration::from_secs(cfg.health_check_timeout_secs);
let deadline = Instant::now() + timeout;
loop {
    if all_healthy(cfg, version).await? { return Ok(()); }
    if Instant::now() > deadline { bail!("health check timeout"); }
    sleep(Duration::from_secs(5)).await;
}
```
Neues Config-Feld: `health_check_timeout_secs` (Default: 120).

### 5 — `local_digest()`: internen Pull entfernen
`local_digest()` soll nur `docker image inspect` aufrufen. Der Pull ist bereits in `pull()` passiert.
Wenn `inspect` fehlschlägt → bail, nicht erneut pullen.

### 6 — Distributed Lock in `etcd.rs`
```rust
pub async fn acquire_lock(&mut self, ttl_secs: i64) -> Result<i64>  // returns lease_id
pub async fn release_lock(&mut self, lease_id: i64) -> Result<()>
```
`acquire_lock` nutzt `etcd_client::Client::lease_grant` + `put` mit `LeaseId` auf `LOCK_KEY`.
Vor jedem Update-Cycle: lock acquiren. Bei Fehler (Lock bereits gehalten): `info!` + skip (kein Fehler).

### 7 — etcd-Watch in `main.rs`
etcd-Client hält eine persistente Verbindung, `watch()` auf `DESIRED_VERSION_KEY`.
Fallback-Poll alle 5 Minuten (Watch kann bei Netzwerkproblemen abreißen).
Eliminiert das unnötige Reconnect bei jedem Poll-Cycle.

### 8 — sudoers-Datei oder D-Bus-Restart
Einfachste Lösung: `/etc/sudoers.d/90-csfx-updater`:
```
csfx-updater ALL=(root) NOPASSWD: /usr/bin/systemctl restart csfx-daemon
```
Dieses File muss Teil des NixOS-Moduls / Deployment-Skripts sein.
Mittelfristig: `zbus`-Crate für D-Bus-nativen systemd-Unit-Restart ohne sudo.

### 9 — Self-Update aus `updater.rs` entfernen
`update_agent_binary()` und `update_self_binary()` aus `updater::run()` entfernen.
Agent-Updates laufen via Heartbeat-Response (Schicht 2).
Updater-Updates laufen via Node-Provisioning.

### 10 — `Dockerfile.csfx-updater` entfernen
Totes Artefakt — CI nutzt es nicht. Verursacht Verwirrung über den kanonischen Build-Pfad.
Kanonisch ist `build-binaries`-Job in `docker-build.yml` (musl, statisches Binary).

### 11 — `update-versions.sh` fixen oder entfernen
Script referenziert `backend/Cargo.toml` (existiert nicht). Versioning läuft über `release-please` + `Cargo.toml` workspace. Script ist funktionslos, sollte entfernt werden.

### 12 — NixOS-Modul: `csfx-agent-update.service` Oneshot-Unit
```nix
systemd.services.csfx-agent-update = {
  description = "CSFX Agent NixOS Update";
  serviceConfig = {
    Type = "oneshot";
    ExecStart = "${pkgs.nixos-rebuild}/bin/nixos-rebuild switch";
    User = "root";
  };
};
security.polkit.extraConfig = ''
  polkit.addRule(function(action, subject) {
    if (action.id === "org.freedesktop.systemd1.manage-units" &&
        action.lookup("unit") === "csfx-agent-update.service" &&
        subject.user === "csfx-daemon") {
      return polkit.Result.YES;
    }
  });
'';
```

---

## Was nicht geändert werden soll

- AES-256-GCM Secret-Handling (`secret.rs`) ist korrekt.
- Semver-Validierung in `main.rs` ist ausreichend.
- GHCR-Token-Exchange-Logik in `verify.rs` ist korrekt.
- `docker compose up -d --remove-orphans` ist der richtige Rolling-Restart-Mechanismus.
- Multi-Arch-Matrix-Build-Strategie (digest-first + manifest) in CI ist korrekt.
- `cargo-chef`-Layer-Caching in `Dockerfile.prod.shared` ist korrekt.
- `release-please` + Conventional Commits als Release-Trigger ist korrekt.
- SHA256-Verify + atomares `rename(2)` beim Binary-Swap ist korrekt.

---

## Deployment-Checkliste

```
[ ] systemd-Unit auf Rust-Binary umgestellt (ExecStart, ETCD_ENDPOINTS, SECRET_ENCRYPTION_KEY)
[ ] applied_cp_version Key beim Start geladen (persistentes last_applied)
[ ] applied_cp_version nach erfolgreichem Update in etcd geschrieben
[ ] Rollback-Logik in updater.rs (compose up mit prev_version bei health-check-Fehler)
[ ] Health-Check: Retry-Loop mit konfigurierbarem Timeout statt pauschalen 15s
[ ] local_digest() ohne internen docker pull Aufruf
[ ] Distributed Lock (acquire/release) in etcd.rs
[ ] etcd-Watch in main.rs (mit Fallback-Poll)
[ ] sudoers-Datei im Deployment oder D-Bus-basierter Restart
[ ] Self-Update (update_agent_binary, update_self_binary) aus updater::run() entfernt
[ ] Dockerfile.csfx-updater entfernt
[ ] update-versions.sh entfernt oder auf Workspace-Cargo.toml korrigiert
[ ] desired_agent_version in etcd schreiben (Admin-API oder Registry-Seite)
[ ] HeartbeatResponse: desired_version Feld ergänzen (Registry + Agent)
[ ] Agent: Version-Check + Update-Trigger (NixOS-Pfad + Fallback)
[ ] NixOS-Modul: csfx-agent-update.service Oneshot-Unit + PolicyKit-Regel
[ ] systemd-Unit Hardening (NoNewPrivileges, ProtectSystem, CapabilityBoundingSet)
```

---

## Nicht in Scope (bewusst ausgeschlossen)

- Watchtower: Dev-only, kein Digest-Verify, kein Rollback — nicht Prod-fähig
- Kubernetes-style Rolling Updates pro Replica: nicht relevant, Docker-Compose-Instanz pro Node
- Automatische Datenbankmigrationen im Updater: `csfx-migrate` Init-Container ist korrekt und bleibt getrennt
- Separate Version-Tracks pro Service: alle Services laufen auf derselben Workspace-Version
