.PHONY: setup setup-contracts contracts rust app demo

CARGO ?= cargo

setup: setup-contracts
	npm ci --prefix e2e
	npm ci --prefix apps/web
	$(CARGO) fetch --locked

setup-contracts:
	if [ ! -d contracts/lib/forge-std ]; then cd contracts && forge install foundry-rs/forge-std@77041d2ce690e692d6e03cc812b57d1ddaa4d505 --no-git; fi
	if [ ! -d contracts/lib/openzeppelin-contracts ]; then cd contracts && forge install OpenZeppelin/openzeppelin-contracts@c64a1edb67b6e3f4a15cca8909c9482ad33a02b0 --no-git; fi

contracts: setup-contracts
	cd contracts && forge fmt --check && forge build && forge test -vv && forge snapshot --check --fuzz-seed 0xc0ffee

rust:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --workspace --all-targets --locked -- -D warnings
	$(CARGO) test --workspace --locked

app:
	npm ci --prefix apps/web
	npm run build --prefix apps/web

demo: setup
	forge build --root contracts
	CARGO=$(CARGO) ./scripts/demo.sh
