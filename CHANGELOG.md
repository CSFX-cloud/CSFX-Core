# Changelog

All notable changes to CSFX-Core will be documented in this file.

## [0.2.3](https://github.com/CSFX-cloud/CSFX-Core/compare/v0.2.2...v0.2.3) (2026-10-03)


### Features

* add animated custom icons for all sidebar navigation items ([8a331f4](https://github.com/CSFX-cloud/CSFX-Core/commit/8a331f432e62af605d65d0b899213c1e0282dba9))
* add auto-generated self-signed TLS to api-gateway ([cdebae4](https://github.com/CSFX-cloud/CSFX-Core/commit/cdebae4220d4d0342ffa20e867d7f2efc9091cd8))
* add binary self-update for csf-agent and csf-updater via github releases ([847dfe5](https://github.com/CSFX-cloud/CSFX-Core/commit/847dfe5b3b13a7764aae36715b751afb829e7142))
* add blobatar avatar fallback with org-wide toggle ([f4f416a](https://github.com/CSFX-cloud/CSFX-Core/commit/f4f416a2168b2ed495ed293d1edf2dd32757fb8e))
* add bucket access key issuance and rotation ([f1a65b2](https://github.com/CSFX-cloud/CSFX-Core/commit/f1a65b26b1380d02cbeb3a6bf526b19761092766))
* add bucket tab to resource group dashboard ([7be4450](https://github.com/CSFX-cloud/CSFX-Core/commit/7be44504b8ee460d6e9ad0054216e91f62364500))
* add centralized log viewer with per-service tracing capture and admin retention settings ([509f7a8](https://github.com/CSFX-cloud/CSFX-Core/commit/509f7a8e966df78f54260b7b79062b75a392cce2))
* add cidr suggestion, rg search, pinned rg cards and settings/delete actions ([55b603c](https://github.com/CSFX-cloud/CSFX-Core/commit/55b603ca9cfb43d85b8eb13b13b0602c3813bf93))
* add container settings edit and fix compose port and status bugs ([6da4250](https://github.com/CSFX-cloud/CSFX-Core/commit/6da42500c582c412eda40ffe9a64dd715f2d2b1d))
* add container status detail, log streaming, and exec shell for workloads ([3de5fab](https://github.com/CSFX-cloud/CSFX-Core/commit/3de5fabff2873017c854f11abe8dfa73a4fa9e6e))
* add container stop, restart, and performance insights UI with backend lifecycle support ([1a03c1c](https://github.com/CSFX-cloud/CSFX-Core/commit/1a03c1cba016feb187fcfcff5547957d7391118b))
* add customizable icon, color and pinning for resource groups ([fb130e6](https://github.com/CSFX-cloud/CSFX-Core/commit/fb130e6d6832b79aa98c03ae57df6542408c3ec0))
* add dark and light background variants to login page ([001c01b](https://github.com/CSFX-cloud/CSFX-Core/commit/001c01ba9d760e3bcea963d292e20c5fea60400e))
* add data model for object storage buckets and garage nodes ([c20964c](https://github.com/CSFX-cloud/CSFX-Core/commit/c20964c2e76f856bf7376b4281fa84997d8d3465))
* add debug tracing across rootfs build and workload boot pipeline ([ddade34](https://github.com/CSFX-cloud/CSFX-Core/commit/ddade34ce2ed862624a19eaf8e3365175497a591))
* add dedicated vm detail page with stats and console ([e9c64d3](https://github.com/CSFX-cloud/CSFX-Core/commit/e9c64d314007d064a6f4099b73d938e6e6b862e1))
* add docker compose stack deployment with service discovery and smart workload icons ([4fef60f](https://github.com/CSFX-cloud/CSFX-Core/commit/4fef60f9a373d10ad92683dc64779ef653470f22))
* add firecracker microvm runtime alongside docker via runtime trait ([91cf259](https://github.com/CSFX-cloud/CSFX-Core/commit/91cf259db5cec28ae9f774d047326e20f5d05143))
* add garage cluster reconciliation with dynamic replication factor ([c6a7ca2](https://github.com/CSFX-cloud/CSFX-Core/commit/c6a7ca2f3954afecc5d83e86c3ca2bdacbe8d08d))
* add garage dev container for local object storage testing ([6b7d93a](https://github.com/CSFX-cloud/CSFX-Core/commit/6b7d93a1f550ec610618b8bc7dfeed9985ab482b))
* add gravatar avatar support via profile email ([ad086f1](https://github.com/CSFX-cloud/CSFX-Core/commit/ad086f17457f7c89d806ea688a8dfe15200a1fd3))
* add hardware alert engine and node maintenance mode ([45c0a6a](https://github.com/CSFX-cloud/CSFX-Core/commit/45c0a6ab99aeac7c9cfda60b3b2373120516d043))
* add hardware alert engine, maintenance mode and node workloads tab ([be0e43b](https://github.com/CSFX-cloud/CSFX-Core/commit/be0e43b2b88229c8740a7c141244cf799d02b426))
* add live status polling and loader badges for workloads ([0134ef1](https://github.com/CSFX-cloud/CSFX-Core/commit/0134ef1536c76641e6c7b5ca6b939e916a8c1b75))
* add log stream diagnostics for guest vsock log path ([2f0c37a](https://github.com/CSFX-cloud/CSFX-Core/commit/2f0c37a23f2939c17bb8bca9f8268773a46eeaf4))
* add manual alert resolve endpoint and node alert action ([3716312](https://github.com/CSFX-cloud/CSFX-Core/commit/3716312a68b912f60700ecd6a6578bc27bdc2171))
* add node reboot, power off, and drain actions ([c606fea](https://github.com/CSFX-cloud/CSFX-Core/commit/c606fea16b3856243a8877b8703382d81bf4b9d2))
* add object-storage service with bucket crud ([cd84454](https://github.com/CSFX-cloud/CSFX-Core/commit/cd84454b5fe281dd15c76424647d39e42f73d55d))
* add OCI registry pull-through cache to avoid docker.io rate limits ([0f9a5ca](https://github.com/CSFX-cloud/CSFX-Core/commit/0f9a5ca0e72b005c63b992cfe998d03b450d3e7d))
* add patroni user, config template and entrypoint script ([ec78a2e](https://github.com/CSFX-cloud/CSFX-Core/commit/ec78a2e92a00c34ecfb990aaed17c47a0e11e951))
* add per-node disk inventory with SMART health data ([68e8a34](https://github.com/CSFX-cloud/CSFX-Core/commit/68e8a3445c5290f846b9488c937bdcecbe23dab0))
* add qemu vm runtime with vnc console bridge ([88efb4f](https://github.com/CSFX-cloud/CSFX-Core/commit/88efb4fff44cb0242ca1f6056f3275609023c388))
* add redeploy and stack management for containers and compose stacks ([ff3b1e7](https://github.com/CSFX-cloud/CSFX-Core/commit/ff3b1e72e43507e65e1e9f68e770a8a56e807fa8))
* add resource group icon upload and redesign detail layout ([4bd3231](https://github.com/CSFX-cloud/CSFX-Core/commit/4bd32315feb83944502e542702dff9f963c2fde7))
* add resource group preview card with persistent preview url ([cf25005](https://github.com/CSFX-cloud/CSFX-Core/commit/cf25005c8c4b2fb242b9de599de4b3e56eb12b9c))
* add resource picker with docker compose stacks and dynamic service icons ([caf3acb](https://github.com/CSFX-cloud/CSFX-Core/commit/caf3acb92b7f6e48fe6fa6f0498db6f6f0dfd39b))
* add RG port mapping with dedicated container/RG/node port UI ([e8f6ad5](https://github.com/CSFX-cloud/CSFX-Core/commit/e8f6ad55d38f3cfbbfb3e1f16e18b592fb8a6046))
* add rg_port dnat for resource-group-scoped port mapping ([0e489f5](https://github.com/CSFX-cloud/CSFX-Core/commit/0e489f5918ac2e3d6e312c4177e86068afcd7e95))
* add rg-internal s3 dns and dnat wiring in agent ([630dedc](https://github.com/CSFX-cloud/CSFX-Core/commit/630dedcd9a8a127d3ad439e4f3872e10b7fe5a76))
* add s3 object browser with presigned upload and download ([2af50c2](https://github.com/CSFX-cloud/CSFX-Core/commit/2af50c227623d3f946f540268324d41eddd8cedb))
* add server rack image with fade overlay to nodes info panel ([4d4c1ab](https://github.com/CSFX-cloud/CSFX-Core/commit/4d4c1ab2e0be44a1f8ce5bdc999d6a53328edc01))
* add sigv4-preserving s3 streaming proxy for external buckets ([11095dd](https://github.com/CSFX-cloud/CSFX-Core/commit/11095dde2fdf8ad2bad540170a7567b921b47690))
* add spotlight command palette with cmd+k shortcut ([67f069e](https://github.com/CSFX-cloud/CSFX-Core/commit/67f069eea1c6364949ccde3e9b595a9434a5170b))
* add Storybook with stories for every lib component ([9a0a4f7](https://github.com/CSFX-cloud/CSFX-Core/commit/9a0a4f7843e096e0b447c129f873896a3e2d920b))
* add welcome header with 24h uptime to dashboard and drop donut card backgrounds ([feb50c0](https://github.com/CSFX-cloud/CSFX-Core/commit/feb50c0b2b1f4afc1064462810487180eca7d8d4))
* add workload self-healing with real capacity scheduling and working failover ([fbb344e](https://github.com/CSFX-cloud/CSFX-Core/commit/fbb344e72308775091fe021c3d73874855411027))
* added admin page and update page for cluster ([ca13d40](https://github.com/CSFX-cloud/CSFX-Core/commit/ca13d40957e93aebc45dd9f29a81f38a361b9231))
* added all frontend auth pages ([3ace818](https://github.com/CSFX-cloud/CSFX-Core/commit/3ace8180c249a4f77fbf979e91e61accf26208ab))
* added frontend for one mono repo ([924865b](https://github.com/CSFX-cloud/CSFX-Core/commit/924865be5002aa93b276c9aa4f8945e952b6cb7d))
* added gh token for image pulling without rate limiting ([b62b2d1](https://github.com/CSFX-cloud/CSFX-Core/commit/b62b2d188762d01e489ad1c8a160db007c7797c6))
* added graph with histroy ([4e73dc5](https://github.com/CSFX-cloud/CSFX-Core/commit/4e73dc5aa8cf0dce4f5b952f190296cdc6a32d9d))
* added heartbeat logic to agent ([a54df53](https://github.com/CSFX-cloud/CSFX-Core/commit/a54df53dfd1fe58a691009477da5729dc75dd6f8))
* added heartbeat logic to agent ([04d3716](https://github.com/CSFX-cloud/CSFX-Core/commit/04d3716739e49107b53b4e8ee508dd402aa420d9))
* added logo in header for svelte ([48190b7](https://github.com/CSFX-cloud/CSFX-Core/commit/48190b7863416a061e5ef8ca9e6ad83e46d78e22))
* added new alpha github run ([40b7af5](https://github.com/CSFX-cloud/CSFX-Core/commit/40b7af555a8e0e84a76929fe9b0425b45974bdaf))
* added new alpha github run ([1ec1a76](https://github.com/CSFX-cloud/CSFX-Core/commit/1ec1a765b8c92a8c67e07c6749087b5728a34fe1))
* added own patroni image ([f4b1938](https://github.com/CSFX-cloud/CSFX-Core/commit/f4b1938627f24038ce548ecb6676b9fc5f5844d0))
* added renovate ([5d0b547](https://github.com/CSFX-cloud/CSFX-Core/commit/5d0b5478160f1749ee57eca875f1c67879daa917))
* added renovate via github actions ([18de04d](https://github.com/CSFX-cloud/CSFX-Core/commit/18de04de9b54478b7a459f65b286fba0175c0173))
* added ressource groups ([e818f2f](https://github.com/CSFX-cloud/CSFX-Core/commit/e818f2f9f29fe2b58b2561c7855e8e046668a721))
* added update mech ([0a33b2b](https://github.com/CSFX-cloud/CSFX-Core/commit/0a33b2b513324ac0812b34d920b24274f638a325))
* added update stop and resume ([fd01b36](https://github.com/CSFX-cloud/CSFX-Core/commit/fd01b361aa31869dd27daadb7694e54f6265c844))
* added user name in sidebar and added nodes page demo for testing ([60f7e41](https://github.com/CSFX-cloud/CSFX-Core/commit/60f7e415c0273d199dfc1e62303969d82160296b))
* **agent:** log update signal when desired flake rev changes ([0cfbcb6](https://github.com/CSFX-cloud/CSFX-Core/commit/0cfbcb6a6408909f413f5594652ddcc8f745f5fa))
* allocate and configure guest ip for firecracker rg mesh ([f891fea](https://github.com/CSFX-cloud/CSFX-Core/commit/f891fea2164419cd99ef383654fc9bb58369515a))
* allow reusing existing iso images when deploying a vm ([4cf9c20](https://github.com/CSFX-cloud/CSFX-Core/commit/4cf9c20e6426be7c51fc0c2b87fda23d364f38e2))
* animated login button and fail modal ([fcf45a2](https://github.com/CSFX-cloud/CSFX-Core/commit/fcf45a2725250c86663cea4c00a2e8fcee5e4907))
* **api-gateway:** restrict system update endpoint to admin-only via RBAC ([988342b](https://github.com/CSFX-cloud/CSFX-Core/commit/988342b237539371555fcb2e97048bb8c28f39f0))
* attach rbd volumes to firecracker workloads via mmds ([6cbbb49](https://github.com/CSFX-cloud/CSFX-Core/commit/6cbbb4953410e69028b5a23fe898782dc58233be))
* bring up per-resource-group wireguard mesh with persistent agent identity ([72cb91f](https://github.com/CSFX-cloud/CSFX-Core/commit/72cb91f5fb10fc201a774a8dea454dcc79d799d9))
* build and publish csfx-guest-init binary in release and prerelease pipelines ([84fcdea](https://github.com/CSFX-cloud/CSFX-Core/commit/84fcdea0496e96d2965665da72c2a8203ac258e6))
* build and publish csfx-guest-init binary in release pipeline ([56aabd9](https://github.com/CSFX-cloud/CSFX-Core/commit/56aabd941018a6098bd3dca1773b77918c7f362f))
* build csf-updater locally via nix and publish alpha binaries to github releases ([eb3df31](https://github.com/CSFX-cloud/CSFX-Core/commit/eb3df3135649fc4796ce05861a60c393c7ad942c))
* build csf-updater locally via nix and publish alpha binaries to… ([3d759e1](https://github.com/CSFX-cloud/CSFX-Core/commit/3d759e1091b9d3dedbc2bf6d98dac0b33d1ad4ae))
* build firecracker rootfs images via oci-client instead of docker ([ac92280](https://github.com/CSFX-cloud/CSFX-Core/commit/ac922801ea8aaa37f8b587e3f542d893bae7d8d5))
* cluster-wide bootstrap tokens ([b479ec6](https://github.com/CSFX-cloud/CSFX-Core/commit/b479ec65937ad0f4569346e2b4f921b0fd5d0a5f))
* collect real cpu memory and network stats for firecracker vms ([51b8c9d](https://github.com/CSFX-cloud/CSFX-Core/commit/51b8c9df8123b4cb3630c9fd8abbac566fb8f517))
* consolidate add bucket into add resource picker and add buckets overview page ([ac451d7](https://github.com/CSFX-cloud/CSFX-Core/commit/ac451d7379cb31bbfdc6580363360dfe28707b6a))
* **cp:** wire csfx-updater as systemd service and rename units ([5bd062e](https://github.com/CSFX-cloud/CSFX-Core/commit/5bd062e4c5941b2c49ce23c76b4f6a92895eb466))
* **csf-updater:** implement secure rust-based updater daemon with nixos integration ([eb065a0](https://github.com/CSFX-cloud/CSFX-Core/commit/eb065a03514fef1e4997da2e2169b0b783cb9de8))
* **csf-updater:** verify image digests against GHCR before applying update ([f791f42](https://github.com/CSFX-cloud/CSFX-Core/commit/f791f42a5fbbe9cd1330424d35539608cc384471))
* **csfx-updater:** add observability logs across update pipeline ([fcd2495](https://github.com/CSFX-cloud/CSFX-Core/commit/fcd249595d31a18d03d89fc476ec4f3625770703))
* docker compose prod ([d5ebd58](https://github.com/CSFX-cloud/CSFX-Core/commit/d5ebd58bebb3dcf32caa80570de00685bb44f982))
* drop jailer root privileges via uid gid allocator ([0a54b98](https://github.com/CSFX-cloud/CSFX-Core/commit/0a54b9815ec917412e40548381305238c49a5c6a))
* enforce resource group network isolation with per-rg bridges and nftables ([7dd3533](https://github.com/CSFX-cloud/CSFX-Core/commit/7dd35330c3cfd9f44a225bac0c73622d4cda4c6c))
* **etcd:** enable authentication and restrict access to csf service user ([ba2a887](https://github.com/CSFX-cloud/CSFX-Core/commit/ba2a88718a38deda7a82e515052456f741256052))
* expose agent and updater binary versions in update-status ([08030a9](https://github.com/CSFX-cloud/CSFX-Core/commit/08030a9bf54add66b3d8ad237a0b440cdd55ddc9))
* force cgroup v2 hierarchy in guest kernel boot args ([0f9ff80](https://github.com/CSFX-cloud/CSFX-Core/commit/0f9ff80b758d1bd6a56dc96b489a39ec59597093))
* generate oci runtime spec config.json instead of shell entrypoint ([688620b](https://github.com/CSFX-cloud/CSFX-Core/commit/688620bad501c7efc7a94de61c97989ec7f1b975))
* implement gitops poller, git mirror, and nix build pipeline in csf-updater ([d08736e](https://github.com/CSFX-cloud/CSFX-Core/commit/d08736eac471364bdbd3893d6004fa9d96c1bf8f))
* implement watchdog heartbeat counter in registry and csf-updater ([3024db4](https://github.com/CSFX-cloud/CSFX-Core/commit/3024db49e879db9b03123693ef321afde3d576a8))
* inject bucket credentials into workload env on bucket binding ([834144f](https://github.com/CSFX-cloud/CSFX-Core/commit/834144f877790a7f844553e9960efcfacd97bb67))
* inject CSF_BUILD_VERSION into binaries at compile time via build.rs ([8d18efb](https://github.com/CSFX-cloud/CSFX-Core/commit/8d18efb8a2fdc7a73aa5f6c2aa7357800163bfc5))
* log workload output immediately and silence expected /dev EBUSY warning ([4c6f71b](https://github.com/CSFX-cloud/CSFX-Core/commit/4c6f71b5e2a3b85cf9de71c32450e3945bab0101))
* login flow ([9f18045](https://github.com/CSFX-cloud/CSFX-Core/commit/9f18045554f0cba824de209bd588eec83197937e))
* new nix config ([26ea9cc](https://github.com/CSFX-cloud/CSFX-Core/commit/26ea9cc9449148345546c23e6d56487ef442b96b))
* propagate agent version on every heartbeat and add live node metrics tunnel ([72c46e3](https://github.com/CSFX-cloud/CSFX-Core/commit/72c46e3ad47056116611a9c7140442e17223f86b))
* propagate desired_flake_rev via heartbeat response to agent update trigger ([97914a6](https://github.com/CSFX-cloud/CSFX-Core/commit/97914a6e052340d11571efce801b00d82b45583b))
* propagate post_update_heartbeats counter to agent for watchdog health check ([88c0051](https://github.com/CSFX-cloud/CSFX-Core/commit/88c005146f9afc59051f707160ca3d2eb1aff8bc))
* push workload assignments to agents instead of relying on poll interval ([993e29b](https://github.com/CSFX-cloud/CSFX-Core/commit/993e29b5ba1ee782d9ffcf06b278a88c0f709d58))
* reconcile firecracker vms from disk state on agent restart ([4fe23df](https://github.com/CSFX-cloud/CSFX-Core/commit/4fe23df95626960b1f2826d50bad3585d23a7652))
* redesign node detail page with load simulation ([1b9c3d6](https://github.com/CSFX-cloud/CSFX-Core/commit/1b9c3d6472e4fa0a24a381b9e042f32bca23d873))
* redesign nodes page with filter panel, sortable metrics table, and search header ([4c31261](https://github.com/CSFX-cloud/CSFX-Core/commit/4c312614509408216786e37fe3426ddd3a6be525))
* remove docker runtime and fix firecracker feature gaps exposed by removal ([b79f429](https://github.com/CSFX-cloud/CSFX-Core/commit/b79f429e11244424dec297a5b94828fc4fc5f5c5))
* renam csf to csfx ([6888431](https://github.com/CSFX-cloud/CSFX-Core/commit/6888431b8a52a4827bc9493f7897534eb6b201f1))
* replace flake-rev API with version-based update scheduling ([64dfe62](https://github.com/CSFX-cloud/CSFX-Core/commit/64dfe62b5724c642b6b6d26d30fc6cb2bc6c48dd))
* replace login split panel with full-bleed background and floating card ([f6dd88a](https://github.com/CSFX-cloud/CSFX-Core/commit/f6dd88af411b2d3f46ebfb861fec07d2fa856bfd))
* replace node detail slider with dedicated page ([37a496b](https://github.com/CSFX-cloud/CSFX-Core/commit/37a496be3fae2dd7be691b0157b037b9acbee111))
* replace node icon svg with server image on node detail page ([1fb8f3c](https://github.com/CSFX-cloud/CSFX-Core/commit/1fb8f3ce8c85bcd01b2d92c57c861d0facf36775))
* replace resource group dialogs with shared-sidebar pages for co… ([7b585cf](https://github.com/CSFX-cloud/CSFX-Core/commit/7b585cfd99ac0afb581f1edafa9f61258aae85cd))
* replace resource group dialogs with shared-sidebar pages for containers, stacks and add resource ([1703fdb](https://github.com/CSFX-cloud/CSFX-Core/commit/1703fdb8fb716ea821176dfa8c0f7f8edc17016e))
* replay boot-time log backlog to new log stream clients ([0263911](https://github.com/CSFX-cloud/CSFX-Core/commit/0263911af96c33781e89b28b3807393c3cc7f684))
* replay boot-time log backlog to new log stream clients ([9b0f011](https://github.com/CSFX-cloud/CSFX-Core/commit/9b0f011aaf1b3580307f7b6d99cdf439a10ba437))
* **resource-groups:** add container deploy, volume management, and detail view ([7b32db8](https://github.com/CSFX-cloud/CSFX-Core/commit/7b32db8ee1dacf56062f96dee4f1649048f13464))
* restructure nodes page layout and add search bar ([af8ebd3](https://github.com/CSFX-cloud/CSFX-Core/commit/af8ebd387b87c1f55d6bb28b33126b6c4f5e0df0))
* run container workloads through libcontainer instead of hand-rolled init ([b4f71d5](https://github.com/CSFX-cloud/CSFX-Core/commit/b4f71d5972639741055ceb6ec18379331d0a4d49))
* serve frontend statically from api-gateway on port 8000 ([69f0ad8](https://github.com/CSFX-cloud/CSFX-Core/commit/69f0ad848167879a504f87083eadf7455596df4f))
* **ssh:** add ephemeral key-based SSH access to cluster nodes ([e4940e7](https://github.com/CSFX-cloud/CSFX-Core/commit/e4940e727322312bbf24dfc911f22acb24cb9612))
* **ui,agent,registry:** add WireGuard P2S VPN, modal forms, and NodePort expose model ([d8c098b](https://github.com/CSFX-cloud/CSFX-Core/commit/d8c098b71064d1e3c1ec32e66d21821ab90e63a8))
* unify user and admin settings into single dialog ([0e050a8](https://github.com/CSFX-cloud/CSFX-Core/commit/0e050a89bccb2f7f11abb2b9ba51fdfbde4de884))
* use mutable binary paths for csf-agent and csf-updater to enable self-update ([64e4326](https://github.com/CSFX-cloud/CSFX-Core/commit/64e4326d174a440a911e8b46f024866dc607e2e8))
* use vcpu and gb units for vm resource form with advanced toggle ([1115e1d](https://github.com/CSFX-cloud/CSFX-Core/commit/1115e1d33f89729e0ade59e500af7aa3b0c804b3))
* wire firecracker rg networking, dns and wireguard without docker ([ac082cc](https://github.com/CSFX-cloud/CSFX-Core/commit/ac082ccbfd9bf6099427d64f96b39af25ae8c43f))
* wire object-storage into api-gateway with rbac and tenant-scoped bucket listing ([c62c7d6](https://github.com/CSFX-cloud/CSFX-Core/commit/c62c7d64fe116f187d81e6a1355560f2d968e6b0))
* wire real pty exec and rootfs entrypoint into guest-init ([0c2d502](https://github.com/CSFX-cloud/CSFX-Core/commit/0c2d5029caa761431691cebf2d9b2635b2933d5e))
* wire vm networking, iso upload and vnc console frontend ([14bd46a](https://github.com/CSFX-cloud/CSFX-Core/commit/14bd46aaae14aedddad39aabcf8cc023695220b8))


### Bug Fixes

* accept self-signed gateway cert when downloading vm iso images ([7acdc3f](https://github.com/CSFX-cloud/CSFX-Core/commit/7acdc3f9cfe00f7353233e7ec55419fe8cbfb771))
* add dedicated vm filter tab instead of counting vms as containers ([903543b](https://github.com/CSFX-cloud/CSFX-Core/commit/903543bfcc22336000d38a5d4136789557cb4bb2))
* add explicit host route to mmds address before fetching guest config ([b1a5f68](https://github.com/CSFX-cloud/CSFX-Core/commit/b1a5f684c5e1e3ba3791be1669f48163dad892ed))
* add object-storage crate to control-plane docker build ([19cbb00](https://github.com/CSFX-cloud/CSFX-Core/commit/19cbb00224c79a86ffde1a36d9f7c7aed458787b))
* add object-storage crate to docker build and compose ([2add4b1](https://github.com/CSFX-cloud/CSFX-Core/commit/2add4b15d06f19a280779db365cd454b9710fd25))
* add patroni bootstrap script to create csf app user and database ([0e07850](https://github.com/CSFX-cloud/CSFX-Core/commit/0e07850de14f5ac35098d4acaa9e2248a5fae7b5))
* added log for testing update flow ([8aaeef0](https://github.com/CSFX-cloud/CSFX-Core/commit/8aaeef0ff44c77e752e04c9dcc5fc0c5a57264bb))
* added log for testing updater ([2728d06](https://github.com/CSFX-cloud/CSFX-Core/commit/2728d06c157b95e2d9124fc2e449c9411075c978))
* added logging for testing ([920e730](https://github.com/CSFX-cloud/CSFX-Core/commit/920e7300a4eacf5d9ce3291bfc2ed38b35ffba7e))
* agent ([b09048e](https://github.com/CSFX-cloud/CSFX-Core/commit/b09048e54df3c4e815f1c095e2af43cd71fc5eb2))
* agent bootstrap error ([013b2c8](https://github.com/CSFX-cloud/CSFX-Core/commit/013b2c899925240cbc667b88052d27bb2c536e7c))
* agent error ([a6b0bbe](https://github.com/CSFX-cloud/CSFX-Core/commit/a6b0bbe86e6de90e9bcab2a1df9a4bed74b33e55))
* **agent:** detect OS from /etc/os-release instead of sysinfo ([1d86d69](https://github.com/CSFX-cloud/CSFX-Core/commit/1d86d69e1afe067bb5fdb6507cc0e202ecc68681))
* **agent:** rename state dir from csfx-daemon to csfx-agent ([0ee1896](https://github.com/CSFX-cloud/CSFX-Core/commit/0ee18965f068a7e526fb64b87bdf3ffb75dcde4a))
* **agent:** start docker on demand via systemd instead of at boot ([86a2d7c](https://github.com/CSFX-cloud/CSFX-Core/commit/86a2d7c4029d721ae51591c79011fe41d9e7813f))
* allow mmds requests on guest network interface ([3d0e59e](https://github.com/CSFX-cloud/CSFX-Core/commit/3d0e59e7c87e9687d90f5aa4833686278e760c00))
* always accept new log vsock client instead of gating on stale connection ([1942fc2](https://github.com/CSFX-cloud/CSFX-Core/commit/1942fc2af072051b3205cd5c572898acc934a19f))
* **api-gateway:** exempt update status routes from rate limiting ([fd08546](https://github.com/CSFX-cloud/CSFX-Core/commit/fd08546c131ee1d91ca53aa3ff20117fe678d58c))
* **api-gateway:** make TLS config generation async to avoid block_on panic ([ba21420](https://github.com/CSFX-cloud/CSFX-Core/commit/ba2142097d629b40b52eef149c11d7dc8c179fd9))
* append resource group search domain to vpn client dns config ([cd417f7](https://github.com/CSFX-cloud/CSFX-Core/commit/cd417f7560bc488f20b71b2b4aee75552c0ad501))
* assign mmds bootstrap address before guest network fetch ([44373d6](https://github.com/CSFX-cloud/CSFX-Core/commit/44373d6ad0dfeee6f0d77e42cb15e7ef10e70425))
* assign storage capacity from disk on first garage registration ([915c10a](https://github.com/CSFX-cloud/CSFX-Core/commit/915c10a238955fadbd4afee1bd39d1dd2d2c344c))
* attach firecracker tap devices to rg bridge and clean up stale rg networks ([8591a8e](https://github.com/CSFX-cloud/CSFX-Core/commit/8591a8ee676e8c190204cb74d25430cd20abcb5a))
* auto-assign workloads to agents and wire up management wireguard tunnel ([e1fa45a](https://github.com/CSFX-cloud/CSFX-Core/commit/e1fa45a2dc02bfc60ef165cd7795a2ff5104732c))
* backfill missing management tunnel ip for agents on heartbeat ([d644977](https://github.com/CSFX-cloud/CSFX-Core/commit/d644977479826e40c1a081c517cb345200c845b5))
* boot vm from disk first so restarts skip the installer iso ([83cadf9](https://github.com/CSFX-cloud/CSFX-Core/commit/83cadf9cdf25d805ab5948ef678b9ca59ff79753))
* bound mmds fetch with timeout and read exact content-length ([ce65500](https://github.com/CSFX-cloud/CSFX-Core/commit/ce655003f6766e2b7840bae5d72668d39a8c0b16))
* build error  with updater hash ([839fd28](https://github.com/CSFX-cloud/CSFX-Core/commit/839fd2836017f3829bc997cea6e8adfe71934719))
* build errors and added frontend ([b1cb885](https://github.com/CSFX-cloud/CSFX-Core/commit/b1cb885b283d785190683537bc40100ade67d0b9))
* bump nixpkgs to 25.05 for Cargo 1.85/edition2024 support ([ddb75f3](https://github.com/CSFX-cloud/CSFX-Core/commit/ddb75f3419e9dcc2fa3a724c96afb93f708893c7))
* bump node version to 22 in release workflows ([657f2f0](https://github.com/CSFX-cloud/CSFX-Core/commit/657f2f0b8213b1d7901b095eed2c6a39e8724381))
* cargo fmt ([93ce25f](https://github.com/CSFX-cloud/CSFX-Core/commit/93ce25f4ee98af4bed40f9f828b9c75119a8450d))
* cert issue ([f544e36](https://github.com/CSFX-cloud/CSFX-Core/commit/f544e36724fcdef4408c7c93ddd40c98bf3ffcc6))
* clean up rootfs extract directory before and after failed layer extraction ([95f9c57](https://github.com/CSFX-cloud/CSFX-Core/commit/95f9c5778162ee78ddd1b90e00dbe39e788372b5))
* clean up stale jailer chroot directory before each start attempt ([4c756a0](https://github.com/CSFX-cloud/CSFX-Core/commit/4c756a082930f207431181d406124d3e73fe334c))
* coerce port fields to string before trim in resource group deploy form ([49d667a](https://github.com/CSFX-cloud/CSFX-Core/commit/49d667aab28eba045760716417f630c8c5c1e0d7))
* correct cgroup path to jailer systemd scope ([bd44a16](https://github.com/CSFX-cloud/CSFX-Core/commit/bd44a1679cae01fcd59dd93c2640eca116664765))
* correct cgroup path to jailer systemd scope, not a nonexistent parent-cgroup dir ([4bc7db5](https://github.com/CSFX-cloud/CSFX-Core/commit/4bc7db5766af5d21a8cedbb8d5f6328ada90a59b))
* correct node status colors and add filter/sort dropdowns to node table header ([b0e3687](https://github.com/CSFX-cloud/CSFX-Core/commit/b0e36879e484eb5bb1848605ebf020210fe3a656))
* correct pre-release version comparison and let forced updates bypass pause ([46266c7](https://github.com/CSFX-cloud/CSFX-Core/commit/46266c7801cebac8a26931f3834276c3be402049))
* correct rootfs extraction, capabilities and log draining for container workload startup ([84f8eed](https://github.com/CSFX-cloud/CSFX-Core/commit/84f8eeda7f78a7edefbcc8d271fe22591833b5f3))
* correct sigv4 encoding and garage node status tracking ([d41bda1](https://github.com/CSFX-cloud/CSFX-Core/commit/d41bda182eee16e14ae8de4b554e2cad07303994))
* create csfx role and database during patroni bootstrap ([7d0486c](https://github.com/CSFX-cloud/CSFX-Core/commit/7d0486c8e6ac3ac890674804932aca399517de5e))
* create metrics fifo before configuring firecracker metrics endpoint ([b412801](https://github.com/CSFX-cloud/CSFX-Core/commit/b4128013ea5cfa37c54e693129f66ab97be9866c))
* create standard guest root directories missing after bundle layout change ([cb700f2](https://github.com/CSFX-cloud/CSFX-Core/commit/cb700f2984252ab45710687ae7cd66f7c91bd1c5))
* **csf-updater:** run as dedicated system user with docker group instead of root ([22cf1e5](https://github.com/CSFX-cloud/CSFX-Core/commit/22cf1e5adabe3f5d23ee64ad7c383ce9cd5d55bb))
* **csf-updater:** validate version string from etcd before executing update ([13e9fdf](https://github.com/CSFX-cloud/CSFX-Core/commit/13e9fdf8630c7f5e3468d16fab26ff77809c60cd))
* **csfx-updater:** add --no-out-link to nixos-rebuild build to avoid permission error ([0b8e472](https://github.com/CSFX-cloud/CSFX-Core/commit/0b8e472c52e23c34b3ce8fda40c3a235f06a5ee0))
* dep update axum 0.7 -&gt; 0.8 ([9be3ff4](https://github.com/CSFX-cloud/CSFX-Core/commit/9be3ff45bc0aa781dd8393325465d89168526a63))
* deps reqwest 0.11 -&gt; 0.13 ([864c2db](https://github.com/CSFX-cloud/CSFX-Core/commit/864c2db5c9471e459c95a35eb1883175996516ca))
* **deps:** update dependency @icons-pack/svelte-simple-icons to v7 ([01dd084](https://github.com/CSFX-cloud/CSFX-Core/commit/01dd084ca79416b911ca6d1ab21b03dbb9e55191))
* **deps:** update dependency @icons-pack/svelte-simple-icons to v7 ([e5cef90](https://github.com/CSFX-cloud/CSFX-Core/commit/e5cef90fec7f18b90831d4b63ec998a38a6a8fa4))
* **deps:** update rust dependencies ([2fe8332](https://github.com/CSFX-cloud/CSFX-Core/commit/2fe83321c0242fad8aa821ed613194d192e1f6df))
* **deps:** update rust dependencies ([b2e32f0](https://github.com/CSFX-cloud/CSFX-Core/commit/b2e32f0139aa08337237fb28198288597c24c91e))
* dev bootstrap ([1547ee8](https://github.com/CSFX-cloud/CSFX-Core/commit/1547ee838877589f000946f0278a70baff698f26))
* disable firecracker seccomp filter incompatible with musl syscalls ([9b63df4](https://github.com/CSFX-cloud/CSFX-Core/commit/9b63df4a0a2f55e0879bbe4019cf2f15fa5c72f6))
* disable oci-client default features to remove aws-lc-sys from musl builds ([e5b09c6](https://github.com/CSFX-cloud/CSFX-Core/commit/e5b09c6ed21be1f0a243c474c34100650e3394e0))
* disable rustls default aws-lc-rs feature to fix musl aarch64 link ([8c61471](https://github.com/CSFX-cloud/CSFX-Core/commit/8c61471f44871a7fd3636d1361cdd068ad3169b4))
* docker access in updater error ([ce45cba](https://github.com/CSFX-cloud/CSFX-Core/commit/ce45cbab1f20aa43a0b2d64c45b72fa3b2979183))
* docker prod build ([581a20d](https://github.com/CSFX-cloud/CSFX-Core/commit/581a20d4b0d8c938cbd541c099d509780d781fdc))
* docker updater error ([d8f7457](https://github.com/CSFX-cloud/CSFX-Core/commit/d8f7457e5412d5397f1b0cee250503c74fdb9f65))
* eliminate aws-lc-sys from musl build by switching to ring crypto provider across all services ([4045fbb](https://github.com/CSFX-cloud/CSFX-Core/commit/4045fbbfe3435c33c67fab4e62730f9c1258e665))
* enable firecracker seccomp filter in jailer spawn ([005395f](https://github.com/CSFX-cloud/CSFX-Core/commit/005395f86123dd07a06479e6d39faf7ed725ec5e))
* enable firecracker seccomp filter in jailer spawn ([30b3a08](https://github.com/CSFX-cloud/CSFX-Core/commit/30b3a0886a7b58ba1962576535a38d146dc60adb))
* etcd connection error ([1a44dc1](https://github.com/CSFX-cloud/CSFX-Core/commit/1a44dc18fdecb233a6c2aa06819508f2eef3564d))
* **etcd:** block etcd ports from external access via firewall rules in install script ([34e1cd2](https://github.com/CSFX-cloud/CSFX-Core/commit/34e1cd26ccd9fc8a40dd4f681ed5e9cff1e281cb))
* exec into running container namespace instead of vm root ([386a401](https://github.com/CSFX-cloud/CSFX-Core/commit/386a4017327e880725f110f05a2c7bcd2e559fbf))
* exempt agent heartbeat/workloads/volumes routes from rate limiting ([1b4d3c3](https://github.com/CSFX-cloud/CSFX-Core/commit/1b4d3c399a69f69953dacb7a0ab3dc7927d81901))
* filter noisy access, sql, heartbeat and performance events from log storage ([a6f7510](https://github.com/CSFX-cloud/CSFX-Core/commit/a6f751056c11bfbcd2dc548ebab909f49796e9b7))
* fix rg dns write sandbox failure and reconcile orphaned containers on agent restart ([b6f2a5d](https://github.com/CSFX-cloud/CSFX-Core/commit/b6f2a5dcbbff140320925e5fbb994f532b8c8392))
* flush log stream headers immediately and keep container stdio pipes alive ([0d864f8](https://github.com/CSFX-cloud/CSFX-Core/commit/0d864f80b6652c8098be0173b7e8c7c9233781cd))
* for local dev env ([5decf9b](https://github.com/CSFX-cloud/CSFX-Core/commit/5decf9bbd747d66c597073992130b78e9d6b2a5c))
* format ([73ab945](https://github.com/CSFX-cloud/CSFX-Core/commit/73ab9453e3fdd4f2cbecaa81ed27408b2dc0eab7))
* **gateway:** exempt registry routes from rate limiting ([c9f30e9](https://github.com/CSFX-cloud/CSFX-Core/commit/c9f30e96cc9f1a59bcbea6efc885597d1139cdbb))
* github pipeline ([791c518](https://github.com/CSFX-cloud/CSFX-Core/commit/791c518c193985d6233768285eb34356a633ddf9))
* grant standard container capabilities to workload processes ([c262358](https://github.com/CSFX-cloud/CSFX-Core/commit/c262358153a71166e712ac473ac3bcc411dc2c33))
* handle 429 rate limit in agent registration with retry backoff ([31d3c29](https://github.com/CSFX-cloud/CSFX-Core/commit/31d3c29e80c596dc2395bdb9b178f1b3c8d527c6))
* image version ([cd9b47c](https://github.com/CSFX-cloud/CSFX-Core/commit/cd9b47ce0cbfb59292ba28fdfc173c5c0f2e9914))
* include guest-init binary version in rootfs cache key ([128f562](https://github.com/CSFX-cloud/CSFX-Core/commit/128f562eba644f795b0270aee90061d2fec3f8c2))
* include response body in firecracker api error messages ([3967e93](https://github.com/CSFX-cloud/CSFX-Core/commit/3967e93425e3cb766046f1282f4bf5a00739804e))
* install rustls aws-lc-rs crypto provider before TLS init ([5ccb37d](https://github.com/CSFX-cloud/CSFX-Core/commit/5ccb37d8e92f12b18b22dc4b85713435fe1e6668))
* keep pending login credentials out of the otp url ([3a7e7ca](https://github.com/CSFX-cloud/CSFX-Core/commit/3a7e7ca4d62d2f3df675e2c8574d0d2a87161a54))
* load times on svg main ([e37e88b](https://github.com/CSFX-cloud/CSFX-Core/commit/e37e88bab79d1e3e040c5446250ce93f1fd59d0e))
* load xterm dynamically to avoid SSR CJS named export error ([db41761](https://github.com/CSFX-cloud/CSFX-Core/commit/db417613a86ae79ffa50d4c10706ab71bb0ffc22))
* lock state when updater go into error ([28c0247](https://github.com/CSFX-cloud/CSFX-Core/commit/28c0247dabb89763f2191c4d8fe95854bf7f8663))
* log full error chain for mmds route and fetch failures ([84f4b54](https://github.com/CSFX-cloud/CSFX-Core/commit/84f4b548b8f3b154a7af97d820e5f43118fc4907))
* log vsock accept guard blocks reconnects after first client ([1fd8ef6](https://github.com/CSFX-cloud/CSFX-Core/commit/1fd8ef69a0ba8d081262bb97bebc62772be3240a))
* make bootstrap registration idempotent by upserting on hostname ([98a935a](https://github.com/CSFX-cloud/CSFX-Core/commit/98a935a7f89bb901b5a3314854016e60a70c9950))
* make firecracker binary path configurable via env var ([da9fa98](https://github.com/CSFX-cloud/CSFX-Core/commit/da9fa980844b65ae441866943ca275fc59c59044))
* manifest build ([dd5d522](https://github.com/CSFX-cloud/CSFX-Core/commit/dd5d522b846b34fb8243257201a84c45a6203c4a))
* master node auto-bootstrap — self-register agent via admin API on first boot ([1d64a1f](https://github.com/CSFX-cloud/CSFX-Core/commit/1d64a1f205d642c5ae18d5fc67447735576247cc))
* match firecracker cmdline after jailer execve for pid lookup ([3940ee4](https://github.com/CSFX-cloud/CSFX-Core/commit/3940ee4526ecd24e85a62e098f04b830897e9023))
* match garage 2.x admin api schema for update cluster layout ([0c8cc67](https://github.com/CSFX-cloud/CSFX-Core/commit/0c8cc6793550c873f55bb94a1b54d22291cd22f7))
* match workload cgroup path to jailer short id for accurate metrics ([eba2df6](https://github.com/CSFX-cloud/CSFX-Core/commit/eba2df60ed08871bd0b851958fa0fefbc3824443))
* metrics error agent ([4ac4ce8](https://github.com/CSFX-cloud/CSFX-Core/commit/4ac4ce842d18dfa0d281d83b59628855d8de7207))
* migrate axum routes to 0.8 syntax and downgrade sysinfo to 0.32 ([d29d12d](https://github.com/CSFX-cloud/CSFX-Core/commit/d29d12d6ca49a200fd4c864d6c40e8404debc20f))
* migration in docker dev enviroment and auth fix frontend ([8140686](https://github.com/CSFX-cloud/CSFX-Core/commit/81406865af1f2aa268543472304da83f764ea8ce))
* mount devpts in guest-init to resolve pty ENOENT on exec ([0d18f12](https://github.com/CSFX-cloud/CSFX-Core/commit/0d18f1286a78ad23ae3eda397be73b5cb5bd8869))
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
* nodes offline check ([353f803](https://github.com/CSFX-cloud/CSFX-Core/commit/353f803e93d9ac069778e6263c96b312f61fb753))
* only report qemu vm as failed on genuine qmp error states ([d630691](https://github.com/CSFX-cloud/CSFX-Core/commit/d630691f08950af32848b06af87949e70360fbc6))
* parse firecracker api response by content-length instead of eof ([9c4d0b4](https://github.com/CSFX-cloud/CSFX-Core/commit/9c4d0b48d876d71ab670d6336ecec3227698f690))
* pass rg_dns_registry as arc to heartbeat loop ([f0e6738](https://github.com/CSFX-cloud/CSFX-Core/commit/f0e6738c6e0bf74b7a78986fbfe0660cee7b3350))
* path error in updater ([e10e08f](https://github.com/CSFX-cloud/CSFX-Core/commit/e10e08fa5884573e89b3a08b40f569055e0c1eb3))
* patroni bootstrap error ([5895235](https://github.com/CSFX-cloud/CSFX-Core/commit/5895235c801f11cba514de1d52d4d75d0cede78e))
* patroni internal error from bootstrap ([d52bc00](https://github.com/CSFX-cloud/CSFX-Core/commit/d52bc00e1fac28e20e18daeb7f62c7dde72b6b11))
* pin rust 1.88.0 via rust-overlay for edition2024/time crate support ([2b6bc89](https://github.com/CSFX-cloud/CSFX-Core/commit/2b6bc897531313710e9021533aafa3fc5d8ebdcf))
* pipeline build error ([ddbfc81](https://github.com/CSFX-cloud/CSFX-Core/commit/ddbfc81032daae687355fd3fba90b7ea7662a820))
* point resource group Corefile at actual zone file directory ([8f9c4a6](https://github.com/CSFX-cloud/CSFX-Core/commit/8f9c4a684740ba65ac778361786178d6cd31ffea))
* preserve original path encoding in s3 proxy and simplify presign url ([b31e0be](https://github.com/CSFX-cloud/CSFX-Core/commit/b31e0be8061972011586ec3f9eb6d8514dab32e6))
* proxy api requests to https gateway with insecure flag for dev ([d65b24c](https://github.com/CSFX-cloud/CSFX-Core/commit/d65b24c60f01d4b4f1201ba6f1794f2d8ccdd16b))
* rate limit by authenticated user instead of shared ip bucket ([4f166c3](https://github.com/CSFX-cloud/CSFX-Core/commit/4f166c3a972489be07eeb1fc745605a6055e211f))
* rate-limit agents by API key hash instead of shared IP bucket ([d4ab5ff](https://github.com/CSFX-cloud/CSFX-Core/commit/d4ab5ffb7053b82a2aa49b6921bb673279a54332))
* reach docker agents by hostname when tunnel is unavailable ([9dd8c14](https://github.com/CSFX-cloud/CSFX-Core/commit/9dd8c146e033737883319259a1d80fbaaaf0657e))
* reap correct container pid in guest-init to stop premature log task teardown ([3c2f109](https://github.com/CSFX-cloud/CSFX-Core/commit/3c2f109ea55bdc94b190561b22dfe237c34bdc2e))
* recover stuck vsock log listener when host client disconnects silently ([e10bbda](https://github.com/CSFX-cloud/CSFX-Core/commit/e10bbda9921413312f0cdec90251803ff2ce73b4))
* recreate resource group dns container when stale instead of skipping ([b9949e5](https://github.com/CSFX-cloud/CSFX-Core/commit/b9949e5aecc78693629f75bbcbc23d44de9bad56))
* recreate stale dns containers, drop loopback proxy shortcut, fix stats history query, and handle 401 in frontend ([282e8cb](https://github.com/CSFX-cloud/CSFX-Core/commit/282e8cbd8d1fb610d3fe0ad690900b38e58360d4))
* register single-node garage deployments without a csfx agent ([077a2a0](https://github.com/CSFX-cloud/CSFX-Core/commit/077a2a023b794beffdb31b113e0674ff4ba415de))
* **registry:** make AgentStatus::from_str case-insensitive ([e80d89b](https://github.com/CSFX-cloud/CSFX-Core/commit/e80d89ba0b002a43fabbd02c5856493d9bd848c8))
* **registry:** migrate route params from :param to {param} syntax ([af9e2ec](https://github.com/CSFX-cloud/CSFX-Core/commit/af9e2ec51343dd228fb10e03498b6cedce8f16e1))
* reload hypervisor version reactively once auth token hydrates ([a8739e2](https://github.com/CSFX-cloud/CSFX-Core/commit/a8739e2af7b0ec9b877466c073555462b514f570))
* remove daemonize flag so systemd correctly tracks the qemu process ([f88e4a6](https://github.com/CSFX-cloud/CSFX-Core/commit/f88e4a6e3fb8c623e6ae19ad656a1c651e0cc454))
* remove dead ready-signal wait blocking guest-init boot ([491570b](https://github.com/CSFX-cloud/CSFX-Core/commit/491570b149916bb160906e056e6da6f459943462))
* remove log client liveness probe causing immediate disconnect in guest-init ([bf7c8d1](https://github.com/CSFX-cloud/CSFX-Core/commit/bf7c8d17cd6be79cdeecc1fe90e8abf86ba9ab2f))
* remove NoNewPrivileges to allow sudo systemctl for binary restart ([ddc22b8](https://github.com/CSFX-cloud/CSFX-Core/commit/ddc22b83f374fae4c0e88c13b66158a6fd95f730))
* remove ssh console button from node detail sheet ([5bc2905](https://github.com/CSFX-cloud/CSFX-Core/commit/5bc29051d2d5841f940b4d86f29e7f640ebc1d0f))
* remove unsupported allow_mmds_requests field from network-interface config ([6d12a37](https://github.com/CSFX-cloud/CSFX-Core/commit/6d12a3703da3b5ae30e8a6317445e84722a8ebc9))
* renovate ([1b87f10](https://github.com/CSFX-cloud/CSFX-Core/commit/1b87f1045250da00cef55a31afbe23326b8eccf6))
* repair jailer chroot cleanup and resource group dns for firecracker workloads ([182aa70](https://github.com/CSFX-cloud/CSFX-Core/commit/182aa700bfe536832b1aa76db527d50a838a9813))
* repair workload exec and log streaming across vsock, tls and lock paths ([2a0b9ab](https://github.com/CSFX-cloud/CSFX-Core/commit/2a0b9ab3110251fe90fb649c66caef6fa44077a2))
* replace aws-lc-rs with ring for musl aarch64 ([7fdc686](https://github.com/CSFX-cloud/CSFX-Core/commit/7fdc6863ba542fa110d17bcfb69a87f866706485))
* replace rustls-tls feature with rustls for reqwest 0.13 compat ([bf6d095](https://github.com/CSFX-cloud/CSFX-Core/commit/bf6d0958752c09bfadc2ef859a289ef28bbeaaa7))
* repo ([fa07190](https://github.com/CSFX-cloud/CSFX-Core/commit/fa07190eeb0a04c9d6087e4bb48c740a6545f63b))
* report disk size from block device instead of filesystem ([20c76c3](https://github.com/CSFX-cloud/CSFX-Core/commit/20c76c395bdbaf36631113c27c9376eda1df4187))
* report real node ip instead of loopback address ([a02705f](https://github.com/CSFX-cloud/CSFX-Core/commit/a02705f8da678d7a2505e7181317f89017fc91ca))
* report stopped status for stacks with all-stopped containers ([13c066d](https://github.com/CSFX-cloud/CSFX-Core/commit/13c066d05c4bc050990e624019d310c4f235b183))
* request mmds root path instead of latest/meta-data in guest-init ([eef9357](https://github.com/CSFX-cloud/CSFX-Core/commit/eef935742a9af651abba012bb051796f106e5a8c))
* resolve mirrored image references without docker mirror query protocol ([5376080](https://github.com/CSFX-cloud/CSFX-Core/commit/53760807963211af0776b2ba4964c71dce6b1c9b))
* resolve musl build failure for csf-updater binary ([50a89e8](https://github.com/CSFX-cloud/CSFX-Core/commit/50a89e88f6338073a52a272870213509c1001d82))
* restrict binary dir permissions and verify sha256 checksum on download ([2b3260f](https://github.com/CSFX-cloud/CSFX-Core/commit/2b3260fe91b3038cda9d7fb8edf9d8a498e6c643))
* retry metrics forwarding to gateway and log permanent failures as errors ([62f9224](https://github.com/CSFX-cloud/CSFX-Core/commit/62f92249ba4c42df495de9b129c4a34e270ab83b))
* rollback on failed update flow ([a1109d2](https://github.com/CSFX-cloud/CSFX-Core/commit/a1109d25996633e3af7b8f2e21c57592d7a7b29c))
* route agent proxy calls via loopback when target is the local WireGuard tunnel IP ([be9d879](https://github.com/CSFX-cloud/CSFX-Core/commit/be9d879dd722a4502dacad1a7cfb8f71bf22cf65))
* route presigned s3 uploads through dedicated gateway port ([e8c70fa](https://github.com/CSFX-cloud/CSFX-Core/commit/e8c70fa4acdfcf1e611a434f6f09b21f6a27689f))
* route qemu vm status inspection to qemu runtime instead of firecracker ([a9206f6](https://github.com/CSFX-cloud/CSFX-Core/commit/a9206f6ee44f1d027e157f7bccb7326e93b412c4))
* run jailer as agent uid/gid so socket permissions match ([83fe2e7](https://github.com/CSFX-cloud/CSFX-Core/commit/83fe2e73b7960b9fd2b94301db9210d7364cff00))
* run qemu as agent user, fix vnc display, add reconcile after restart ([61bbabd](https://github.com/CSFX-cloud/CSFX-Core/commit/61bbabd76a42e85fb4a91c74d19117b2164150af))
* scope object storage data proxy under /s3data prefix ([42dcd08](https://github.com/CSFX-cloud/CSFX-Core/commit/42dcd08440614e102cab47f34669db345c3c3736))
* seed vsock cid counter from host state and clean up jailer on boot failure ([82cea21](https://github.com/CSFX-cloud/CSFX-Core/commit/82cea2181556b5b40c486ae3405b7b14e085465a))
* separate resource group cidr from mgmt tunnel range and prevent vsock cid reuse# ([ad9ff6e](https://github.com/CSFX-cloud/CSFX-Core/commit/ad9ff6eaa10675445b7b46beb7ae8e3e6ffa89ba))
* set tap device owner and named systemd unit for privileged chroot cleanup ([43949f9](https://github.com/CSFX-cloud/CSFX-Core/commit/43949f9e06e6cbde3ad9298ed966fbf6319f4e05))
* set TLS_ENABLED=false in docker-compose and accept self-signed certs in agent ([a14e75d](https://github.com/CSFX-cloud/CSFX-Core/commit/a14e75defb4a644c9aa1fa7463b5e8b453de0487))
* set working directory to /tmp for nixos-rebuild to avoid symlink permission error ([74b7bd1](https://github.com/CSFX-cloud/CSFX-Core/commit/74b7bd1882fab31bea909a98e6c2c139e648e9f0))
* shorten jailer chroot path to fit unix socket SUN_LEN limit ([fb1b2da](https://github.com/CSFX-cloud/CSFX-Core/commit/fb1b2da24e1a0c55aa2bced57aa98a9f3d420b51))
* shorten tap device name to fit linux 15 character ifname limit ([4c6b998](https://github.com/CSFX-cloud/CSFX-Core/commit/4c6b998a7f9062f1b1e4bd748807dbf04b712fa2))
* show vm workloads correctly instead of container in resource group ui ([1c4384d](https://github.com/CSFX-cloud/CSFX-Core/commit/1c4384d0181ea4c34f5967632c23cd071fcfccbb))
* sign gateway tls cert from internal ca and persist container state across reboot ([1a6180b](https://github.com/CSFX-cloud/CSFX-Core/commit/1a6180b5493649902dde54b5a9f395a10bdb31ab))
* single-node patroni+etcd, remove haproxy ([45952da](https://github.com/CSFX-cloud/CSFX-Core/commit/45952da7810a0b980e334d4eaf8f6493d994fb8e))
* small change for testing updater ([1f5509f](https://github.com/CSFX-cloud/CSFX-Core/commit/1f5509fe0c8dada44290e98502321e69ea97866c))
* spawn jailer as root via systemd-run and fix chroot base dir nesting ([90329e4](https://github.com/CSFX-cloud/CSFX-Core/commit/90329e489f13781b4556fd062e9a0d53e5d90aa4))
* ssh error for dev ([a6a4fc9](https://github.com/CSFX-cloud/CSFX-Core/commit/a6a4fc9bcfceca80bb3a10dae50ff3cd7b389e5e))
* stage kernel, rootfs, and volume devices inside firecracker jailer chroot ([270ff86](https://github.com/CSFX-cloud/CSFX-Core/commit/270ff8691f6e24c894a728d5fa1482049923e669))
* stop forwarding raw agent status codes as gateway response codes ([f286562](https://github.com/CSFX-cloud/CSFX-Core/commit/f286562d172669444535209a2b7665edacc1171f))
* stop qemu unit on boot failure and stop without handle ([d05adad](https://github.com/CSFX-cloud/CSFX-Core/commit/d05adad4fe05fc44bcd262f038858bdbce4f1db6))
* stop qemu unit on boot failure and stop without handle ([840ec50](https://github.com/CSFX-cloud/CSFX-Core/commit/840ec50d0b70bc84d019f84f673c83ea04507a88))
* stream s3 proxy request body instead of buffering in memory ([406badb](https://github.com/CSFX-cloud/CSFX-Core/commit/406badb9f803c42e1c32d1ae419301948e8afcdb))
* stream s3 proxy request body instead of buffering in memory ([d8c1b43](https://github.com/CSFX-cloud/CSFX-Core/commit/d8c1b433945ee826a62e21daf67af7358d68a2b2))
* strip digest newlines to prevent invalid image reference in manifest ([fdd4d7e](https://github.com/CSFX-cloud/CSFX-Core/commit/fdd4d7e4b4cba4b5acfb861f2071c050fa627a09))
* strip trailing semicolon from parsed nix version strings ([346e5f4](https://github.com/CSFX-cloud/CSFX-Core/commit/346e5f4ed78e90f04c5132d94d24b8dda5cc88ac))
* swagger ui ([54113f4](https://github.com/CSFX-cloud/CSFX-Core/commit/54113f4d6ef39718123f1f0978a18b527d0d2628))
* swagger ui ([033f790](https://github.com/CSFX-cloud/CSFX-Core/commit/033f790c400efc7981de8c150f88136139c6ba74))
* switch rustls provider from aws-lc-rs to ring for musl compat ([7c6c62b](https://github.com/CSFX-cloud/CSFX-Core/commit/7c6c62b43ec605e0eefb18dbdc528cdc79bef2e3))
* sync missing permissions to existing Admin role on startup ([bf050ae](https://github.com/CSFX-cloud/CSFX-Core/commit/bf050aeaa133b33286114cf5b90694016c28f8ef))
* sync workload status on stop/restart and auto-detect gateway TLS SAN ([7be20b8](https://github.com/CSFX-cloud/CSFX-Core/commit/7be20b8740b5c0cb13772341fe39cfed3b1942ff))
* token route for dev ([97d0776](https://github.com/CSFX-cloud/CSFX-Core/commit/97d07769537cf0a119a860a2a8cea646970cf9e8))
* unblock agent log streaming and auto-allocate node ports ([56a09fc](https://github.com/CSFX-cloud/CSFX-Core/commit/56a09fc5a9b9bb366c0191faf306e495ffd433bf))
* unblock heartbeat loop from metrics fifo read and guest log reconnects ([ed4cb37](https://github.com/CSFX-cloud/CSFX-Core/commit/ed4cb37a8cf3e5b627586f8ecfa67743474a444d))
* unblock manual updates, auto-allocate agent tunnel IP, fix ceph rbd config ([a4ef5f8](https://github.com/CSFX-cloud/CSFX-Core/commit/a4ef5f87a070e2e30d48578e300bfd7ae8749c50))
* unset no_new_privileges to silence unenforced seccomp warning in guest init ([3b1ce4b](https://github.com/CSFX-cloud/CSFX-Core/commit/3b1ce4be86441379a410f17f49903878371f3964))
* update bollard, rand, sysinfo, axum FromRequestParts for dep updates ([588d22b](https://github.com/CSFX-cloud/CSFX-Core/commit/588d22b99c898b9e03b4dfbd908202bc92b3560a))
* update h2, rustls and crossbeam-epoch to patched versions ([58d92a6](https://github.com/CSFX-cloud/CSFX-Core/commit/58d92a6c491be6c2deb1f79be702c3c431eab4bb))
* update h2, rustls and crossbeam-epoch to patched versions ([46edfe3](https://github.com/CSFX-cloud/CSFX-Core/commit/46edfe3866f63c7955d23cdcb7b20a83d626f0ee))
* updater error with images and pull ([bbb8694](https://github.com/CSFX-cloud/CSFX-Core/commit/bbb8694fc2c7a3a065c5a08af073d2009b8a9fd4))
* updater flow ([3d9b26c](https://github.com/CSFX-cloud/CSFX-Core/commit/3d9b26ce73f21adad7574df48f0c5995c1a82ea8))
* updater flow with api-gateway ([1de5b80](https://github.com/CSFX-cloud/CSFX-Core/commit/1de5b806df10452027c8b9a97c3808228178c064))
* updater flow with api-gateway ([5bf29ab](https://github.com/CSFX-cloud/CSFX-Core/commit/5bf29ab4e746717538d56956c32a76aa8532d245))
* **updater:** correct GitHub refs API path and treat 304 as no-op ([e11c40e](https://github.com/CSFX-cloud/CSFX-Core/commit/e11c40e2c513a6aee2aeaf90e17129a739756009))
* **updater:** set explicit nixosConfigurations attr in flake URL to match csfx-node ([bdaa676](https://github.com/CSFX-cloud/CSFX-Core/commit/bdaa676c07542440e1c9fcd190b4419d36b8850a))
* **updater:** surface version resolution failures to etcd ([9ad7152](https://github.com/CSFX-cloud/CSFX-Core/commit/9ad7152a544172d0a3ac348249a1f8decddb2c5e))
* use docker icon for compose stack row instead of generic layers icon ([7ca06ad](https://github.com/CSFX-cloud/CSFX-Core/commit/7ca06ada868b277775db01d328bba35ae0f03280))
* use firecracker uds handshake instead of af_vsock for guest exec and log streams ([7a1d0a0](https://github.com/CSFX-cloud/CSFX-Core/commit/7a1d0a0b4440bb45890c1acca009337103469afd))
* use garage binary for healthcheck instead of missing wget ([84b394e](https://github.com/CSFX-cloud/CSFX-Core/commit/84b394e1d5b0f95d561b8d23324026eb054e2d9e))
* use jail-relative metrics path so firecracker can create it in chroot ([e651b36](https://github.com/CSFX-cloud/CSFX-Core/commit/e651b3654ffd4edb16d79fbb622460131ff3544e))
* use native byte order when packing ipv4 sockaddr for ioctl calls ([6d926cc](https://github.com/CSFX-cloud/CSFX-Core/commit/6d926cce4bf27d66cb753c5ebd0e819139c3bda7))
* use portable c_char type for interface name buffer on arm64 musl ([ea88be5](https://github.com/CSFX-cloud/CSFX-Core/commit/ea88be5e7eae0d0a1e5fb67b3769afdecc102365))
* use relative API URLs for same-origin deployment ([16495a4](https://github.com/CSFX-cloud/CSFX-Core/commit/16495a45961bd049d54d9d05c9f56b6cec36ca06))
* use valid 32-byte hex rpc secret for dev garage container ([0110aa3](https://github.com/CSFX-cloud/CSFX-Core/commit/0110aa311873d4c3649455f614894332100f921d))
* verify agent stream tls against internal ca and pull missing coredns image ([69d4910](https://github.com/CSFX-cloud/CSFX-Core/commit/69d49103bdd244a018cdce041bfbce1152323d37))
* version docker image ([8768620](https://github.com/CSFX-cloud/CSFX-Core/commit/87686205a3bc01b67b7c6897c736da65f1ec752d))
* wait for auth token before loading data on nodes, resource-groups and logs pages ([9ff3735](https://github.com/CSFX-cloud/CSFX-Core/commit/9ff3735b34d241669772152ae034715a7fabcc27))
* wait for auth token before loading resource group on mount ([b67e6f1](https://github.com/CSFX-cloud/CSFX-Core/commit/b67e6f1ea1d25758eb025d546e4870e2fafa270e))
* wait for ca file and order gateway after registry to avoid self-signed race ([b1f256f](https://github.com/CSFX-cloud/CSFX-Core/commit/b1f256f70dd9d634a1925cba2b392fa0aca52b02))
* wire TLS through dev environment for gateway, agents, and registry ([8028bae](https://github.com/CSFX-cloud/CSFX-Core/commit/8028bae86db21caa40c0a8e05e099952787977fe))
* workflow ([c71c60f](https://github.com/CSFX-cloud/CSFX-Core/commit/c71c60f945d715666e6e0ed01678c5e59b0c48f4))
* workload log streaming and cgroup metrics path ([251803f](https://github.com/CSFX-cloud/CSFX-Core/commit/251803fc1ff02d5b55d70b5aed2f6260ede3d536))


### Performance Improvements

* push assignments to agents on all scheduling paths and start workloads in parallel ([d7818b7](https://github.com/CSFX-cloud/CSFX-Core/commit/d7818b7e4c24cfdaa468ddb4e56c6a7ba1ac4247))


### Reverts

* restore --no-seccomp jailer flag, firecracker default filter kills vm on connect syscall ([e56ee71](https://github.com/CSFX-cloud/CSFX-Core/commit/e56ee71a4098f548b1cb710211a23f0e74065181))

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
