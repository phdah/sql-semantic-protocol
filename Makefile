.PHONY: build fmt lint test doc check dbt-e2e

DBT_E2E_PROJECT := tests/fixtures/dbt_core_project
DBT_E2E_DATABASE := $(abspath target/dbt-core-e2e.duckdb)
CONFORMANCE_PROJECT := tests/fixtures/conformance/v1/dbt_core_project
CONFORMANCE_DATABASE := $(abspath target/protocol-conformance-e2e.duckdb)

build:
	cargo build

fmt:
	cargo fmt --check

lint:
	cargo clippy --all-targets --all-features -- -D warnings

test:
	cargo test

doc:
	cargo doc --no-deps

check: fmt lint test doc

dbt-e2e:
	rm -rf $(DBT_E2E_PROJECT)/target $(DBT_E2E_DATABASE)
	mkdir -p target
	DBT_E2E_DATABASE=$(DBT_E2E_DATABASE) dbt seed --project-dir $(DBT_E2E_PROJECT) --profiles-dir $(DBT_E2E_PROJECT)
	DBT_E2E_DATABASE=$(DBT_E2E_DATABASE) dbt run --project-dir $(DBT_E2E_PROJECT) --profiles-dir $(DBT_E2E_PROJECT)
	DBT_E2E_DATABASE=$(DBT_E2E_DATABASE) dbt run --project-dir $(DBT_E2E_PROJECT) --profiles-dir $(DBT_E2E_PROJECT)
	DBT_E2E_DATABASE=$(DBT_E2E_DATABASE) dbt docs generate --project-dir $(DBT_E2E_PROJECT) --profiles-dir $(DBT_E2E_PROJECT)
	cargo test --test dbt_core_e2e -- --ignored --nocapture
	DBT_E2E_DATABASE=$(DBT_E2E_DATABASE) cargo test --test task_189_dbt_core_e2e -- --ignored --nocapture
	rm -rf $(CONFORMANCE_PROJECT)/target $(CONFORMANCE_DATABASE)
	DBT_E2E_DATABASE=$(CONFORMANCE_DATABASE) dbt seed --project-dir $(CONFORMANCE_PROJECT) --profiles-dir $(CONFORMANCE_PROJECT)
	DBT_E2E_DATABASE=$(CONFORMANCE_DATABASE) dbt run --project-dir $(CONFORMANCE_PROJECT) --profiles-dir $(CONFORMANCE_PROJECT)
	DBT_E2E_DATABASE=$(CONFORMANCE_DATABASE) dbt docs generate --project-dir $(CONFORMANCE_PROJECT) --profiles-dir $(CONFORMANCE_PROJECT)
	DBT_E2E_DATABASE=$(CONFORMANCE_DATABASE) cargo test --test protocol_conformance_dbt_e2e -- --ignored --nocapture
