# Changelog

All notable changes to CSFX-Core will be documented in this file.

## [0.2.3](https://github.com/CSFX-cloud/CSFX-Core/compare/v0.2.2...v0.2.3) (2026-05-16)


### Features

* 2FA ([c180085](https://github.com/CSFX-cloud/CSFX-Core/commit/c1800850a8b2a2b02160125d8b9ebdd88e9771a6))
* Add Azure-style marketplace with Docker resource management ([dda38e3](https://github.com/CSFX-cloud/CSFX-Core/commit/dda38e37a8dedaa3ed5719bde646f768d865966a))
* add binary self-update for csf-agent and csf-updater via github releases ([847dfe5](https://github.com/CSFX-cloud/CSFX-Core/commit/847dfe5b3b13a7764aae36715b751afb829e7142))
* add build-from-source support for development installations ([1e8a1a8](https://github.com/CSFX-cloud/CSFX-Core/commit/1e8a1a820d53601b5331a1f2f41218f214ab6b3f))
* add dev install helper script ([53e7314](https://github.com/CSFX-cloud/CSFX-Core/commit/53e73148b6e2e2067c5ac8c5b6c01ac5cd63a3af))
* Add full Docker container integration with auto-creation and control ([b00e239](https://github.com/CSFX-cloud/CSFX-Core/commit/b00e2396a9a47cb928eef43f2344e9d0f541f4b0))
* add patroni user, config template and entrypoint script ([ec78a2e](https://github.com/CSFX-cloud/CSFX-Core/commit/ec78a2e92a00c34ecfb990aaed17c47a0e11e951))
* Add self-monitoring service for automatic local agent data coll… ([1a1893c](https://github.com/CSFX-cloud/CSFX-Core/commit/1a1893c52e1cbe8b8c5658e307c71400055b3d67))
* Add self-monitoring service for automatic local agent data collection ([1e42c59](https://github.com/CSFX-cloud/CSFX-Core/commit/1e42c59d8d0a3dd0c9b0aa2786b1416164ce4273))
* addded ground setup for scheduler ([2cca6f1](https://github.com/CSFX-cloud/CSFX-Core/commit/2cca6f138a90471c505c1437696002f66bcb5679))
* added agend volume mount ([8549880](https://github.com/CSFX-cloud/CSFX-Core/commit/85498809cecb27e3a72946c1d841101dffd9f07c))
* added container placement in agent and scheduler ([f84c304](https://github.com/CSFX-cloud/CSFX-Core/commit/f84c3049580e96f7b2f2eabc98e5aaeb2d424d6b))
* added endpoints in api gateway ([c9a3563](https://github.com/CSFX-cloud/CSFX-Core/commit/c9a35630d38decf7a007b8c7747da4f17a83597c))
* added gh token for image pulling without rate limiting ([b62b2d1](https://github.com/CSFX-cloud/CSFX-Core/commit/b62b2d188762d01e489ad1c8a160db007c7797c6))
* added ha with patroni on postgres ([e6f6037](https://github.com/CSFX-cloud/CSFX-Core/commit/e6f603718c26cf52d283391bfd3e510fbd2c9763))
* added heartbeat logic to agent ([a54df53](https://github.com/CSFX-cloud/CSFX-Core/commit/a54df53dfd1fe58a691009477da5729dc75dd6f8))
* added heartbeat logic to agent ([04d3716](https://github.com/CSFX-cloud/CSFX-Core/commit/04d3716739e49107b53b4e8ee508dd402aa420d9))
* added LICENSE.txt ([82b587a](https://github.com/CSFX-cloud/CSFX-Core/commit/82b587ac1a66edce7b4877de3a311ae24b5d2085))
* added migrations for workloads ([964c20a](https://github.com/CSFX-cloud/CSFX-Core/commit/964c20a0c0d3d98fcf2e044ff38cc8605ddbddd7))
* added mtls encryption ([9692379](https://github.com/CSFX-cloud/CSFX-Core/commit/969237918f5ce7e7b861a8cca9df4dc48c14cd3f))
* added new alpha github run ([40b7af5](https://github.com/CSFX-cloud/CSFX-Core/commit/40b7af555a8e0e84a76929fe9b0425b45974bdaf))
* added new alpha github run ([1ec1a76](https://github.com/CSFX-cloud/CSFX-Core/commit/1ec1a765b8c92a8c67e07c6749087b5728a34fe1))
* added nix config for nix os master node ([8c3a866](https://github.com/CSFX-cloud/CSFX-Core/commit/8c3a8666973ac1c1cd06a0b1932af25f8842ee92))
* added OpenTelemetry Tracing ([2d11250](https://github.com/CSFX-cloud/CSFX-Core/commit/2d1125022cacd19c63e92d6e02cd0ec12a4896a5))
* added organization managment ([cd8dd46](https://github.com/CSFX-cloud/CSFX-Core/commit/cd8dd46d051bd3af54aacdc37d1ea56514339f56))
* added own patroni image ([f4b1938](https://github.com/CSFX-cloud/CSFX-Core/commit/f4b1938627f24038ce548ecb6676b9fc5f5844d0))
* added pki for agent ([73fd3ad](https://github.com/CSFX-cloud/CSFX-Core/commit/73fd3ad794e1392c4eee1f9d944d82e9d28f9fb4))
* added postgres as db and default admin user ([77ca3b9](https://github.com/CSFX-cloud/CSFX-Core/commit/77ca3b9051ba96f182bfb0780935e1f5a82b8574))
* added Prometheus Metrics and  Rate Limiting ([fbf9a48](https://github.com/CSFX-cloud/CSFX-Core/commit/fbf9a48b20f6f4a9113c3a3f80051827333e3f41))
* added rbac for all things ([76faa2a](https://github.com/CSFX-cloud/CSFX-Core/commit/76faa2a0127bd7735a8c4a96639377069fe7b5ad))
* added rbac for all things ([3b1be5b](https://github.com/CSFX-cloud/CSFX-Core/commit/3b1be5b3e2b3af6ea290d0f531df8856b87cf708))
* added renovate ([5d0b547](https://github.com/CSFX-cloud/CSFX-Core/commit/5d0b5478160f1749ee57eca875f1c67879daa917))
* added renovate via github actions ([18de04d](https://github.com/CSFX-cloud/CSFX-Core/commit/18de04de9b54478b7a459f65b286fba0175c0173))
* added ressource modell and workload specs ([50aac08](https://github.com/CSFX-cloud/CSFX-Core/commit/50aac08caafa6ce4fc0e6deb1eba06aedea4ebbf))
* added sdn controller for network ([59ace74](https://github.com/CSFX-cloud/CSFX-Core/commit/59ace74411e55b3ac65d922469fe3bd8ae893a64))
* added sdn controller for network ([49b5ed9](https://github.com/CSFX-cloud/CSFX-Core/commit/49b5ed90d558a112f6d1e7e6c8e247a2a81372f2))
* added update mech ([0a33b2b](https://github.com/CSFX-cloud/CSFX-Core/commit/0a33b2b513324ac0812b34d920b24274f638a325))
* added update stop and resume ([fd01b36](https://github.com/CSFX-cloud/CSFX-Core/commit/fd01b361aa31869dd27daadb7694e54f6265c844))
* added volume manager mount ([f0e3313](https://github.com/CSFX-cloud/CSFX-Core/commit/f0e3313b68e0c026ca6fd6ebeee41616cd23dce3))
* agent and physical server managment ([94919c6](https://github.com/CSFX-cloud/CSFX-Core/commit/94919c69405016d06c581177584846941712f78d))
* agent and server architecture ([e9c6877](https://github.com/CSFX-cloud/CSFX-Core/commit/e9c6877d8348388846d2e6ec36232b5d56a39946))
* agents with mtls ([bceb6e0](https://github.com/CSFX-cloud/CSFX-Core/commit/bceb6e0faa39f95eb3a02fc556ab60b6b835f3ca))
* allow PUBLIC_API_BASE_URL to be configured via environment variable ([9c355a7](https://github.com/CSFX-cloud/CSFX-Core/commit/9c355a71934ab9d3f0e3c607f65c46fd82bd5d8b))
* **api-gateway:** restrict system update endpoint to admin-only via RBAC ([988342b](https://github.com/CSFX-cloud/CSFX-Core/commit/988342b237539371555fcb2e97048bb8c28f39f0))
* auto-install build-essential, separate dev/prod installation logic ([9f933e6](https://github.com/CSFX-cloud/CSFX-Core/commit/9f933e65092a11c3e3926aaf368cd72c3d524b81))
* auto-install Rust/Cargo when building from source ([156d548](https://github.com/CSFX-cloud/CSFX-Core/commit/156d54808ffebe402a7292a14e2d1cf46ec04c74))
* automatically open firewall port 8000 for external access ([d2e7b10](https://github.com/CSFX-cloud/CSFX-Core/commit/d2e7b1052720ced1e88fe509430aa25a0cf14175))
* build csf-updater locally via nix and publish alpha binaries to github releases ([eb3df31](https://github.com/CSFX-cloud/CSFX-Core/commit/eb3df3135649fc4796ce05861a60c393c7ad942c))
* build csf-updater locally via nix and publish alpha binaries to… ([3d759e1](https://github.com/CSFX-cloud/CSFX-Core/commit/3d759e1091b9d3dedbc2bf6d98dac0b33d1ad4ae))
* cluster-wide bootstrap tokens ([b479ec6](https://github.com/CSFX-cloud/CSFX-Core/commit/b479ec65937ad0f4569346e2b4f921b0fd5d0a5f))
* **cp:** wire csfx-updater as systemd service and rename units ([5bd062e](https://github.com/CSFX-cloud/CSFX-Core/commit/5bd062e4c5941b2c49ce23c76b4f6a92895eb466))
* **csf-updater:** implement secure rust-based updater daemon with nixos integration ([eb065a0](https://github.com/CSFX-cloud/CSFX-Core/commit/eb065a03514fef1e4997da2e2169b0b783cb9de8))
* **csf-updater:** verify image digests against GHCR before applying update ([f791f42](https://github.com/CSFX-cloud/CSFX-Core/commit/f791f42a5fbbe9cd1330424d35539608cc384471))
* dashboard not working only test ([091289e](https://github.com/CSFX-cloud/CSFX-Core/commit/091289e8311d4cf0efe009799460a81e54702f40))
* dashboard not working only test ([cfe6edb](https://github.com/CSFX-cloud/CSFX-Core/commit/cfe6edb108d5b48f144122e06160211fd9b06a61))
* deamon install scripts with ports and other small changes ([3a45df0](https://github.com/CSFX-cloud/CSFX-Core/commit/3a45df016890fbc60cc5b65eda3ae1267302ff8e))
* docker compose prod ([d5ebd58](https://github.com/CSFX-cloud/CSFX-Core/commit/d5ebd58bebb3dcf32caa80570de00685bb44f982))
* docker logs and docker exec ([0062cf8](https://github.com/CSFX-cloud/CSFX-Core/commit/0062cf844cf6869c502b45fbcbd0b56213482f0d))
* entry point for schedueler in etcd cluster ([080225c](https://github.com/CSFX-cloud/CSFX-Core/commit/080225c57fb984d4c713a4593bd1b83a0c571b7d))
* etcd ([5c0720a](https://github.com/CSFX-cloud/CSFX-Core/commit/5c0720a531a0a729e7a7805eb91433e4bbd4e7d9))
* **etcd:** enable authentication and restrict access to csf service user ([ba2a887](https://github.com/CSFX-cloud/CSFX-Core/commit/ba2a88718a38deda7a82e515052456f741256052))
* expose agent and updater binary versions in update-status ([08030a9](https://github.com/CSFX-cloud/CSFX-Core/commit/08030a9bf54add66b3d8ad237a0b440cdd55ddc9))
* fix semantic release and binary builds ([ef6d085](https://github.com/CSFX-cloud/CSFX-Core/commit/ef6d085618ca36fa3d2db789805faffa07a0d203))
* ground setup ceph storage ([1ad3e67](https://github.com/CSFX-cloud/CSFX-Core/commit/1ad3e67a1d032af910b64b9a98ae649aff3b6620))
* ground setup failover controler ([806149f](https://github.com/CSFX-cloud/CSFX-Core/commit/806149feaee42ac62c174694f4bddbbb54f55bdf))
* ground setup failover controler ([47ea8b6](https://github.com/CSFX-cloud/CSFX-Core/commit/47ea8b647bdc621503a5a06e2afd80ed351c7a27))
* ground setup registry for agents ([9ed2f4a](https://github.com/CSFX-cloud/CSFX-Core/commit/9ed2f4a04d941fea35a3f1067c649db9bb512779))
* impl communcitaion and hearbeat ([d51fb91](https://github.com/CSFX-cloud/CSFX-Core/commit/d51fb91651189334c38132427d4c11da6af7accf))
* implement gitops poller, git mirror, and nix build pipeline in csf-updater ([d08736e](https://github.com/CSFX-cloud/CSFX-Core/commit/d08736eac471364bdbd3893d6004fa9d96c1bf8f))
* implement watchdog heartbeat counter in registry and csf-updater ([3024db4](https://github.com/CSFX-cloud/CSFX-Core/commit/3024db49e879db9b03123693ef321afde3d576a8))
* inject CSF_BUILD_VERSION into binaries at compile time via build.rs ([8d18efb](https://github.com/CSFX-cloud/CSFX-Core/commit/8d18efb8a2fdc7a73aa5f6c2aa7357800163bfc5))
* installer scripts ([8440210](https://github.com/CSFX-cloud/CSFX-Core/commit/8440210ee6cc1cab24b0acd3988e343557918ebd))
* lint pipeline and fix builds ([78f9dea](https://github.com/CSFX-cloud/CSFX-Core/commit/78f9dea6c3525efe6a9ad845d9a4545327f99fce))
* live update screen ([14bee30](https://github.com/CSFX-cloud/CSFX-Core/commit/14bee30b0c7fb9b8b0934e370f3914a36f900f74))
* live update screen ([879c62e](https://github.com/CSFX-cloud/CSFX-Core/commit/879c62efc32fbfbf97d4e2d97ac6ab3f0b7384de))
* new beta branch features ([b88b509](https://github.com/CSFX-cloud/CSFX-Core/commit/b88b509342da00aeea618ece55bc6d911ac543e5))
* new connection to db and refactor ([063bc84](https://github.com/CSFX-cloud/CSFX-Core/commit/063bc842d07926aa7bba3441a781bd9df5100f0a))
* new nix config ([26ea9cc](https://github.com/CSFX-cloud/CSFX-Core/commit/26ea9cc9449148345546c23e6d56487ef442b96b))
* new ressource group managment ([5b83cb7](https://github.com/CSFX-cloud/CSFX-Core/commit/5b83cb7fd37bf4ca632944cf35d2fc19b519153e))
* nix deployment for auto docker deploy ([fb3c2da](https://github.com/CSFX-cloud/CSFX-Core/commit/fb3c2da09b697c7631f4665a653780d34f95e3d2))
* nix os config deploy on test server ([b8b5aff](https://github.com/CSFX-cloud/CSFX-Core/commit/b8b5affb0a0170bfd833936c3dd0b1ea6d14259d))
* pre reg agent for zero trust ([85bd67a](https://github.com/CSFX-cloud/CSFX-Core/commit/85bd67af18bc070a2db463a1e44651c2562885b4))
* propagate desired_flake_rev via heartbeat response to agent update trigger ([97914a6](https://github.com/CSFX-cloud/CSFX-Core/commit/97914a6e052340d11571efce801b00d82b45583b))
* propagate post_update_heartbeats counter to agent for watchdog health check ([88c0051](https://github.com/CSFX-cloud/CSFX-Core/commit/88c005146f9afc59051f707160ca3d2eb1aff8bc))
* renam csf to csfx ([6888431](https://github.com/CSFX-cloud/CSFX-Core/commit/6888431b8a52a4827bc9493f7897534eb6b201f1))
* replace flake-rev API with version-based update scheduling ([64dfe62](https://github.com/CSFX-cloud/CSFX-Core/commit/64dfe62b5724c642b6b6d26d30fc6cb2bc6c48dd))
* setup ceph storage ([a8c2dc5](https://github.com/CSFX-cloud/CSFX-Core/commit/a8c2dc541ca7969112e797f950e5f6819f6de776))
* setup for etcd cluster ([af2db8d](https://github.com/CSFX-cloud/CSFX-Core/commit/af2db8d3777fa0090a646b3a122984f38df248bd))
* setup ground struc agent ([eeb12eb](https://github.com/CSFX-cloud/CSFX-Core/commit/eeb12eb7fbb7b00874158ec08386650502133864))
* updater for programm ([7b064b8](https://github.com/CSFX-cloud/CSFX-Core/commit/7b064b8255b34cde174a591e93c7c67604997f2c))
* use mutable binary paths for csf-agent and csf-updater to enable self-update ([64e4326](https://github.com/CSFX-cloud/CSFX-Core/commit/64e4326d174a440a911e8b46f024866dc607e2e8))


### Bug Fixes

* add ENV_FILE support and auto-detect ORIGIN for CORS, improve config.env ([7767d84](https://github.com/CSFX-cloud/CSFX-Core/commit/7767d84d336d009f1c96dda578e198a6ce46c81a))
* add patroni bootstrap script to create csf app user and database ([0e07850](https://github.com/CSFX-cloud/CSFX-Core/commit/0e07850de14f5ac35098d4acaa9e2248a5fae7b5))
* added docker compose and fix build in docker ([e24e7b9](https://github.com/CSFX-cloud/CSFX-Core/commit/e24e7b9e441289a6edee0c0e244ae5489fe1d413))
* added log for testing update flow ([8aaeef0](https://github.com/CSFX-cloud/CSFX-Core/commit/8aaeef0ff44c77e752e04c9dcc5fc0c5a57264bb))
* added log for testing updater ([2728d06](https://github.com/CSFX-cloud/CSFX-Core/commit/2728d06c157b95e2d9124fc2e449c9411075c978))
* added test file ([b949322](https://github.com/CSFX-cloud/CSFX-Core/commit/b9493227571d73c6166c4d43bf105e94471a8e20))
* added test file ([7b37dc1](https://github.com/CSFX-cloud/CSFX-Core/commit/7b37dc1ea34a2a4aefe54c6d348124ac8d99a640))
* added test file for updater ([8414f8b](https://github.com/CSFX-cloud/CSFX-Core/commit/8414f8b74c9d4b99bb5da9c9468e972a4ad38a3a))
* added test file for updater ([d3cc2f3](https://github.com/CSFX-cloud/CSFX-Core/commit/d3cc2f341347274e7f7d1ed2de48a7340a049d4a))
* agent ([b09048e](https://github.com/CSFX-cloud/CSFX-Core/commit/b09048e54df3c4e815f1c095e2af43cd71fc5eb2))
* agent bootstrap error ([013b2c8](https://github.com/CSFX-cloud/CSFX-Core/commit/013b2c899925240cbc667b88052d27bb2c536e7c))
* agent error ([a6b0bbe](https://github.com/CSFX-cloud/CSFX-Core/commit/a6b0bbe86e6de90e9bcab2a1df9a4bed74b33e55))
* **agent:** detect OS from /etc/os-release instead of sysinfo ([1d86d69](https://github.com/CSFX-cloud/CSFX-Core/commit/1d86d69e1afe067bb5fdb6507cc0e202ecc68681))
* **agent:** rename state dir from csfx-daemon to csfx-agent ([0ee1896](https://github.com/CSFX-cloud/CSFX-Core/commit/0ee18965f068a7e526fb64b87bdf3ffb75dcde4a))
* api gateway and regisrty ([68a9671](https://github.com/CSFX-cloud/CSFX-Core/commit/68a96718290e13b308a2f4e9512e10da1d8a7cc7))
* arm runner and manifest error ([394f159](https://github.com/CSFX-cloud/CSFX-Core/commit/394f1598824f9d8cda0fe243b090b5fbbd716425))
* arm runner and manifest error ([a1ad641](https://github.com/CSFX-cloud/CSFX-Core/commit/a1ad641c705482d52e592b1ff729dfcdbab958f5))
* auth probelm ([80ef776](https://github.com/CSFX-cloud/CSFX-Core/commit/80ef776441ffd6b20ae1d451500e2b38e1b4d770))
* backend as deamon service ([ca14d20](https://github.com/CSFX-cloud/CSFX-Core/commit/ca14d207a6254e2381b9ba7c8824aeb25dd4e5a9))
* backend compile error ([5db3510](https://github.com/CSFX-cloud/CSFX-Core/commit/5db351022e79e9629677ca05fec48a4403a86040))
* backend compile error ([cb476e5](https://github.com/CSFX-cloud/CSFX-Core/commit/cb476e55cb7c3b04c2981fae716a45c2bd208995))
* backend error ([f4c6023](https://github.com/CSFX-cloud/CSFX-Core/commit/f4c6023deb08ea68510b087426f7c3fb6e31303e))
* backend error ([cd69b8c](https://github.com/CSFX-cloud/CSFX-Core/commit/cd69b8c9d89faa11a7c12c6eb42262428f7e6777))
* backend frontend connection ([d12c054](https://github.com/CSFX-cloud/CSFX-Core/commit/d12c05416d5e8a44f05da5a6cc2addc9007992ca))
* backup dir error ([21811e7](https://github.com/CSFX-cloud/CSFX-Core/commit/21811e73fd73a349eec130b9cc37eb3f17869b95))
* backup dir error ([e36b67e](https://github.com/CSFX-cloud/CSFX-Core/commit/e36b67e0942df2cb2526becae260dabd77bd3148))
* build binarys with gh actions ([91bc5b9](https://github.com/CSFX-cloud/CSFX-Core/commit/91bc5b96ecaefd5d333d1bc4360d95ab84840cf7))
* build error ([9a2e6d2](https://github.com/CSFX-cloud/CSFX-Core/commit/9a2e6d2b2cd2d86343a0bcb161392a3d55d01b5f))
* build error ([78364e8](https://github.com/CSFX-cloud/CSFX-Core/commit/78364e809165d8f3b598279d08180ec5b95480af))
* build error  with updater hash ([839fd28](https://github.com/CSFX-cloud/CSFX-Core/commit/839fd2836017f3829bc997cea6e8adfe71934719))
* build in pipleine ([a7a0a1c](https://github.com/CSFX-cloud/CSFX-Core/commit/a7a0a1c0f29a0b59bf9f3aeba7b6ce604049d46c))
* bump nixpkgs to 25.05 for Cargo 1.85/edition2024 support ([ddb75f3](https://github.com/CSFX-cloud/CSFX-Core/commit/ddb75f3419e9dcc2fa3a724c96afb93f708893c7))
* cert issue ([f544e36](https://github.com/CSFX-cloud/CSFX-Core/commit/f544e36724fcdef4408c7c93ddd40c98bf3ffcc6))
* Change capabilities from Vec&lt;String&gt; to serde_json::Value in agents entity ([524a823](https://github.com/CSFX-cloud/CSFX-Core/commit/524a823157777a0e7a5af3ce406c6fa4b9565906))
* ci ([ceb3486](https://github.com/CSFX-cloud/CSFX-Core/commit/ceb3486dccc2bb34f8e83cf85d9f298845cd1c7e))
* ci ([2d5b689](https://github.com/CSFX-cloud/CSFX-Core/commit/2d5b689c3008f6dc210a43a7984278a4f54205ae))
* ci pipeline new setup with nix ([eaffdf5](https://github.com/CSFX-cloud/CSFX-Core/commit/eaffdf54134c3415b0e55e9e3e5bbcb7f2689adb))
* compile errors ([6cfe5ae](https://github.com/CSFX-cloud/CSFX-Core/commit/6cfe5aedb51effa4152975ede9438391948a5241))
* create .env file for frontend build with PUBLIC_API_BASE_URL ([d6050ed](https://github.com/CSFX-cloud/CSFX-Core/commit/d6050ed146e01ca8b384ebe7fd8529d5f06ac599))
* create csfx role and database during patroni bootstrap ([7d0486c](https://github.com/CSFX-cloud/CSFX-Core/commit/7d0486c8e6ac3ac890674804932aca399517de5e))
* **csf-updater:** run as dedicated system user with docker group instead of root ([22cf1e5](https://github.com/CSFX-cloud/CSFX-Core/commit/22cf1e5adabe3f5d23ee64ad7c383ce9cd5d55bb))
* **csf-updater:** validate version string from etcd before executing update ([13e9fdf](https://github.com/CSFX-cloud/CSFX-Core/commit/13e9fdf8630c7f5e3468d16fab26ff77809c60cd))
* dev bootstrap ([1547ee8](https://github.com/CSFX-cloud/CSFX-Core/commit/1547ee838877589f000946f0278a70baff698f26))
* docker access in updater error ([ce45cba](https://github.com/CSFX-cloud/CSFX-Core/commit/ce45cbab1f20aa43a0b2d64c45b72fa3b2979183))
* docker compose ([8fd8be6](https://github.com/CSFX-cloud/CSFX-Core/commit/8fd8be64d461b2679f4aa23aec807e92a9b4b820))
* docker container on nix ([a285a18](https://github.com/CSFX-cloud/CSFX-Core/commit/a285a1885b0bc0e98b6afddc2acb2e808febe52f))
* docker container on nix ([2aca464](https://github.com/CSFX-cloud/CSFX-Core/commit/2aca464388c5efb0acf330ac9de332b0da925b89))
* docker deployment ([949d19b](https://github.com/CSFX-cloud/CSFX-Core/commit/949d19b36cd3ebbde77340837bba2a01866ce31c))
* docker long build ([2372614](https://github.com/CSFX-cloud/CSFX-Core/commit/2372614b0f8a6a10ab2997d657cad211e8e53b98))
* docker prod build ([581a20d](https://github.com/CSFX-cloud/CSFX-Core/commit/581a20d4b0d8c938cbd541c099d509780d781fdc))
* docker start ([a247b64](https://github.com/CSFX-cloud/CSFX-Core/commit/a247b6453b18ac9c26138e37285047885e7ad3e4))
* docker updater error ([d8f7457](https://github.com/CSFX-cloud/CSFX-Core/commit/d8f7457e5412d5397f1b0cee250503c74fdb9f65))
* docker warn on linux kernel ([1de9a08](https://github.com/CSFX-cloud/CSFX-Core/commit/1de9a084cbbe5cec93fc2205415c3f1f5ab5b597))
* double vv in version ([506c289](https://github.com/CSFX-cloud/CSFX-Core/commit/506c2890855d2a2b000e8f87eef3bf56d7fc5ef3))
* double vv in version ([48065e5](https://github.com/CSFX-cloud/CSFX-Core/commit/48065e564a46bfa497fa61be1145437ec06d5415))
* error backup location ([6ae9411](https://github.com/CSFX-cloud/CSFX-Core/commit/6ae9411407ec437eb833fd1b0c92840abc12dbea))
* error backup location ([b3d0246](https://github.com/CSFX-cloud/CSFX-Core/commit/b3d024694be9c5aad6fb6e55af460c3757eb9f89))
* error with component ([f0e86e8](https://github.com/CSFX-cloud/CSFX-Core/commit/f0e86e804255239b087e98e08ac5e6d3ab754982))
* erros ([15623c3](https://github.com/CSFX-cloud/CSFX-Core/commit/15623c34830ff30d88b0d8cf844dec77dcd77245))
* etcd connection error ([1a44dc1](https://github.com/CSFX-cloud/CSFX-Core/commit/1a44dc18fdecb233a6c2aa06819508f2eef3564d))
* **etcd:** block etcd ports from external access via firewall rules in install script ([34e1cd2](https://github.com/CSFX-cloud/CSFX-Core/commit/34e1cd26ccd9fc8a40dd4f681ed5e9cff1e281cb))
* frontend build error ([afec643](https://github.com/CSFX-cloud/CSFX-Core/commit/afec64354d33c9e70cf32cee2483a03250c1b108))
* **gateway:** exempt registry routes from rate limiting ([c9f30e9](https://github.com/CSFX-cloud/CSFX-Core/commit/c9f30e96cc9f1a59bcbea6efc885597d1139cdbb))
* github pipeline ([791c518](https://github.com/CSFX-cloud/CSFX-Core/commit/791c518c193985d6233768285eb34356a633ddf9))
* github pipleine time ([f88850e](https://github.com/CSFX-cloud/CSFX-Core/commit/f88850e3e29fcc63556f8fa12c192ba6f8a54207))
* github pipleine time ([6e8e2cd](https://github.com/CSFX-cloud/CSFX-Core/commit/6e8e2cd25bd17c41db5aa6ad64d3d7519c17c809))
* gitignore ([d6c3dca](https://github.com/CSFX-cloud/CSFX-Core/commit/d6c3dcafc7f3aa28cf9c49ac9963d67797c9620f))
* ha for postgres with patroni ([078de22](https://github.com/CSFX-cloud/CSFX-Core/commit/078de2230c5fc93871bf0c1bd64e5933ce5ea7a4))
* handle 429 rate limit in agent registration with retry backoff ([31d3c29](https://github.com/CSFX-cloud/CSFX-Core/commit/31d3c29e80c596dc2395bdb9b178f1b3c8d527c6))
* image version ([cd9b47c](https://github.com/CSFX-cloud/CSFX-Core/commit/cd9b47ce0cbfb59292ba28fdfc173c5c0f2e9914))
* improve frontend build with better error handling and npm install ([f027a20](https://github.com/CSFX-cloud/CSFX-Core/commit/f027a20d6d0767d25d2ab0a30287c8eb1a07a512))
* improve Rust installation and build logic ([91d1c61](https://github.com/CSFX-cloud/CSFX-Core/commit/91d1c619ecccd46522bc63a64a7aa80eb2e8baf3))
* include production node_modules in frontend package and add download stats ([13d7460](https://github.com/CSFX-cloud/CSFX-Core/commit/13d746039901b4de70f02b9d99651d6b374965c3))
* install script pull ([63814d1](https://github.com/CSFX-cloud/CSFX-Core/commit/63814d1ab67b694bb94ae69176ba03c67793d7b9))
* installation fix ([709a676](https://github.com/CSFX-cloud/CSFX-Core/commit/709a676d041695c2e676da9949ef79e9cd4927e0))
* installation script ([ef58a23](https://github.com/CSFX-cloud/CSFX-Core/commit/ef58a23bbe86c955c4e5717d25bb7d9259fd315a))
* installation script ([03fcd72](https://github.com/CSFX-cloud/CSFX-Core/commit/03fcd72cb5a8491ffc412b85601e474b3602abc7))
* leader election ([28871eb](https://github.com/CSFX-cloud/CSFX-Core/commit/28871eb6b9fc98ee7a6792e618834baaf374f706))
* leader select ([e5d8867](https://github.com/CSFX-cloud/CSFX-Core/commit/e5d88678b3378da846b18ea045d179a57651a47f))
* Load config.env and copy .env to frontend during installation ([04f9c1e](https://github.com/CSFX-cloud/CSFX-Core/commit/04f9c1e857a889ab29389069c9eaafdb6af5e900))
* lock state when updater go into error ([28c0247](https://github.com/CSFX-cloud/CSFX-Core/commit/28c0247dabb89763f2191c4d8fe95854bf7f8663))
* make bootstrap registration idempotent by upserting on hostname ([98a935a](https://github.com/CSFX-cloud/CSFX-Core/commit/98a935a7f89bb901b5a3314854016e60a70c9950))
* manifest build ([dd5d522](https://github.com/CSFX-cloud/CSFX-Core/commit/dd5d522b846b34fb8243257201a84c45a6203c4a))
* marketplace only docker ([263b532](https://github.com/CSFX-cloud/CSFX-Core/commit/263b5321e7d5483ed91b2664fec39c081ce9d274))
* master node auto-bootstrap — self-register agent via admin API on first boot ([1d64a1f](https://github.com/CSFX-cloud/CSFX-Core/commit/1d64a1f205d642c5ae18d5fc67447735576247cc))
* merge errors ([3d808ae](https://github.com/CSFX-cloud/CSFX-Core/commit/3d808aed9b35da3f2f86aaa3f79a946256d899ea))
* metrics error agent ([4ac4ce8](https://github.com/CSFX-cloud/CSFX-Core/commit/4ac4ce842d18dfa0d281d83b59628855d8de7207))
* mtls handshake ([cbe31b3](https://github.com/CSFX-cloud/CSFX-Core/commit/cbe31b3765a60bb945bd5753b925236f0ab9042a))
* mtls heart beat ([93fb782](https://github.com/CSFX-cloud/CSFX-Core/commit/93fb78235d31544fe1950cbfbb27e078067735d2))
* mtls heart beat ([869170f](https://github.com/CSFX-cloud/CSFX-Core/commit/869170f2b51fc821bf2d64d5dfb4a7860446ab25))
* mtls issue ([26e0cb2](https://github.com/CSFX-cloud/CSFX-Core/commit/26e0cb21b0691c34388278cbc8ce251ec1d26146))
* mulitple docker builds ([9a857ca](https://github.com/CSFX-cloud/CSFX-Core/commit/9a857ca9ce05afc74da10988a8007865c3c94f04))
* mulitple docker builds ([6a55d51](https://github.com/CSFX-cloud/CSFX-Core/commit/6a55d51027477a7226915e7f4ef61a45a8013692))
* new structure project ([5280c7e](https://github.com/CSFX-cloud/CSFX-Core/commit/5280c7e562c9191c420353285a1e646657782a94))
* new test version ([109c775](https://github.com/CSFX-cloud/CSFX-Core/commit/109c7751eb3a874d7c69f845dade267e80771527))
* nix compile error ([8d6d32e](https://github.com/CSFX-cloud/CSFX-Core/commit/8d6d32e1178c28d31113a48b495659acb9eb61f2))
* nix compile error ([48d2bde](https://github.com/CSFX-cloud/CSFX-Core/commit/48d2bde073f248546886630edbe63fc4359e9603))
* nix config error ([6b5adf7](https://github.com/CSFX-cloud/CSFX-Core/commit/6b5adf761e94725978d2eae57de95ffbebcdb410))
* nix config with path ([1d80789](https://github.com/CSFX-cloud/CSFX-Core/commit/1d807891e75767847657cd8d8e4b213cfcfaa7f5))
* nix container version ([4e6da4e](https://github.com/CSFX-cloud/CSFX-Core/commit/4e6da4e81a8a4ab61db85f25c63db98ca64f85ff))
* nix error ([ebfdf55](https://github.com/CSFX-cloud/CSFX-Core/commit/ebfdf554ec6969fd612ff868502fb3847d167bf5))
* nix os config version ([8ed2d60](https://github.com/CSFX-cloud/CSFX-Core/commit/8ed2d60dc0a98474801ef8a1b3ad624917c2ca52))
* nixos test version updated ([0230d7f](https://github.com/CSFX-cloud/CSFX-Core/commit/0230d7f19c89cc2d043544485bc8c580e3cc10ba))
* nixos updater error ([2b46867](https://github.com/CSFX-cloud/CSFX-Core/commit/2b46867d09d908db5e7dc613883903e28e507046))
* nixos version ([64cfcec](https://github.com/CSFX-cloud/CSFX-Core/commit/64cfcecad0faf97ea62a51753df171cea5ccf6ba))
* node deduplication and cluster telemetry ([2259013](https://github.com/CSFX-cloud/CSFX-Core/commit/2259013cd36d995360cc79e35699132616174f48))
* path error in updater ([e10e08f](https://github.com/CSFX-cloud/CSFX-Core/commit/e10e08fa5884573e89b3a08b40f569055e0c1eb3))
* patroni bootstrap error ([5895235](https://github.com/CSFX-cloud/CSFX-Core/commit/5895235c801f11cba514de1d52d4d75d0cede78e))
* patroni internal error from bootstrap ([d52bc00](https://github.com/CSFX-cloud/CSFX-Core/commit/d52bc00e1fac28e20e18daeb7f62c7dde72b6b11))
* persistante update screen ([f43c476](https://github.com/CSFX-cloud/CSFX-Core/commit/f43c476d8926f475102f2de0eb48ca5c60c5f35f))
* pin rust 1.88.0 via rust-overlay for edition2024/time crate support ([2b6bc89](https://github.com/CSFX-cloud/CSFX-Core/commit/2b6bc897531313710e9021533aafa3fc5d8ebdcf))
* pipeline ([d01b1ee](https://github.com/CSFX-cloud/CSFX-Core/commit/d01b1ee1f238b6becbee40d73eaf4fc78418bd66))
* pipeline ([25a9442](https://github.com/CSFX-cloud/CSFX-Core/commit/25a944201cafbd55e6202f8fa47858ebf3445717))
* pipeline ([4f38260](https://github.com/CSFX-cloud/CSFX-Core/commit/4f38260edbe01e734ddb4ba45e131144b4164171))
* pipeline ([3f6b004](https://github.com/CSFX-cloud/CSFX-Core/commit/3f6b004bba89ba8a2637ff7ca74b43c4b7fba7d7))
* pipeline ([7a0154d](https://github.com/CSFX-cloud/CSFX-Core/commit/7a0154d9b71931db881783b179f599316d44ce9e))
* pipeline binary push beta releases ([07fa042](https://github.com/CSFX-cloud/CSFX-Core/commit/07fa0422b30350e9fc6ccf2031b4f659d0020797))
* pipeline binary push beta releases ([4ce046c](https://github.com/CSFX-cloud/CSFX-Core/commit/4ce046ce9d1d6480cf70413883dba7ccc3fecd48))
* pipeline build error ([ddbfc81](https://github.com/CSFX-cloud/CSFX-Core/commit/ddbfc81032daae687355fd3fba90b7ea7662a820))
* pipeline build error ([8007bc4](https://github.com/CSFX-cloud/CSFX-Core/commit/8007bc47a90f049421f4d0a7d420424bab969e03))
* pipeline docker build ([c7d94c1](https://github.com/CSFX-cloud/CSFX-Core/commit/c7d94c1818e948f3ded5bd4f12979017cbdabd03))
* pipeline docker build ([909cc1a](https://github.com/CSFX-cloud/CSFX-Core/commit/909cc1a4c8cf776d188191b52f9c7ce902bb5ff8))
* pipeline docker image ([5f91789](https://github.com/CSFX-cloud/CSFX-Core/commit/5f917898e25b514b505db32e7f97c28a6391ddab))
* pipeline docker image ([c5743de](https://github.com/CSFX-cloud/CSFX-Core/commit/c5743de52af8862013515bccc8aa9fb82267219e))
* pipeline time ([20e929c](https://github.com/CSFX-cloud/CSFX-Core/commit/20e929c0286b624f482d06863ee0397dfc10b897))
* pipeline time ([a642161](https://github.com/CSFX-cloud/CSFX-Core/commit/a64216117ad7600664d424d81989bb509f0020a2))
* pipleine ([bcdc605](https://github.com/CSFX-cloud/CSFX-Core/commit/bcdc605a9caf49ff3d00ed5759118d7813f8c44c))
* pipleine ([885eadd](https://github.com/CSFX-cloud/CSFX-Core/commit/885eadd3f0ec2154659070e901bc03c7c2f294d2))
* provide complete workspace structure to cargo-chef ([d1c2022](https://github.com/CSFX-cloud/CSFX-Core/commit/d1c20229585c112d90e915520bbb7cf61ef07222))
* provide complete workspace structure to cargo-chef ([5bef937](https://github.com/CSFX-cloud/CSFX-Core/commit/5bef937644c38dec3d3bb9dc39a4ef5df85c1268))
* redirect ([a763eee](https://github.com/CSFX-cloud/CSFX-Core/commit/a763eee26077b2b6164a92efd438313ec9952188))
* release pipeline for beat ([36da76b](https://github.com/CSFX-cloud/CSFX-Core/commit/36da76bc5d455446fe3cd9cf674326b91834a2be))
* release pipeline for beat ([a908b37](https://github.com/CSFX-cloud/CSFX-Core/commit/a908b3711c537ef0b3ceeb90fe6acb915fdb7945))
* remove NoNewPrivileges to allow sudo systemctl for binary restart ([ddc22b8](https://github.com/CSFX-cloud/CSFX-Core/commit/ddc22b83f374fae4c0e88c13b66158a6fd95f730))
* remove unicode characters causing bash errors ([deff5d9](https://github.com/CSFX-cloud/CSFX-Core/commit/deff5d90d0518f085d149685b39b0640637fdd8b))
* removed old scripts ([dc19cb5](https://github.com/CSFX-cloud/CSFX-Core/commit/dc19cb582571cf3515effb8ba122f23536e31a3a))
* renovate ([1b87f10](https://github.com/CSFX-cloud/CSFX-Core/commit/1b87f1045250da00cef55a31afbe23326b8eccf6))
* repo ([fa07190](https://github.com/CSFX-cloud/CSFX-Core/commit/fa07190eeb0a04c9d6087e4bb48c740a6545f63b))
* Resolve compilation errors in self_monitor ([efb2b3c](https://github.com/CSFX-cloud/CSFX-Core/commit/efb2b3cda56fca59806dc41c05d9e48b5a264599))
* resolve musl build failure for csf-updater binary ([50a89e8](https://github.com/CSFX-cloud/CSFX-Core/commit/50a89e88f6338073a52a272870213509c1001d82))
* restrict binary dir permissions and verify sha256 checksum on download ([2b3260f](https://github.com/CSFX-cloud/CSFX-Core/commit/2b3260fe91b3038cda9d7fb8edf9d8a498e6c643))
* reverse proxy on registry routet through api gateway ([6e2ce4f](https://github.com/CSFX-cloud/CSFX-Core/commit/6e2ce4fa34f145cf4cad3d966de33bc4fef13491))
* rm enity folder in every project ([dbd5178](https://github.com/CSFX-cloud/CSFX-Core/commit/dbd51781b76cb6bd793a361ba39ad40f8bb4f9dd))
* script error ([4b1b343](https://github.com/CSFX-cloud/CSFX-Core/commit/4b1b3436aa9b28bf90c8bd97ca1074b9ef1d9b28))
* securtity issue fix sha-1 to sha-256 ([4252495](https://github.com/CSFX-cloud/CSFX-Core/commit/42524951a5d53c7fe48de02abb7cec99d7ee0550))
* securtiy issues on agent registration ([228a81f](https://github.com/CSFX-cloud/CSFX-Core/commit/228a81f818b5a0519ac026ba71790fe713431010))
* self kill error updater and manuell fix updater error ([f17d096](https://github.com/CSFX-cloud/CSFX-Core/commit/f17d09653022c39061ad6b9c7648161c2ee56cb4))
* semantic release commit befor build ([9927644](https://github.com/CSFX-cloud/CSFX-Core/commit/99276446079e169853a7b2b7848a369b45d0f930))
* semantic release workflow ([a33727e](https://github.com/CSFX-cloud/CSFX-Core/commit/a33727edfad870e446c46ecf96158207aca885bb))
* semantic release workflow ([bbf0598](https://github.com/CSFX-cloud/CSFX-Core/commit/bbf0598ff105bfa982568907b1111be3496b178e))
* semantiv release versioning ([4b4ce16](https://github.com/CSFX-cloud/CSFX-Core/commit/4b4ce161a29b96531248f11b228a71d2cce0b950))
* service ([23ae2f0](https://github.com/CSFX-cloud/CSFX-Core/commit/23ae2f08acd405f6e452bd7fc983389b84f21573))
* settings token and other things ([fa0acf9](https://github.com/CSFX-cloud/CSFX-Core/commit/fa0acf93f37583cd2419f3d044b13292c14b1a3d))
* shared folder ([bc7ce08](https://github.com/CSFX-cloud/CSFX-Core/commit/bc7ce08c1399119c835e3120c03372672a6d0631))
* shared folder ([752f4df](https://github.com/CSFX-cloud/CSFX-Core/commit/752f4df9fd771d6a7380b9bc949378856ffefa78))
* single-node patroni+etcd, remove haproxy ([45952da](https://github.com/CSFX-cloud/CSFX-Core/commit/45952da7810a0b980e334d4eaf8f6493d994fb8e))
* small change for testing updater ([1f5509f](https://github.com/CSFX-cloud/CSFX-Core/commit/1f5509fe0c8dada44290e98502321e69ea97866c))
* ssh error for dev ([a6a4fc9](https://github.com/CSFX-cloud/CSFX-Core/commit/a6a4fc9bcfceca80bb3a10dae50ff3cd7b389e5e))
* strip digest newlines to prevent invalid image reference in manifest ([fdd4d7e](https://github.com/CSFX-cloud/CSFX-Core/commit/fdd4d7e4b4cba4b5acfb861f2071c050fa627a09))
* structure porject fix ([8d40d6a](https://github.com/CSFX-cloud/CSFX-Core/commit/8d40d6aa329ed175a41e38dcbf4c6aae1c55bd86))
* structure project ([d2d83f1](https://github.com/CSFX-cloud/CSFX-Core/commit/d2d83f1bb0f6a1a27ace1213d98d7bac2879a949))
* support multiple CORS origins from ORIGIN env variable ([b96b530](https://github.com/CSFX-cloud/CSFX-Core/commit/b96b530796cdd5b6a5f76aaca1c0020d5963ea0c))
* swagger ui ([54113f4](https://github.com/CSFX-cloud/CSFX-Core/commit/54113f4d6ef39718123f1f0978a18b527d0d2628))
* swagger ui ([033f790](https://github.com/CSFX-cloud/CSFX-Core/commit/033f790c400efc7981de8c150f88136139c6ba74))
* test file for updater ([472f282](https://github.com/CSFX-cloud/CSFX-Core/commit/472f282ae756db5762aea6911de03be5fdc6ed8d))
* test file for updater ([0927186](https://github.com/CSFX-cloud/CSFX-Core/commit/0927186d706062c85eebe48c35a11e3db3073357))
* test file updater ([d9a485a](https://github.com/CSFX-cloud/CSFX-Core/commit/d9a485a4a6a582fbbf7b3ee858f2dd7502135c8b))
* test file updater ([6e55d23](https://github.com/CSFX-cloud/CSFX-Core/commit/6e55d23e61a81b7ad5696f046a168a87bc4f6716))
* test file updater ([0cdebfb](https://github.com/CSFX-cloud/CSFX-Core/commit/0cdebfb25d5d9dcb6d9de51a9f76600cdf173d0d))
* test file updater ([398adf1](https://github.com/CSFX-cloud/CSFX-Core/commit/398adf17716f012109360cb81b29e64d596a40bc))
* test file updater ([7f45bbc](https://github.com/CSFX-cloud/CSFX-Core/commit/7f45bbc1fa1b5c231723137224eb2133f6832b9a))
* test file updater ([666f334](https://github.com/CSFX-cloud/CSFX-Core/commit/666f334b08076d832e2c9cd04345cb654f70206b))
* test file updtaer ([b7241fc](https://github.com/CSFX-cloud/CSFX-Core/commit/b7241fc938b17e70e2571e3d423b5d3a1d78c233))
* test file updtaer ([aae1373](https://github.com/CSFX-cloud/CSFX-Core/commit/aae1373ec9b9649fcddfdbc2345286eaeb14af17))
* token route for dev ([97d0776](https://github.com/CSFX-cloud/CSFX-Core/commit/97d07769537cf0a119a860a2a8cea646970cf9e8))
* update permission error ([1d5a1c1](https://github.com/CSFX-cloud/CSFX-Core/commit/1d5a1c1cd82ff13209d4b60f1fb5f093c98e198d))
* update permission error ([f7b57ec](https://github.com/CSFX-cloud/CSFX-Core/commit/f7b57ec497bf172d9a68e17af401e4bc156fdd26))
* update script added to installation ([0392447](https://github.com/CSFX-cloud/CSFX-Core/commit/03924476948d0b649fe942c69e97225da79e9221))
* update script added to installation ([385c30e](https://github.com/CSFX-cloud/CSFX-Core/commit/385c30ebafafae24f5b6f572ac16a211938cb2b2))
* Update self_monitor to use Json type from sea_orm ([f53b567](https://github.com/CSFX-cloud/CSFX-Core/commit/f53b567c2526074af10474665feb34d646487cd1))
* update test file ([8c52df7](https://github.com/CSFX-cloud/CSFX-Core/commit/8c52df714880a389d13b77c6bb4f168e9f545d54))
* update test file ([12def61](https://github.com/CSFX-cloud/CSFX-Core/commit/12def61b65de4a7b4e3a621865af99da3ba15990))
* updater ([e1b45c8](https://github.com/CSFX-cloud/CSFX-Core/commit/e1b45c86e69fcbd046a79eecb6aaf7fa6dbb5f6a))
* updater ([a637575](https://github.com/CSFX-cloud/CSFX-Core/commit/a637575bdefb907fbab57985a75bd6c7ff5ebeab))
* updater backend ([2c51cf4](https://github.com/CSFX-cloud/CSFX-Core/commit/2c51cf4188339327ee24efdd2f4bdc7080bd0ae9))
* updater backend ([ff2d41a](https://github.com/CSFX-cloud/CSFX-Core/commit/ff2d41afe46fbb55f97f67871965f0f96b1d28b8))
* updater download ([57bb7e5](https://github.com/CSFX-cloud/CSFX-Core/commit/57bb7e55ff8663110d8efa372d511429a58f3f6c))
* updater download ([d66bb2b](https://github.com/CSFX-cloud/CSFX-Core/commit/d66bb2b80e43ab2a0059d563d31aa29d36ad1254))
* updater error ([f614134](https://github.com/CSFX-cloud/CSFX-Core/commit/f61413419e196801e50d4162d0130516f0838a3a))
* updater error ([c2d3273](https://github.com/CSFX-cloud/CSFX-Core/commit/c2d32738bf5b865c0f6e210d291cf973b26b9dcd))
* updater error ([cae097c](https://github.com/CSFX-cloud/CSFX-Core/commit/cae097c91a170873c0c4dc8147f31e3a537de3fd))
* updater error ([8775558](https://github.com/CSFX-cloud/CSFX-Core/commit/877555809fb6160c59710444cebb2ccdab9088bc))
* updater error with images and pull ([bbb8694](https://github.com/CSFX-cloud/CSFX-Core/commit/bbb8694fc2c7a3a065c5a08af073d2009b8a9fd4))
* updater fix complete log ([10e0943](https://github.com/CSFX-cloud/CSFX-Core/commit/10e094324832fa675cccabfb2f139dac60d51391))
* updater fix complete log ([87ed08e](https://github.com/CSFX-cloud/CSFX-Core/commit/87ed08e00b21cd138c6c25a28023dae90f559592))
* updater flow ([3d9b26c](https://github.com/CSFX-cloud/CSFX-Core/commit/3d9b26ce73f21adad7574df48f0c5995c1a82ea8))
* updater flow with api-gateway ([1de5b80](https://github.com/CSFX-cloud/CSFX-Core/commit/1de5b806df10452027c8b9a97c3808228178c064))
* updater flow with api-gateway ([5bf29ab](https://github.com/CSFX-cloud/CSFX-Core/commit/5bf29ab4e746717538d56956c32a76aa8532d245))
* updater from frontend ([0c4e596](https://github.com/CSFX-cloud/CSFX-Core/commit/0c4e596d1b728001c4c104b97327132fe92cd53f))
* updater from frontend ([d6f72c3](https://github.com/CSFX-cloud/CSFX-Core/commit/d6f72c392ae8b70bd0c447b78e7dcd83ef2aebd2))
* updater frontend screen ([30e638f](https://github.com/CSFX-cloud/CSFX-Core/commit/30e638f7efa853404e8af948794812750cebde1b))
* updater frontend screen ([8cbfbdc](https://github.com/CSFX-cloud/CSFX-Core/commit/8cbfbdc9d92151eee8751cab461398681159ed9b))
* updater prevelidge error ([af1bf41](https://github.com/CSFX-cloud/CSFX-Core/commit/af1bf41ce7a3560e7f18e28b730d9a075623dff5))
* updater prevelidge error ([788a637](https://github.com/CSFX-cloud/CSFX-Core/commit/788a6372dddc6f675b157f6e1e7bedd649d0d350))
* updater pull ([3ef7e36](https://github.com/CSFX-cloud/CSFX-Core/commit/3ef7e36cee7a2aeac7d6b6aa11107ccc712c12b5))
* updater screen ([7b48394](https://github.com/CSFX-cloud/CSFX-Core/commit/7b48394d12a9a20271e71ddf50545ce6859ff947))
* updater screen ([2b153ba](https://github.com/CSFX-cloud/CSFX-Core/commit/2b153ba21e6939806ecb03424b41e7144f73b39e))
* updater script ([364fc05](https://github.com/CSFX-cloud/CSFX-Core/commit/364fc0568d6521d028ab04f6736274c657c31708))
* updater script ([8f95aee](https://github.com/CSFX-cloud/CSFX-Core/commit/8f95aee1e388725512a38f6334e064c34163108f))
* updater test file ([8a277df](https://github.com/CSFX-cloud/CSFX-Core/commit/8a277df5a9d347755f55ebb71f013395bf944bbf))
* updater test file ([5008a78](https://github.com/CSFX-cloud/CSFX-Core/commit/5008a788071992090b9087a6dc3a3af960441067))
* updater test file ([300475b](https://github.com/CSFX-cloud/CSFX-Core/commit/300475bcaa25553a8c1b6caf262cce96580138ac))
* updater test file ([c67ceba](https://github.com/CSFX-cloud/CSFX-Core/commit/c67ceba0ac9ff976baed17e8dadc6ae0c1511984))
* updater test file ([f3a5766](https://github.com/CSFX-cloud/CSFX-Core/commit/f3a5766bdd6e6e8fb587f53255779e2eef92a141))
* updater test file ([05e1a16](https://github.com/CSFX-cloud/CSFX-Core/commit/05e1a165464307a18a2e5ea9735d73754a4132e5))
* updater test file ([2dc6d5c](https://github.com/CSFX-cloud/CSFX-Core/commit/2dc6d5c6fc4667c8d4f4c75656d431b5d3422ab3))
* updater test file ([b721cbe](https://github.com/CSFX-cloud/CSFX-Core/commit/b721cbeea7aae6d3205a0bdad4010377c7d58f6f))
* version ([3d63017](https://github.com/CSFX-cloud/CSFX-Core/commit/3d63017237d93288ba1645d9eb6b6f0f318c2ec3))
* version ([23573b8](https://github.com/CSFX-cloud/CSFX-Core/commit/23573b862761811ef1b8234477ccb63307687750))
* version docker image ([8768620](https://github.com/CSFX-cloud/CSFX-Core/commit/87686205a3bc01b67b7c6897c736da65f1ec752d))
* workflow ([c71c60f](https://github.com/CSFX-cloud/CSFX-Core/commit/c71c60f945d715666e6e0ed01678c5e59b0c48f4))

## [0.5.1](https://github.com/CS-Foundry/CSFX-Core/compare/v0.5.0...v0.5.1) (2026-03-07)


### Bug Fixes

* swagger ui ([033f790](https://github.com/CS-Foundry/CSFX-Core/commit/033f790c400efc7981de8c150f88136139c6ba74))

# [0.5.0](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.0...v0.5.0) (2026-03-07)


### Features

* added OpenTelemetry Tracing ([2d11250](https://github.com/CS-Foundry/CSFX-Core/commit/2d1125022cacd19c63e92d6e02cd0ec12a4896a5))

# [0.4.0](https://github.com/CS-Foundry/CSFX-Core/compare/v0.3.0...v0.4.0) (2026-03-07)


### Bug Fixes

* added docker compose and fix build in docker ([e24e7b9](https://github.com/CS-Foundry/CSFX-Core/commit/e24e7b9e441289a6edee0c0e244ae5489fe1d413))
* gitignore ([d6c3dca](https://github.com/CS-Foundry/CSFX-Core/commit/d6c3dcafc7f3aa28cf9c49ac9963d67797c9620f))
* mtls handshake ([cbe31b3](https://github.com/CS-Foundry/CSFX-Core/commit/cbe31b3765a60bb945bd5753b925236f0ab9042a))
* mtls heart beat ([869170f](https://github.com/CS-Foundry/CSFX-Core/commit/869170f2b51fc821bf2d64d5dfb4a7860446ab25))
* mtls issue ([26e0cb2](https://github.com/CS-Foundry/CSFX-Core/commit/26e0cb21b0691c34388278cbc8ce251ec1d26146))


### Features

* addded ground setup for scheduler ([2cca6f1](https://github.com/CS-Foundry/CSFX-Core/commit/2cca6f138a90471c505c1437696002f66bcb5679))
* added agend volume mount ([8549880](https://github.com/CS-Foundry/CSFX-Core/commit/85498809cecb27e3a72946c1d841101dffd9f07c))
* added container placement in agent and scheduler ([f84c304](https://github.com/CS-Foundry/CSFX-Core/commit/f84c3049580e96f7b2f2eabc98e5aaeb2d424d6b))
* added endpoints in api gateway ([c9a3563](https://github.com/CS-Foundry/CSFX-Core/commit/c9a35630d38decf7a007b8c7747da4f17a83597c))
* added migrations for workloads ([964c20a](https://github.com/CS-Foundry/CSFX-Core/commit/964c20a0c0d3d98fcf2e044ff38cc8605ddbddd7))
* added mtls encryption ([9692379](https://github.com/CS-Foundry/CSFX-Core/commit/969237918f5ce7e7b861a8cca9df4dc48c14cd3f))
* added pki for agent ([73fd3ad](https://github.com/CS-Foundry/CSFX-Core/commit/73fd3ad794e1392c4eee1f9d944d82e9d28f9fb4))
* added Prometheus Metrics and  Rate Limiting ([fbf9a48](https://github.com/CS-Foundry/CSFX-Core/commit/fbf9a48b20f6f4a9113c3a3f80051827333e3f41))
* added rbac for all things ([3b1be5b](https://github.com/CS-Foundry/CSFX-Core/commit/3b1be5b3e2b3af6ea290d0f531df8856b87cf708))
* added ressource modell and workload specs ([50aac08](https://github.com/CS-Foundry/CSFX-Core/commit/50aac08caafa6ce4fc0e6deb1eba06aedea4ebbf))
* added sdn controller for network ([49b5ed9](https://github.com/CS-Foundry/CSFX-Core/commit/49b5ed90d558a112f6d1e7e6c8e247a2a81372f2))
* added volume manager mount ([f0e3313](https://github.com/CS-Foundry/CSFX-Core/commit/f0e3313b68e0c026ca6fd6ebeee41616cd23dce3))
* entry point for schedueler in etcd cluster ([080225c](https://github.com/CS-Foundry/CSFX-Core/commit/080225c57fb984d4c713a4593bd1b83a0c571b7d))
* ground setup failover controler ([47ea8b6](https://github.com/CS-Foundry/CSFX-Core/commit/47ea8b647bdc621503a5a06e2afd80ed351c7a27))
* impl communcitaion and hearbeat ([d51fb91](https://github.com/CS-Foundry/CSFX-Core/commit/d51fb91651189334c38132427d4c11da6af7accf))

# [0.3.0](https://github.com/CS-Foundry/CSFX-Core/compare/v0.2.0...v0.3.0) (2026-03-03)


### Bug Fixes

* api gateway and regisrty ([68a9671](https://github.com/CS-Foundry/CSFX-Core/commit/68a96718290e13b308a2f4e9512e10da1d8a7cc7))
* auth probelm ([80ef776](https://github.com/CS-Foundry/CSFX-Core/commit/80ef776441ffd6b20ae1d451500e2b38e1b4d770))
* compile errors ([6cfe5ae](https://github.com/CS-Foundry/CSFX-Core/commit/6cfe5aedb51effa4152975ede9438391948a5241))
* docker compose ([8fd8be6](https://github.com/CS-Foundry/CSFX-Core/commit/8fd8be64d461b2679f4aa23aec807e92a9b4b820))
* docker long build ([2372614](https://github.com/CS-Foundry/CSFX-Core/commit/2372614b0f8a6a10ab2997d657cad211e8e53b98))
* ha for postgres with patroni ([078de22](https://github.com/CS-Foundry/CSFX-Core/commit/078de2230c5fc93871bf0c1bd64e5933ce5ea7a4))
* reverse proxy on registry routet through api gateway ([6e2ce4f](https://github.com/CS-Foundry/CSFX-Core/commit/6e2ce4fa34f145cf4cad3d966de33bc4fef13491))
* securtiy issues on agent registration ([228a81f](https://github.com/CS-Foundry/CSFX-Core/commit/228a81f818b5a0519ac026ba71790fe713431010))


### Features

* added ha with patroni on postgres ([e6f6037](https://github.com/CS-Foundry/CSFX-Core/commit/e6f603718c26cf52d283391bfd3e510fbd2c9763))
* ground setup ceph storage ([1ad3e67](https://github.com/CS-Foundry/CSFX-Core/commit/1ad3e67a1d032af910b64b9a98ae649aff3b6620))
* new connection to db and refactor ([063bc84](https://github.com/CS-Foundry/CSFX-Core/commit/063bc842d07926aa7bba3441a781bd9df5100f0a))
* pre reg agent for zero trust ([85bd67a](https://github.com/CS-Foundry/CSFX-Core/commit/85bd67af18bc070a2db463a1e44651c2562885b4))
* setup ground struc agent ([eeb12eb](https://github.com/CS-Foundry/CSFX-Core/commit/eeb12eb7fbb7b00874158ec08386650502133864))

# [0.2.0](https://github.com/CS-Foundry/CSFX-Core/compare/v0.1.13...v0.2.0) (2026-02-06)


### Bug Fixes

* leader election ([28871eb](https://github.com/CS-Foundry/CSFX-Core/commit/28871eb6b9fc98ee7a6792e618834baaf374f706))
* leader select ([e5d8867](https://github.com/CS-Foundry/CSFX-Core/commit/e5d88678b3378da846b18ea045d179a57651a47f))


### Features

* setup for etcd cluster ([af2db8d](https://github.com/CS-Foundry/CSFX-Core/commit/af2db8d3777fa0090a646b3a122984f38df248bd))

## [0.1.13](https://github.com/CS-Foundry/CSFX-Core/compare/v0.1.12...v0.1.13) (2026-02-03)


### Bug Fixes

* docker container on nix ([2aca464](https://github.com/CS-Foundry/CSFX-Core/commit/2aca464388c5efb0acf330ac9de332b0da925b89))
* docker start ([a247b64](https://github.com/CS-Foundry/CSFX-Core/commit/a247b6453b18ac9c26138e37285047885e7ad3e4))
* erros ([15623c3](https://github.com/CS-Foundry/CSFX-Core/commit/15623c34830ff30d88b0d8cf844dec77dcd77245))
* merge errors ([3d808ae](https://github.com/CS-Foundry/CSFX-Core/commit/3d808aed9b35da3f2f86aaa3f79a946256d899ea))
* new structure project ([5280c7e](https://github.com/CS-Foundry/CSFX-Core/commit/5280c7e562c9191c420353285a1e646657782a94))
* removed old scripts ([dc19cb5](https://github.com/CS-Foundry/CSFX-Core/commit/dc19cb582571cf3515effb8ba122f23536e31a3a))
* rm enity folder in every project ([dbd5178](https://github.com/CS-Foundry/CSFX-Core/commit/dbd51781b76cb6bd793a361ba39ad40f8bb4f9dd))
* securtity issue fix sha-1 to sha-256 ([4252495](https://github.com/CS-Foundry/CSFX-Core/commit/42524951a5d53c7fe48de02abb7cec99d7ee0550))
* shared folder ([bc7ce08](https://github.com/CS-Foundry/CSFX-Core/commit/bc7ce08c1399119c835e3120c03372672a6d0631))
* shared folder ([752f4df](https://github.com/CS-Foundry/CSFX-Core/commit/752f4df9fd771d6a7380b9bc949378856ffefa78))
* structure porject fix ([8d40d6a](https://github.com/CS-Foundry/CSFX-Core/commit/8d40d6aa329ed175a41e38dcbf4c6aae1c55bd86))
* structure project ([d2d83f1](https://github.com/CS-Foundry/CSFX-Core/commit/d2d83f1bb0f6a1a27ace1213d98d7bac2879a949))

## [0.1.12](https://github.com/CS-Foundry/CSFX-Core/compare/v0.1.11...v0.1.12) (2026-01-25)


### Bug Fixes

* backend error ([cd69b8c](https://github.com/CS-Foundry/CSFX-Core/commit/cd69b8c9d89faa11a7c12c6eb42262428f7e6777))

## [0.1.11](https://github.com/CS-Foundry/CSFX-Core/compare/v0.1.10...v0.1.11) (2026-01-25)


### Bug Fixes

* arm runner and manifest error ([a1ad641](https://github.com/CS-Foundry/CSFX-Core/commit/a1ad641c705482d52e592b1ff729dfcdbab958f5))

## [0.1.10](https://github.com/CS-Foundry/CSFX-Core/compare/v0.1.9...v0.1.10) (2026-01-24)


### Bug Fixes

* mulitple docker builds ([6a55d51](https://github.com/CS-Foundry/CSFX-Core/commit/6a55d51027477a7226915e7f4ef61a45a8013692))

## [0.1.9](https://github.com/CS-Foundry/CSFX-Core/compare/v0.1.8...v0.1.9) (2026-01-24)


### Bug Fixes

* provide complete workspace structure to cargo-chef ([5bef937](https://github.com/CS-Foundry/CSFX-Core/commit/5bef937644c38dec3d3bb9dc39a4ef5df85c1268))

## [0.1.8](https://github.com/CS-Foundry/CSFX-Core/compare/v0.1.7...v0.1.8) (2026-01-24)


### Bug Fixes

* github pipleine time ([6e8e2cd](https://github.com/CS-Foundry/CSFX-Core/commit/6e8e2cd25bd17c41db5aa6ad64d3d7519c17c809))

## [0.1.7](https://github.com/CS-Foundry/CSFX-Core/compare/v0.1.6...v0.1.7) (2026-01-24)


### Bug Fixes

* pipeline time ([a642161](https://github.com/CS-Foundry/CSFX-Core/commit/a64216117ad7600664d424d81989bb509f0020a2))

## [0.1.6](https://github.com/CS-Foundry/CSFX-Core/compare/v0.1.5...v0.1.6) (2026-01-24)


### Bug Fixes

* pipeline ([25a9442](https://github.com/CS-Foundry/CSFX-Core/commit/25a944201cafbd55e6202f8fa47858ebf3445717))

## [0.1.5](https://github.com/CS-Foundry/CSFX-Core/compare/v0.1.4...v0.1.5) (2026-01-24)


### Bug Fixes

* pipeline docker image ([c5743de](https://github.com/CS-Foundry/CSFX-Core/commit/c5743de52af8862013515bccc8aa9fb82267219e))

## [0.1.4](https://github.com/CS-Foundry/CSFX-Core/compare/v0.1.3...v0.1.4) (2026-01-23)


### Bug Fixes

* pipeline ([3f6b004](https://github.com/CS-Foundry/CSFX-Core/commit/3f6b004bba89ba8a2637ff7ca74b43c4b7fba7d7))

## [0.1.3](https://github.com/CS-Foundry/CSFX-Core/compare/v0.1.2...v0.1.3) (2026-01-23)


### Bug Fixes

* pipeline docker build ([909cc1a](https://github.com/CS-Foundry/CSFX-Core/commit/909cc1a4c8cf776d188191b52f9c7ce902bb5ff8))

## [0.1.2](https://github.com/CS-Foundry/CSFX-Core/compare/v0.1.1...v0.1.2) (2026-01-23)


### Bug Fixes

* pipleine ([885eadd](https://github.com/CS-Foundry/CSFX-Core/commit/885eadd3f0ec2154659070e901bc03c7c2f294d2))

## [0.1.1](https://github.com/CS-Foundry/CSFX-Core/compare/v0.1.0...v0.1.1) (2026-01-23)


### Bug Fixes

* ci ([2d5b689](https://github.com/CS-Foundry/CSFX-Core/commit/2d5b689c3008f6dc210a43a7984278a4f54205ae))

# [0.1.0](https://github.com/CS-Foundry/CSFX-Core/compare/v0.0.1...v0.1.0) (2026-01-23)


### Bug Fixes

* added test file ([7b37dc1](https://github.com/CS-Foundry/CSFX-Core/commit/7b37dc1ea34a2a4aefe54c6d348124ac8d99a640))
* added test file for updater ([d3cc2f3](https://github.com/CS-Foundry/CSFX-Core/commit/d3cc2f341347274e7f7d1ed2de48a7340a049d4a))
* backend compile error ([cb476e5](https://github.com/CS-Foundry/CSFX-Core/commit/cb476e55cb7c3b04c2981fae716a45c2bd208995))
* backend frontend connection ([d12c054](https://github.com/CS-Foundry/CSFX-Core/commit/d12c05416d5e8a44f05da5a6cc2addc9007992ca))
* backup dir error ([e36b67e](https://github.com/CS-Foundry/CSFX-Core/commit/e36b67e0942df2cb2526becae260dabd77bd3148))
* build error ([78364e8](https://github.com/CS-Foundry/CSFX-Core/commit/78364e809165d8f3b598279d08180ec5b95480af))
* build in pipleine ([a7a0a1c](https://github.com/CS-Foundry/CSFX-Core/commit/a7a0a1c0f29a0b59bf9f3aeba7b6ce604049d46c))
* ci pipeline new setup with nix ([eaffdf5](https://github.com/CS-Foundry/CSFX-Core/commit/eaffdf54134c3415b0e55e9e3e5bbcb7f2689adb))
* docker warn on linux kernel ([1de9a08](https://github.com/CS-Foundry/CSFX-Core/commit/1de9a084cbbe5cec93fc2205415c3f1f5ab5b597))
* double vv in version ([48065e5](https://github.com/CS-Foundry/CSFX-Core/commit/48065e564a46bfa497fa61be1145437ec06d5415))
* error backup location ([b3d0246](https://github.com/CS-Foundry/CSFX-Core/commit/b3d024694be9c5aad6fb6e55af460c3757eb9f89))
* frontend build error ([afec643](https://github.com/CS-Foundry/CSFX-Core/commit/afec64354d33c9e70cf32cee2483a03250c1b108))
* include production node_modules in frontend package and add download stats ([13d7460](https://github.com/CS-Foundry/CSFX-Core/commit/13d746039901b4de70f02b9d99651d6b374965c3))
* install script pull ([63814d1](https://github.com/CS-Foundry/CSFX-Core/commit/63814d1ab67b694bb94ae69176ba03c67793d7b9))
* persistante update screen ([f43c476](https://github.com/CS-Foundry/CSFX-Core/commit/f43c476d8926f475102f2de0eb48ca5c60c5f35f))
* pipeline ([7a0154d](https://github.com/CS-Foundry/CSFX-Core/commit/7a0154d9b71931db881783b179f599316d44ce9e))
* pipeline binary push beta releases ([4ce046c](https://github.com/CS-Foundry/CSFX-Core/commit/4ce046ce9d1d6480cf70413883dba7ccc3fecd48))
* pipeline build error ([8007bc4](https://github.com/CS-Foundry/CSFX-Core/commit/8007bc47a90f049421f4d0a7d420424bab969e03))
* release pipeline for beat ([a908b37](https://github.com/CS-Foundry/CSFX-Core/commit/a908b3711c537ef0b3ceeb90fe6acb915fdb7945))
* script error ([4b1b343](https://github.com/CS-Foundry/CSFX-Core/commit/4b1b3436aa9b28bf90c8bd97ca1074b9ef1d9b28))
* self kill error updater and manuell fix updater error ([f17d096](https://github.com/CS-Foundry/CSFX-Core/commit/f17d09653022c39061ad6b9c7648161c2ee56cb4))
* semantic release commit befor build ([9927644](https://github.com/CS-Foundry/CSFX-Core/commit/99276446079e169853a7b2b7848a369b45d0f930))
* semantiv release versioning ([4b4ce16](https://github.com/CS-Foundry/CSFX-Core/commit/4b4ce161a29b96531248f11b228a71d2cce0b950))
* test file for updater ([0927186](https://github.com/CS-Foundry/CSFX-Core/commit/0927186d706062c85eebe48c35a11e3db3073357))
* test file updater ([6e55d23](https://github.com/CS-Foundry/CSFX-Core/commit/6e55d23e61a81b7ad5696f046a168a87bc4f6716))
* test file updater ([398adf1](https://github.com/CS-Foundry/CSFX-Core/commit/398adf17716f012109360cb81b29e64d596a40bc))
* test file updater ([666f334](https://github.com/CS-Foundry/CSFX-Core/commit/666f334b08076d832e2c9cd04345cb654f70206b))
* test file updtaer ([aae1373](https://github.com/CS-Foundry/CSFX-Core/commit/aae1373ec9b9649fcddfdbc2345286eaeb14af17))
* update permission error ([f7b57ec](https://github.com/CS-Foundry/CSFX-Core/commit/f7b57ec497bf172d9a68e17af401e4bc156fdd26))
* update script added to installation ([385c30e](https://github.com/CS-Foundry/CSFX-Core/commit/385c30ebafafae24f5b6f572ac16a211938cb2b2))
* update test file ([12def61](https://github.com/CS-Foundry/CSFX-Core/commit/12def61b65de4a7b4e3a621865af99da3ba15990))
* updater ([a637575](https://github.com/CS-Foundry/CSFX-Core/commit/a637575bdefb907fbab57985a75bd6c7ff5ebeab))
* updater backend ([ff2d41a](https://github.com/CS-Foundry/CSFX-Core/commit/ff2d41afe46fbb55f97f67871965f0f96b1d28b8))
* updater download ([d66bb2b](https://github.com/CS-Foundry/CSFX-Core/commit/d66bb2b80e43ab2a0059d563d31aa29d36ad1254))
* updater error ([c2d3273](https://github.com/CS-Foundry/CSFX-Core/commit/c2d32738bf5b865c0f6e210d291cf973b26b9dcd))
* updater error ([8775558](https://github.com/CS-Foundry/CSFX-Core/commit/877555809fb6160c59710444cebb2ccdab9088bc))
* updater fix complete log ([87ed08e](https://github.com/CS-Foundry/CSFX-Core/commit/87ed08e00b21cd138c6c25a28023dae90f559592))
* updater from frontend ([d6f72c3](https://github.com/CS-Foundry/CSFX-Core/commit/d6f72c392ae8b70bd0c447b78e7dcd83ef2aebd2))
* updater frontend screen ([8cbfbdc](https://github.com/CS-Foundry/CSFX-Core/commit/8cbfbdc9d92151eee8751cab461398681159ed9b))
* updater prevelidge error ([788a637](https://github.com/CS-Foundry/CSFX-Core/commit/788a6372dddc6f675b157f6e1e7bedd649d0d350))
* updater pull ([3ef7e36](https://github.com/CS-Foundry/CSFX-Core/commit/3ef7e36cee7a2aeac7d6b6aa11107ccc712c12b5))
* updater screen ([2b153ba](https://github.com/CS-Foundry/CSFX-Core/commit/2b153ba21e6939806ecb03424b41e7144f73b39e))
* updater script ([8f95aee](https://github.com/CS-Foundry/CSFX-Core/commit/8f95aee1e388725512a38f6334e064c34163108f))
* updater test file ([5008a78](https://github.com/CS-Foundry/CSFX-Core/commit/5008a788071992090b9087a6dc3a3af960441067))
* updater test file ([c67ceba](https://github.com/CS-Foundry/CSFX-Core/commit/c67ceba0ac9ff976baed17e8dadc6ae0c1511984))
* updater test file ([05e1a16](https://github.com/CS-Foundry/CSFX-Core/commit/05e1a165464307a18a2e5ea9735d73754a4132e5))
* updater test file ([b721cbe](https://github.com/CS-Foundry/CSFX-Core/commit/b721cbeea7aae6d3205a0bdad4010377c7d58f6f))
* version ([3d63017](https://github.com/CS-Foundry/CSFX-Core/commit/3d63017237d93288ba1645d9eb6b6f0f318c2ec3))
* version ([23573b8](https://github.com/CS-Foundry/CSFX-Core/commit/23573b862761811ef1b8234477ccb63307687750))


### Features

* added nix config for nix os master node ([8c3a866](https://github.com/CS-Foundry/CSFX-Core/commit/8c3a8666973ac1c1cd06a0b1932af25f8842ee92))
* agents with mtls ([bceb6e0](https://github.com/CS-Foundry/CSFX-Core/commit/bceb6e0faa39f95eb3a02fc556ab60b6b835f3ca))
* dashboard not working only test ([cfe6edb](https://github.com/CS-Foundry/CSFX-Core/commit/cfe6edb108d5b48f144122e06160211fd9b06a61))
* live update screen ([879c62e](https://github.com/CS-Foundry/CSFX-Core/commit/879c62efc32fbfbf97d4e2d97ac6ab3f0b7384de))
* new beta branch features ([b88b509](https://github.com/CS-Foundry/CSFX-Core/commit/b88b509342da00aeea618ece55bc6d911ac543e5))
* nix deployment for auto docker deploy ([fb3c2da](https://github.com/CS-Foundry/CSFX-Core/commit/fb3c2da09b697c7631f4665a653780d34f95e3d2))
* nix os config deploy on test server ([b8b5aff](https://github.com/CS-Foundry/CSFX-Core/commit/b8b5affb0a0170bfd833936c3dd0b1ea6d14259d))
* updater for programm ([7b064b8](https://github.com/CS-Foundry/CSFX-Core/commit/7b064b8255b34cde174a591e93c7c67604997f2c))

## [0.4.25](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.24...v0.4.25) (2026-01-17)


### Bug Fixes

* updater test file ([5008a78](https://github.com/CS-Foundry/CSFX-Core/commit/5008a788071992090b9087a6dc3a3af960441067))

## [0.4.24](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.23...v0.4.24) (2026-01-17)


### Bug Fixes

* updater ([a637575](https://github.com/CS-Foundry/CSFX-Core/commit/a637575bdefb907fbab57985a75bd6c7ff5ebeab))

## [0.4.23](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.22...v0.4.23) (2026-01-12)


### Bug Fixes

* updater test file ([c67ceba](https://github.com/CS-Foundry/CSFX-Core/commit/c67ceba0ac9ff976baed17e8dadc6ae0c1511984))

## [0.4.22](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.21...v0.4.22) (2026-01-12)


### Bug Fixes

* updater from frontend ([d6f72c3](https://github.com/CS-Foundry/CSFX-Core/commit/d6f72c392ae8b70bd0c447b78e7dcd83ef2aebd2))

## [0.4.21](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.20...v0.4.21) (2026-01-12)


### Bug Fixes

* test file updater ([6e55d23](https://github.com/CS-Foundry/CSFX-Core/commit/6e55d23e61a81b7ad5696f046a168a87bc4f6716))

## [0.4.20](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.19...v0.4.20) (2026-01-11)


### Bug Fixes

* double vv in version ([48065e5](https://github.com/CS-Foundry/CSFX-Core/commit/48065e564a46bfa497fa61be1145437ec06d5415))

## [0.4.19](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.18...v0.4.19) (2026-01-11)


### Bug Fixes

* test file updater ([398adf1](https://github.com/CS-Foundry/CSFX-Core/commit/398adf17716f012109360cb81b29e64d596a40bc))

## [0.4.18](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.17...v0.4.18) (2026-01-11)


### Bug Fixes

* updater screen ([2b153ba](https://github.com/CS-Foundry/CSFX-Core/commit/2b153ba21e6939806ecb03424b41e7144f73b39e))

## [0.4.17](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.16...v0.4.17) (2026-01-11)


### Bug Fixes

* test file updater ([666f334](https://github.com/CS-Foundry/CSFX-Core/commit/666f334b08076d832e2c9cd04345cb654f70206b))

## [0.4.16](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.15...v0.4.16) (2026-01-10)


### Bug Fixes

* updater frontend screen ([8cbfbdc](https://github.com/CS-Foundry/CSFX-Core/commit/8cbfbdc9d92151eee8751cab461398681159ed9b))

## [0.4.15](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.14...v0.4.15) (2026-01-10)


### Bug Fixes

* added test file for updater ([d3cc2f3](https://github.com/CS-Foundry/CSFX-Core/commit/d3cc2f341347274e7f7d1ed2de48a7340a049d4a))

## [0.4.14](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.13...v0.4.14) (2026-01-10)


### Bug Fixes

* updater fix complete log ([87ed08e](https://github.com/CS-Foundry/CSFX-Core/commit/87ed08e00b21cd138c6c25a28023dae90f559592))

## [0.4.13](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.12...v0.4.13) (2026-01-10)


### Bug Fixes

* updater test file ([05e1a16](https://github.com/CS-Foundry/CSFX-Core/commit/05e1a165464307a18a2e5ea9735d73754a4132e5))

## [0.4.12](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.11...v0.4.12) (2026-01-10)


### Bug Fixes

* persistante update screen ([f43c476](https://github.com/CS-Foundry/CSFX-Core/commit/f43c476d8926f475102f2de0eb48ca5c60c5f35f))
* self kill error updater and manuell fix updater error ([f17d096](https://github.com/CS-Foundry/CSFX-Core/commit/f17d09653022c39061ad6b9c7648161c2ee56cb4))

## [0.4.11](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.10...v0.4.11) (2026-01-09)


### Bug Fixes

* updater test file ([b721cbe](https://github.com/CS-Foundry/CSFX-Core/commit/b721cbeea7aae6d3205a0bdad4010377c7d58f6f))

## [0.4.10](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.9...v0.4.10) (2026-01-09)


### Bug Fixes

* updater prevelidge error ([788a637](https://github.com/CS-Foundry/CSFX-Core/commit/788a6372dddc6f675b157f6e1e7bedd649d0d350))

## [0.4.9](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.8...v0.4.9) (2026-01-09)


### Bug Fixes

* test file updtaer ([aae1373](https://github.com/CS-Foundry/CSFX-Core/commit/aae1373ec9b9649fcddfdbc2345286eaeb14af17))

## [0.4.8](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.7...v0.4.8) (2026-01-08)


### Bug Fixes

* update permission error ([f7b57ec](https://github.com/CS-Foundry/CSFX-Core/commit/f7b57ec497bf172d9a68e17af401e4bc156fdd26))

## [0.4.7](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.6...v0.4.7) (2026-01-08)


### Bug Fixes

* update test file ([12def61](https://github.com/CS-Foundry/CSFX-Core/commit/12def61b65de4a7b4e3a621865af99da3ba15990))

## [0.4.6](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.5...v0.4.6) (2026-01-08)


### Bug Fixes

* backend compile error ([cb476e5](https://github.com/CS-Foundry/CSFX-Core/commit/cb476e55cb7c3b04c2981fae716a45c2bd208995))
* script error ([4b1b343](https://github.com/CS-Foundry/CSFX-Core/commit/4b1b3436aa9b28bf90c8bd97ca1074b9ef1d9b28))

## [0.4.5](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.4...v0.4.5) (2026-01-08)


### Bug Fixes

* updater backend ([ff2d41a](https://github.com/CS-Foundry/CSFX-Core/commit/ff2d41afe46fbb55f97f67871965f0f96b1d28b8))

## [0.4.4](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.3...v0.4.4) (2026-01-08)


### Bug Fixes

* added test file ([7b37dc1](https://github.com/CS-Foundry/CSFX-Core/commit/7b37dc1ea34a2a4aefe54c6d348124ac8d99a640))

## [0.4.3](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.2...v0.4.3) (2026-01-08)


### Bug Fixes

* updater download ([d66bb2b](https://github.com/CS-Foundry/CSFX-Core/commit/d66bb2b80e43ab2a0059d563d31aa29d36ad1254))

## [0.4.2](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.1...v0.4.2) (2026-01-08)


### Bug Fixes

* test file for updater ([0927186](https://github.com/CS-Foundry/CSFX-Core/commit/0927186d706062c85eebe48c35a11e3db3073357))

## [0.4.1](https://github.com/CS-Foundry/CSFX-Core/compare/v0.4.0...v0.4.1) (2026-01-08)


### Bug Fixes

* updater error ([c2d3273](https://github.com/CS-Foundry/CSFX-Core/commit/c2d32738bf5b865c0f6e210d291cf973b26b9dcd))

# [0.4.0](https://github.com/CS-Foundry/CSFX-Core/compare/v0.3.4...v0.4.0) (2026-01-08)


### Bug Fixes

* pipeline binary push beta releases ([4ce046c](https://github.com/CS-Foundry/CSFX-Core/commit/4ce046ce9d1d6480cf70413883dba7ccc3fecd48))
* release pipeline for beat ([a908b37](https://github.com/CS-Foundry/CSFX-Core/commit/a908b3711c537ef0b3ceeb90fe6acb915fdb7945))


### Features

* dashboard not working only test ([cfe6edb](https://github.com/CS-Foundry/CSFX-Core/commit/cfe6edb108d5b48f144122e06160211fd9b06a61))

## [0.3.4](https://github.com/CS-Foundry/CSFX-Core/compare/v0.3.3...v0.3.4) (2026-01-07)


### Bug Fixes

* error backup location ([b3d0246](https://github.com/CS-Foundry/CSFX-Core/commit/b3d024694be9c5aad6fb6e55af460c3757eb9f89))

## [0.3.3](https://github.com/CS-Foundry/CSFX-Core/compare/v0.3.2...v0.3.3) (2026-01-07)


### Bug Fixes

* backup dir error ([e36b67e](https://github.com/CS-Foundry/CSFX-Core/commit/e36b67e0942df2cb2526becae260dabd77bd3148))

## [0.3.2](https://github.com/CS-Foundry/CSFX-Core/compare/v0.3.1...v0.3.2) (2026-01-07)


### Bug Fixes

* updater script ([8f95aee](https://github.com/CS-Foundry/CSFX-Core/commit/8f95aee1e388725512a38f6334e064c34163108f))

## [0.3.1](https://github.com/CS-Foundry/CSFX-Core/compare/v0.3.0...v0.3.1) (2026-01-06)


### Bug Fixes

* build error ([78364e8](https://github.com/CS-Foundry/CSFX-Core/commit/78364e809165d8f3b598279d08180ec5b95480af))

# [0.3.0](https://github.com/CS-Foundry/CSFX-Core/compare/v0.2.4...v0.3.0) (2026-01-06)


### Features

* live update screen ([879c62e](https://github.com/CS-Foundry/CSFX-Core/commit/879c62efc32fbfbf97d4e2d97ac6ab3f0b7384de))

## [0.2.4](https://github.com/CS-Foundry/CSFX-Core/compare/v0.2.3...v0.2.4) (2026-01-06)


### Bug Fixes

* update script added to installation ([385c30e](https://github.com/CS-Foundry/CSFX-Core/commit/385c30ebafafae24f5b6f572ac16a211938cb2b2))
* updater error ([8775558](https://github.com/CS-Foundry/CSFX-Core/commit/877555809fb6160c59710444cebb2ccdab9088bc))

# [0.3.0-beta.3](https://github.com/CS-Foundry/CSFX-Core/compare/v0.3.0-beta.2...v0.3.0-beta.3) (2026-01-05)


### Bug Fixes

* release pipeline for beat ([a908b37](https://github.com/CS-Foundry/CSFX-Core/commit/a908b3711c537ef0b3ceeb90fe6acb915fdb7945))

# [0.3.0-beta.2](https://github.com/CS-Foundry/CSFX-Core/compare/v0.3.0-beta.1...v0.3.0-beta.2) (2026-01-05)


### Bug Fixes

* pipeline binary push beta releases ([4ce046c](https://github.com/CS-Foundry/CSFX-Core/commit/4ce046ce9d1d6480cf70413883dba7ccc3fecd48))

# [0.3.0-beta.1](https://github.com/CS-Foundry/CSFX-Core/compare/v0.2.2...v0.3.0-beta.1) (2026-01-05)


### Features

* dashboard not working only test ([cfe6edb](https://github.com/CS-Foundry/CSFX-Core/commit/cfe6edb108d5b48f144122e06160211fd9b06a61))

>>>>>>> origin/main
## [0.2.2](https://github.com/CS-Foundry/CSFX-Core/compare/v0.2.1...v0.2.2) (2026-01-05)


### Bug Fixes

* frontend build error ([afec643](https://github.com/CS-Foundry/CSFX-Core/commit/afec64354d33c9e70cf32cee2483a03250c1b108))

## [0.2.1](https://github.com/CS-Foundry/CSFX-Core/compare/v0.2.0...v0.2.1) (2026-01-05)


### Bug Fixes

* semantic release commit befor build ([9927644](https://github.com/CS-Foundry/CSFX-Core/commit/99276446079e169853a7b2b7848a369b45d0f930))

# [0.2.0](https://github.com/CS-Foundry/CSFX-Core/compare/v0.1.3...v0.2.0) (2026-01-05)


### Features

* new beta branch features ([b88b509](https://github.com/CS-Foundry/CSFX-Core/commit/b88b509342da00aeea618ece55bc6d911ac543e5))

## [0.1.3](https://github.com/CS-Foundry/CSFX-Core/compare/v0.1.2...v0.1.3) (2026-01-04)


### Bug Fixes

* semantiv release versioning ([4b4ce16](https://github.com/CS-Foundry/CSFX-Core/commit/4b4ce161a29b96531248f11b228a71d2cce0b950))

## [0.1.2](https://github.com/CS-Foundry/CSFX-Core/compare/v0.1.1...v0.1.2) (2026-01-04)


### Bug Fixes

* version ([3d63017](https://github.com/CS-Foundry/CSFX-Core/commit/3d63017237d93288ba1645d9eb6b6f0f318c2ec3))
* version ([23573b8](https://github.com/CS-Foundry/CSFX-Core/commit/23573b862761811ef1b8234477ccb63307687750))

## [0.1.1](https://github.com/CS-Foundry/CSFX-Core/compare/v0.1.0...v0.1.1) (2026-01-04)


### Bug Fixes

* updater pull ([3ef7e36](https://github.com/CS-Foundry/CSFX-Core/commit/3ef7e36cee7a2aeac7d6b6aa11107ccc712c12b5))

# [0.1.0](https://github.com/CS-Foundry/CSFX-Core/compare/v0.0.8...v0.1.0) (2026-01-04)


### Features

* updater for programm ([7b064b8](https://github.com/CS-Foundry/CSFX-Core/commit/7b064b8255b34cde174a591e93c7c67604997f2c))

## [0.0.8](https://github.com/CS-Foundry/CSFX-Core/compare/v0.0.7...v0.0.8) (2026-01-04)


### Bug Fixes

* docker warn on linux kernel ([1de9a08](https://github.com/CS-Foundry/CSFX-Core/commit/1de9a084cbbe5cec93fc2205415c3f1f5ab5b597))
