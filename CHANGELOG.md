# Changelog

## [1.0.0](https://github.com/phdah/sql-semantic-protocol/compare/v0.2.0...v1.0.0) (2026-10-03)


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
* **api:** establish library-first analysis API ([#2](https://github.com/phdah/sql-semantic-protocol/issues/2)) ([5efc586](https://github.com/phdah/sql-semantic-protocol/commit/5efc586e3ea392f1246e2dfc847ec63302594e75))
* **bundle:** link relation dependency graph ([#14](https://github.com/phdah/sql-semantic-protocol/issues/14)) ([a15c751](https://github.com/phdah/sql-semantic-protocol/commit/a15c751bfa2aafcf1ab9acf83d25aa2b8c9a6837))
* **bundle:** select explicit target outcomes ([#23](https://github.com/phdah/sql-semantic-protocol/issues/23)) ([4575583](https://github.com/phdah/sql-semantic-protocol/commit/4575583ff25e1fe9a4ff3182b65440c717733038))
* **bundle:** validate extended catalog-aware workflows ([#27](https://github.com/phdah/sql-semantic-protocol/issues/27)) ([291ec7a](https://github.com/phdah/sql-semantic-protocol/commit/291ec7aca76283446f55a24243aff3431123e067))
* **catalog:** add catalog-aware relation resolution ([#26](https://github.com/phdah/sql-semantic-protocol/issues/26)) ([6e7f2df](https://github.com/phdah/sql-semantic-protocol/commit/6e7f2df9c12f41245d9ec09fb2554c1b230617cf))
* **cli:** emit deterministic protocol JSON ([#8](https://github.com/phdah/sql-semantic-protocol/issues/8)) ([e9aa556](https://github.com/phdah/sql-semantic-protocol/commit/e9aa55621f292e8479f7fe8452a7149f01a22eaa))
* compose semantics across transformation layers ([#15](https://github.com/phdah/sql-semantic-protocol/issues/15)) ([17388b5](https://github.com/phdah/sql-semantic-protocol/commit/17388b5ec176d9c7380e655fe9caff88a22a2f21))
* **dbt:** add manifest adapter ([#28](https://github.com/phdah/sql-semantic-protocol/issues/28)) ([b6e3910](https://github.com/phdah/sql-semantic-protocol/commit/b6e3910a02639e47ce61ae64c278f9cd812b10f5))
* keep complete outcomes with terminal classification ([#16](https://github.com/phdah/sql-semantic-protocol/issues/16)) ([04f7645](https://github.com/phdah/sql-semantic-protocol/commit/04f7645642985b5a8ffb02ff65f6eef86d42ceeb))
* **manifest:** add declarative analysis manifests ([#24](https://github.com/phdah/sql-semantic-protocol/issues/24)) ([3367685](https://github.com/phdah/sql-semantic-protocol/commit/3367685d1a773dbd72dfa11b124ef9135071dd2f))
* **protocol:** define multi-query composition contract ([#11](https://github.com/phdah/sql-semantic-protocol/issues/11)) ([e29d2f5](https://github.com/phdah/sql-semantic-protocol/commit/e29d2f5dee65a648608cd039573e513fd8c84c96))
* **protocol:** define v0 json contract ([#1](https://github.com/phdah/sql-semantic-protocol/issues/1)) ([526fc96](https://github.com/phdah/sql-semantic-protocol/commit/526fc9682e24eacd4a86b46d07973f70add44026))

## Changelog

Release Please maintains this file from Conventional Commits.
