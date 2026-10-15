SYS_PATH := /usr/bin:/bin:$(PATH)
EVAL_MODEL ?= claude-sonnet-5-5
EVAL_RUNS ?= 3
EVAL_MAX_USD ?= 3

.PHONY: check fmt lint test cov mutate build dist validate evals demo
DIST := target/plugin-dist
check: fmt lint test cov validate
fmt: ; cargo fmt --check
lint: ; cargo clippy --all-targets -- -D warnings
test: ; cargo test -q
cov: ; cargo llvm-cov --quiet --fail-under-lines 90 --ignore-filename-regex 'main\.rs'
mutate: ; cargo mutants --no-shuffle -j 4
build: ; cargo build --release -q
validate: ; claude plugin validate .
dist: build ; rm -rf $(DIST) && mkdir -p $(DIST) && cp -R .claude-plugin hooks skills evals $(DIST)/ && rm -rf $(DIST)/evals/results && mkdir -p $(DIST)/skills/girth/bin && cp target/release/girth $(DIST)/skills/girth/bin/girth
evals: dist ; cd $(DIST) && PATH=$(SYS_PATH) claude plugin eval . --scaffold --trust-plugin --no-publish --ablation none --runs $(EVAL_RUNS) --threshold 0.66 --model $(EVAL_MODEL) --max-cost-usd $(EVAL_MAX_USD) --allow-tools Write Edit Bash
# README demo: real girth commands on public skills (no model calls). Needs asciinema 3 and agg.
demo: build ; asciinema rec --overwrite --headless --window-size 110x30 -c "sh examples/demo/demo.sh" target/demo.cast && agg --font-size 16 --idle-time-limit 2 target/demo.cast assets/demo.gif
