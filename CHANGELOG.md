# Changelog

## [2.0.2](https://github.com/phdah/sql-semantic-protocol/compare/v2.0.1...v2.0.2) (2026-10-08)


### Bug Fixes

* **dbt:** resolve source relationships tests without attached_node ([#72](https://github.com/phdah/sql-semantic-protocol/issues/72)) ([355a992](https://github.com/phdah/sql-semantic-protocol/commit/355a99238c8e75f1e5969ef945481f3241c99c43))

## [2.0.1](https://github.com/phdah/sql-semantic-protocol/compare/v2.0.0...v2.0.1) (2026-10-08)


### Bug Fixes

* **cli:** present grouped, terminal-aware help ([#71](https://github.com/phdah/sql-semantic-protocol/issues/71)) ([be711f0](https://github.com/phdah/sql-semantic-protocol/commit/be711f0f40d2cfe97e589008b685b517bf9f192b))

## [2.0.0](https://github.com/phdah/sql-semantic-protocol/compare/v1.0.1...v2.0.0) (2026-10-08)


### ⚠ BREAKING CHANGES

* **protocol:** classify residual causes and normalize boolean predicates ([#66](https://github.com/phdah/sql-semantic-protocol/issues/66))

### Features

* **analysis:** compose CASE branch source domains ([#51](https://github.com/phdah/sql-semantic-protocol/issues/51)) ([f5f9974](https://github.com/phdah/sql-semantic-protocol/commit/f5f9974ad34ae84aba259b33df7cb87b878cce4f))
* **analysis:** retain typed comparison domains with explicit assumptions (TASK-53) ([#65](https://github.com/phdah/sql-semantic-protocol/issues/65)) ([c5e359a](https://github.com/phdah/sql-semantic-protocol/commit/c5e359a7e114a94002fd66fb21b930bc315548ab))
* **analysis:** type domain literals from source schemas ([#59](https://github.com/phdah/sql-semantic-protocol/issues/59)) ([1c8f086](https://github.com/phdah/sql-semantic-protocol/commit/1c8f0862d82bdf7f64b0059f47e8ccf8ca97c453))
* **constraints:** add canonical column constraints ([#45](https://github.com/phdah/sql-semantic-protocol/issues/45)) ([ed92419](https://github.com/phdah/sql-semantic-protocol/commit/ed92419ea2f70e514365b4cf5dda49a0a9514d5d))
* **constraints:** define NULL and accepted-value typing semantics ([#54](https://github.com/phdah/sql-semantic-protocol/issues/54)) ([2e6998e](https://github.com/phdah/sql-semantic-protocol/commit/2e6998e4ff3426b7aa393bf48279e00cace30745))
* **dbt:** analyze typed manifests without catalog.json ([#61](https://github.com/phdah/sql-semantic-protocol/issues/61)) ([6c506ab](https://github.com/phdah/sql-semantic-protocol/commit/6c506ab98c50fb33891b12acab44a5245961e7ea))
* **dbt:** fall back to manifest column types ([#46](https://github.com/phdah/sql-semantic-protocol/issues/46)) ([117cd10](https://github.com/phdah/sql-semantic-protocol/commit/117cd10b05512ba76ff185c00b7219f9d741d673))
* expose composed physical join equalities ([#58](https://github.com/phdah/sql-semantic-protocol/issues/58)) ([adcaccc](https://github.com/phdah/sql-semantic-protocol/commit/adcaccc5a1ec07450347abda472f701f6f6ee625))
* **odcs:** add v3.2 external metadata adapter ([#47](https://github.com/phdah/sql-semantic-protocol/issues/47)) ([d2b4de9](https://github.com/phdah/sql-semantic-protocol/commit/d2b4de9f3f0e029a2d3d785606c490f16b7567be))
* **protocol:** add canonical relation key constraints ([#44](https://github.com/phdah/sql-semantic-protocol/issues/44)) ([dcc8680](https://github.com/phdah/sql-semantic-protocol/commit/dcc8680d1c5bc6970cf3f96da8d80e5958cfa00b))
* **protocol:** define row-condition exactness ([#55](https://github.com/phdah/sql-semantic-protocol/issues/55)) ([e1c54dd](https://github.com/phdah/sql-semantic-protocol/commit/e1c54dd5243e076e30122a25eff2f1f10f6b9cda))
* **protocol:** emit CASE branch source domains ([#42](https://github.com/phdah/sql-semantic-protocol/issues/42)) ([4cb53db](https://github.com/phdah/sql-semantic-protocol/commit/4cb53db21c8c3bb5921cdb8e6d2111829b06ebf0))
* **schema:** validate query and constraint references against typed evidence ([#60](https://github.com/phdah/sql-semantic-protocol/issues/60)) ([4976351](https://github.com/phdah/sql-semantic-protocol/commit/49763513c380eca31206c307ccb3f71822533e18))


### Bug Fixes

* **analysis:** carry CTE and derived-table semantics ([#40](https://github.com/phdah/sql-semantic-protocol/issues/40)) ([8fe27a4](https://github.com/phdah/sql-semantic-protocol/commit/8fe27a43ac570bd1b7ba13609374f4776aec344a))
* **analysis:** normalize timestamp bounds (TASK-55) ([#67](https://github.com/phdah/sql-semantic-protocol/issues/67)) ([f86f0d6](https://github.com/phdah/sql-semantic-protocol/commit/f86f0d611c1bcb79a0b02eb1ebcd7bbb434b468d))
* **analysis:** normalize timezone-aware timestamp literal spellings (TASK-56) ([#68](https://github.com/phdah/sql-semantic-protocol/issues/68)) ([49b5b99](https://github.com/phdah/sql-semantic-protocol/commit/49b5b99a9d7edac629f04e6271d17a095ec00279))
* **analysis:** resolve local joins to physical columns ([#49](https://github.com/phdah/sql-semantic-protocol/issues/49)) ([5a34eda](https://github.com/phdah/sql-semantic-protocol/commit/5a34edab03f558d73296999db4a6ef9c28f973b9))
* **analysis:** stop mapping domains through computed columns ([#48](https://github.com/phdah/sql-semantic-protocol/issues/48)) ([4e2d4f1](https://github.com/phdah/sql-semantic-protocol/commit/4e2d4f1a706797060d6fed43251999e4c8200fb3))
* **analysis:** surface local predicate semantics ([#50](https://github.com/phdah/sql-semantic-protocol/issues/50)) ([c2d5956](https://github.com/phdah/sql-semantic-protocol/commit/c2d595686961a4111a67f8cf1e1681e5a722f3f0))
* carry exactness through local relations and composition ([#57](https://github.com/phdah/sql-semantic-protocol/issues/57)) ([7521077](https://github.com/phdah/sql-semantic-protocol/commit/7521077f7ee0da2344de8625dcfad2748c111281))
* **dbt:** report unsupported constraint metadata ([#53](https://github.com/phdah/sql-semantic-protocol/issues/53)) ([2254f18](https://github.com/phdah/sql-semantic-protocol/commit/2254f1829d77b4eac92fcc92027a31baf8d1ae1d))
* **dbt:** resolve canonical foreign key targets ([#52](https://github.com/phdah/sql-semantic-protocol/issues/52)) ([e1a7957](https://github.com/phdah/sql-semantic-protocol/commit/e1a795744dfd2fd3051c990855810047bfbbee13))
* **protocol:** classify residual causes and normalize boolean predicates ([#66](https://github.com/phdah/sql-semantic-protocol/issues/66)) ([3a4d3c6](https://github.com/phdah/sql-semantic-protocol/commit/3a4d3c6870d38eb9310af54a9533806e8098a05c))

## [1.0.1](https://github.com/phdah/sql-semantic-protocol/compare/v1.0.0...v1.0.1) (2026-10-04)


### Bug Fixes

* **ci:** identify crates.io recovery requests ([#37](https://github.com/phdah/sql-semantic-protocol/issues/37)) ([e7ac121](https://github.com/phdah/sql-semantic-protocol/commit/e7ac12153a41b3178e0894d753249bfbcf726d84))
* **ci:** recover unpublished tagged crate ([#32](https://github.com/phdah/sql-semantic-protocol/issues/32)) ([7833d86](https://github.com/phdah/sql-semantic-protocol/commit/7833d86e3971ebfce9f4f535a19febac167c8713))

## [1.0.0](https://github.com/phdah/sql-semantic-protocol/compare/v0.2.0...v1.0.0) (2026-10-04)


### Features

* **analysis:** analyze output columns and lineage ([#6](https://github.com/phdah/sql-semantic-protocol/issues/6)) ([51677fe](https://github.com/phdah/sql-semantic-protocol/commit/51677fe52291994383cc086bc9c553ce5ae5dfdc))
* **analysis:** analyze relations joins and dependencies ([#5](https://github.com/phdah/sql-semantic-protocol/issues/5)) ([61258b3](https://github.com/phdah/sql-semantic-protocol/commit/61258b39e7875a635a40dcc805d9d929936e2c6f))
* **analysis:** analyze SQL set operations ([#17](https://github.com/phdah/sql-semantic-protocol/issues/17)) ([261d502](https://github.com/phdah/sql-semantic-protocol/commit/261d50258db6d2df04ab32663576e5b0b91d95db))
* **analysis:** analyze window functions ([#18](https://github.com/phdah/sql-semantic-protocol/issues/18)) ([a258125](https://github.com/phdah/sql-semantic-protocol/commit/a25812589bc512939f3c2271e62b8b1149154055))
* **analysis:** capture ddl-produced relation identities ([#13](https://github.com/phdah/sql-semantic-protocol/issues/13)) ([af524e3](https://github.com/phdah/sql-semantic-protocol/commit/af524e36beb5c5b9faf07a1f3ddd4c5a7b96f6d6))
* **analysis:** derive column value domains ([#7](https://github.com/phdah/sql-semantic-protocol/issues/7)) ([6a409f8](https://github.com/phdah/sql-semantic-protocol/commit/6a409f8f75ae95cc70d32106c9eddeb17e1d4b88))
* **analysis:** derive output column value domains ([#21](https://github.com/phdah/sql-semantic-protocol/issues/21)) ([59b1fb2](https://github.com/phdah/sql-semantic-protocol/commit/59b1fb21e3db153422fc106e4540f0edca3f24f7))
* **analysis:** expand aggregation and grouping semantics ([#19](https://github.com/phdah/sql-semantic-protocol/issues/19)) ([baefe6d](https://github.com/phdah/sql-semantic-protocol/commit/baefe6d1f6de9b4c3d7454470f7494704b22b0ff))
* **analysis:** expand subquery and table-source semantics ([#20](https://github.com/phdah/sql-semantic-protocol/issues/20)) ([74e9167](https://github.com/phdah/sql-semantic-protocol/commit/74e9167852f040170a4a614e79b9ba29fd8455d4))
* **analysis:** link DML-produced transformations ([#25](https://github.com/phdah/sql-semantic-protocol/issues/25)) ([40acdb7](https://github.com/phdah/sql-semantic-protocol/commit/40acdb75e4804ac88bd21589c35ac6de410d9a4b))
* **analysis:** normalize expressions and predicates ([#4](https://github.com/phdah/sql-semantic-protocol/issues/4)) ([ec15f2e](https://github.com/phdah/sql-semantic-protocol/commit/ec15f2ea6d5de2afe7da0ca07f114d087cbb6339))
* **analysis:** represent unsupported semantics ([#3](https://github.com/phdah/sql-semantic-protocol/issues/3)) ([4bba3a9](https://github.com/phdah/sql-semantic-protocol/commit/4bba3a9453f302d9c7b5e2e8885668639fde422b))
* **api:** accept multiple SQL inputs ([#12](https://github.com/phdah/sql-semantic-protocol/issues/12)) ([24967f1](https://github.com/phdah/sql-semantic-protocol/commit/24967f1f5e07b383bb80f3e5a70ff149b6dc712a))
* **api:** construct canonical datatype fields ([0749650](https://github.com/phdah/sql-semantic-protocol/commit/074965053ac11ec0eebd5b9173a562ed38dd4f3d))
* **api:** establish library-first analysis API ([#2](https://github.com/phdah/sql-semantic-protocol/issues/2)) ([5efc586](https://github.com/phdah/sql-semantic-protocol/commit/5efc586e3ea392f1246e2dfc847ec63302594e75))
* **api:** expose canonical datatype model ([0b991bf](https://github.com/phdah/sql-semantic-protocol/commit/0b991bfdacfdfa9805f40165a1f7527a3421b597))
* **api:** expose complete dbt artifact adapter ([0ad829f](https://github.com/phdah/sql-semantic-protocol/commit/0ad829fc2d7fa66d8f8f02b9ea05c260b90f9058))
* **api:** expose dialect selection for consumers ([db523d9](https://github.com/phdah/sql-semantic-protocol/commit/db523d97feaa07b461fef84d78855f78674a1cc9))
* **api:** expose schema completeness ([c1b79ea](https://github.com/phdah/sql-semantic-protocol/commit/c1b79ea1eb5616c3435e67b913a2ec9b7f7b5e20))
* **bundle:** link relation dependency graph ([#14](https://github.com/phdah/sql-semantic-protocol/issues/14)) ([a15c751](https://github.com/phdah/sql-semantic-protocol/commit/a15c751bfa2aafcf1ab9acf83d25aa2b8c9a6837))
* **bundle:** preserve source schema metadata ([7d777a8](https://github.com/phdah/sql-semantic-protocol/commit/7d777a86f8a91264aaec35f8f7cc145b86b960e2))
* **bundle:** select explicit target outcomes ([#23](https://github.com/phdah/sql-semantic-protocol/issues/23)) ([4575583](https://github.com/phdah/sql-semantic-protocol/commit/4575583ff25e1fe9a4ff3182b65440c717733038))
* **bundle:** validate extended catalog-aware workflows ([#27](https://github.com/phdah/sql-semantic-protocol/issues/27)) ([291ec7a](https://github.com/phdah/sql-semantic-protocol/commit/291ec7aca76283446f55a24243aff3431123e067))
* **catalog:** add catalog-aware relation resolution ([#26](https://github.com/phdah/sql-semantic-protocol/issues/26)) ([6e7f2df](https://github.com/phdah/sql-semantic-protocol/commit/6e7f2df9c12f41245d9ec09fb2554c1b230617cf))
* **catalog:** use canonical source datatypes ([47b3689](https://github.com/phdah/sql-semantic-protocol/commit/47b368957655761009c5cb03648ecfd377bd22ae))
* **cli:** consume dbt manifest and catalog together ([8977ecf](https://github.com/phdah/sql-semantic-protocol/commit/8977ecf73a3c896ee1911280bae9930595ca319f))
* **cli:** emit deterministic protocol JSON ([#8](https://github.com/phdah/sql-semantic-protocol/issues/8)) ([e9aa556](https://github.com/phdah/sql-semantic-protocol/commit/e9aa55621f292e8479f7fe8452a7149f01a22eaa))
* compose semantics across transformation layers ([#15](https://github.com/phdah/sql-semantic-protocol/issues/15)) ([17388b5](https://github.com/phdah/sql-semantic-protocol/commit/17388b5ec176d9c7380e655fe9caff88a22a2f21))
* **dbt:** add manifest adapter ([#28](https://github.com/phdah/sql-semantic-protocol/issues/28)) ([b6e3910](https://github.com/phdah/sql-semantic-protocol/commit/b6e3910a02639e47ce61ae64c278f9cd812b10f5))
* **dbt:** combine manifest and catalog semantics ([7addf65](https://github.com/phdah/sql-semantic-protocol/commit/7addf65307f001383b73c6556d71423d8bbf7e7e))
* **dbt:** model catalog artifacts ([7a82faa](https://github.com/phdah/sql-semantic-protocol/commit/7a82faad37746443c3805e6f9b5a285ee9beaf58))
* **dbt:** parse catalog artifacts ([5e11921](https://github.com/phdah/sql-semantic-protocol/commit/5e11921f0deee66921acbba2c6209975e4693592))
* **emission:** emit typed source schemas ([3f68a50](https://github.com/phdah/sql-semantic-protocol/commit/3f68a50193eb1ad1d1817bb9c80bd56827e4d3bc))
* **emission:** serialize canonical source datatypes ([d00faae](https://github.com/phdah/sql-semantic-protocol/commit/d00faae610ac931738c8b6d97c86a8d5354004c4))
* keep complete outcomes with terminal classification ([#16](https://github.com/phdah/sql-semantic-protocol/issues/16)) ([04f7645](https://github.com/phdah/sql-semantic-protocol/commit/04f7645642985b5a8ffb02ff65f6eef86d42ceeb))
* **manifest:** add declarative analysis manifests ([#24](https://github.com/phdah/sql-semantic-protocol/issues/24)) ([3367685](https://github.com/phdah/sql-semantic-protocol/commit/3367685d1a773dbd72dfa11b124ef9135071dd2f))
* **protocol:** add typed source schema metadata ([60a98ab](https://github.com/phdah/sql-semantic-protocol/commit/60a98ab9270d513b80c384bc175309483fab37de))
* **protocol:** define multi-query composition contract ([#11](https://github.com/phdah/sql-semantic-protocol/issues/11)) ([e29d2f5](https://github.com/phdah/sql-semantic-protocol/commit/e29d2f5dee65a648608cd039573e513fd8c84c96))
* **protocol:** define typed schemas and complete dbt adapter ([1fd61d2](https://github.com/phdah/sql-semantic-protocol/commit/1fd61d25ec94d61a8966c171108836a3acac642c))
* **protocol:** define v0 json contract ([#1](https://github.com/phdah/sql-semantic-protocol/issues/1)) ([526fc96](https://github.com/phdah/sql-semantic-protocol/commit/526fc9682e24eacd4a86b46d07973f70add44026))
* **protocol:** keep source scalar types extensible ([41bd250](https://github.com/phdah/sql-semantic-protocol/commit/41bd2501bca430d7c6236d0b395cf726b3d0057c))
* **protocol:** normalize dialect datatypes ([a4d643d](https://github.com/phdah/sql-semantic-protocol/commit/a4d643d7e0ffb370c223a481ef2a0a719a2d8775))
* **schema:** define canonical source datatypes ([2ff3727](https://github.com/phdah/sql-semantic-protocol/commit/2ff3727663fe00a739aeba1dc2d0ab86608f1815))
* **schema:** define source schema metadata ([c2158f3](https://github.com/phdah/sql-semantic-protocol/commit/c2158f3d4ae6c60a8f247fe425414c38d262f5f0))
* **schema:** represent partial relation schemas ([11cf16c](https://github.com/phdah/sql-semantic-protocol/commit/11cf16c22c33aa524ce2c9e995649b743246b5d8))


### Bug Fixes

* **api:** satisfy dialect lookup lint ([51811f8](https://github.com/phdah/sql-semantic-protocol/commit/51811f8b03d9ec0d841d3c5e023a3e2536bf25bd))
* **cli:** map dbt artifact parsing errors ([a0af04b](https://github.com/phdah/sql-semantic-protocol/commit/a0af04b292d8e51ef1573ada03b5a95fe04e7a01))
* **dbt:** deduplicate shared physical catalog relations ([c7e4454](https://github.com/phdah/sql-semantic-protocol/commit/c7e44540fa6f6553673ad0a4aa3cf42868bbdf39))
* **test:** use owned structured field names ([33d84e9](https://github.com/phdah/sql-semantic-protocol/commit/33d84e945aa3d52ff99c993ffb3b48fdce341e19))

## Changelog

Release Please maintains this file from Conventional Commits.
