run: githooks
	SKIP_WASM_BUILD= cargo run -- --dev -lruntime=debug

toolchain:
	./scripts/init.sh

build-full: githooks
	cargo build

check: githooks
	SKIP_WASM_BUILD= cargo check

check-tests: githooks
	SKIP_WASM_BUILD= cargo check --tests --all

check-debug:
	RUSTFLAGS="-Z external-macro-backtrace" SKIP_WASM_BUILD= cargo +nightly check

test: githooks
	SKIP_WASM_BUILD= cargo test --all

build: githooks
	SKIP_WASM_BUILD= cargo build

purge: target/debug/shadows
	target/debug/shadows purge-chain --dev -y

restart: purge run

target/debug/shadows: build

GITHOOKS_SRC = $(wildcard githooks/*)
GITHOOKS_DEST = $(patsubst githooks/%, .git/hooks/%, $(GITHOOKS_SRC))

.git/hooks:
	mkdir .git/hooks

.git/hooks/%: githooks/%
	cp $^ $@

githooks: .git/hooks $(GITHOOKS_DEST)

init: toolchain submodule build-full

submodule:
	git submodule update --init --recursive

update-orml:
	cd orml && git checkout master && git pull
	git add orml

update: update-orml
	cargo update
	make check

build-wasm-shadows:
	./scripts/build-only-wasm.sh shadows-runtime