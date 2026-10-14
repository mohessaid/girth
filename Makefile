SYS_PATH := /usr/bin:/bin:$(PATH)
EVAL_MODEL ?= claude-sonnet-5-5
EVAL_RUNS ?= 3
EVAL_MAX_USD ?= 3

.PHONY: check fmt lint test cov mutate build validate evals
check: fmt lint test cov validate
fmt: ; cargo fmt --check
lint: ; cargo clippy --all-targets -- -D warnings
test: ; cargo test -q
cov: ; cargo llvm-cov --quiet --fail-under-lines 90 --ignore-filename-regex 'main\.rs'
mutate: ; cargo mutants --no-shuffle -j 4
build: ; cargo build --release -q
validate: ; claude plugin validate .
evals: build ; PATH=$(SYS_PATH) claude plugin eval . --scaffold --trust-plugin --no-publish --ablation none --runs $(EVAL_RUNS) --threshold 0.66 --model $(EVAL_MODEL) --max-cost-usd $(EVAL_MAX_USD) --allow-tools Write Edit Bash
