# Changelog

All notable changes to CSFX-Core will be documented in this file.

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
