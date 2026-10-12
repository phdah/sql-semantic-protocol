# Demo: sql-tdg generates one source dataset for the whole dbt project, dbt runs on it,
# and DuckDB shows the results.
# Run `make clean` afterwards: `load` overwrites tracked seeds, and leftover artifacts
# are copied into the dbt e2e test project.
SQL_TDG  ?= sql-tdg
DB_FILE  := $(CURDIR)/fixture.duckdb
MATCHING ?= 100
REJECTED ?= 10
SEED     ?= 42
OUT      := generated
LOG      := generate.log

export DBT_E2E_DATABASE := $(DB_FILE)
export DBT_PROFILES_DIR := $(CURDIR)

# Prints the row count of every table and view in one schema.
define count_rows
	@for t in $$(duckdb -noheader -list $(DB_FILE) \
	  "select table_name from information_schema.tables where table_schema = '$(1)' order by 1"); do \
	  printf '    %-44s %s\n' "$(1).$$t" \
	    "$$(duckdb -noheader -list $(DB_FILE) "select count(*) from \"$(1)\".\"$$t\"")"; \
	done
endef

.PHONY: help all bootstrap docs generate load verify dbt-run results dbt-test show-results clean

help:
	@echo "Targets:"
	@echo "  help          Show this help (default)"
	@echo "  all           Run bootstrap, generate, load, verify, dbt-run, results"
	@echo "  bootstrap     dbt seed/run/docs generate on the original seeds (builds catalog.json)"
	@echo "  docs          Bootstrap, then serve the dbt docs (Ctrl-C to stop)"
	@echo "  generate      Generate CSV source data for all dbt models with sql-tdg into $(OUT)/"
	@echo "  load          Copy generated CSVs over the dbt seeds and run dbt seed"
	@echo "  verify        Show source table row counts (target: MATCHING + REJECTED each)"
	@echo "  dbt-run       Run all dbt models"
	@echo "  results       Show dbt model row counts (target: MATCHING per row-preserving model)"
	@echo "  dbt-test      Run dbt tests"
	@echo "  show-results  Open $(notdir $(DB_FILE)) in the DuckDB UI"
	@echo "  clean         Remove demo artifacts and restore the original seeds"
	@echo ""
	@echo "Variables (current value):"
	@echo "  SQL_TDG=$(SQL_TDG) MATCHING=$(MATCHING) REJECTED=$(REJECTED) SEED=$(SEED)"

all: bootstrap generate load verify dbt-run results

# Builds the warehouse from the hand-written seeds so catalog.json has source types.
bootstrap:
	@echo "==> Bootstrapping dbt artifacts..."
	dbt seed --full-refresh
	dbt run --full-refresh
	dbt docs generate

docs: bootstrap
	@echo "==> Serving dbt docs (Ctrl-C to stop)..."
	dbt docs serve

# dbt seed, test and parse rewrite manifest.json without compiled SQL, so compile first.
generate:
	@echo "==> Generating test data for all dbt models with sql-tdg..."
	@rm -rf $(OUT) $(LOG)
	dbt compile --quiet
	$(SQL_TDG) generate --dbt-project . \
	  --matching $(MATCHING) --rejected $(REJECTED) \
	  --assume-comparison binary_collation --assume-comparison no_char_padding \
	  --assume-comparison no_nan --assume-comparison signed_zero_equivalent \
	  --assume-comparison session_time_zone \
	  --seed $(SEED) --format csv --output $(OUT) > $(LOG)
	@cat $(LOG)

# Copies each generated relation over its dbt seed, using sql-tdg's relation=/path= output.
load:
	@echo "==> Loading generated data as dbt seeds..."
	@grep '^relation=' $(LOG) | while read -r line; do \
	  rel=$${line#relation=}; rel=$${rel%% path=*}; path=$${line##* path=}; \
	  leaf=$$(printf '%s' "$${rel##*.}" | tr -d '"'); \
	  echo "    $$path -> seeds/$$leaf.csv"; \
	  cp "$$path" "seeds/$$leaf.csv"; \
	done
	dbt seed --full-refresh

verify:
	@echo "==> Source tables (target: $(MATCHING) matching + $(REJECTED) rejected rows each)"
	$(call count_rows,raw)

dbt-run:
	@echo "==> Running dbt models..."
	dbt run --full-refresh

results:
	@echo "==> dbt models (target: $(MATCHING) rows per row-preserving model)"
	$(call count_rows,raw_analytics)

dbt-test:
	@echo "==> Running dbt tests..."
	dbt test

show-results:
	@echo "==> Opening DuckDB UI..."
	duckdb -ui $(DB_FILE)

clean:
	rm -rf target logs $(OUT) $(LOG) $(DB_FILE) $(DB_FILE).wal .user.yml
	git checkout -- seeds
