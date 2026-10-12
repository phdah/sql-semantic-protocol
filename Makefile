.PHONY: build fmt lint test doc check dbt-e2e

DBT_E2E_PROJECT := tests/fixtures/dbt_core_project
DBT_E2E_DATABASE := $(abspath target/dbt-core-e2e.duckdb)
TDG_PINNED_PROJECT := tests/fixtures/sql_tdg/dbt_core_project
TDG_PINNED_DATABASE := $(abspath target/sql-tdg-pinned-e2e.duckdb)

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
	rm -rf $(TDG_PINNED_PROJECT)/target $(TDG_PINNED_DATABASE)
	DBT_E2E_DATABASE=$(TDG_PINNED_DATABASE) dbt seed --project-dir $(TDG_PINNED_PROJECT) --profiles-dir $(TDG_PINNED_PROJECT)
	DBT_E2E_DATABASE=$(TDG_PINNED_DATABASE) dbt run --project-dir $(TDG_PINNED_PROJECT) --profiles-dir $(TDG_PINNED_PROJECT)
	DBT_E2E_DATABASE=$(TDG_PINNED_DATABASE) dbt docs generate --project-dir $(TDG_PINNED_PROJECT) --profiles-dir $(TDG_PINNED_PROJECT)
	DBT_E2E_DATABASE=$(TDG_PINNED_DATABASE) cargo test --test sql_tdg_pinned_dbt_e2e -- --ignored --nocapture
