.PHONY: dev lint lint\:fix

dev:
	cargo run

debug:
	cargo run -- --port 8080 $(ARGS)

listen: # to capture LLM traffic and debug
	mitmproxy --mode reverse:http://127.0.0.1:7777 \
		--listen-host 127.0.0.1 \
		--listen-port 8080

lint:
	cargo fmt --check
	cargo clippy

lint\:fix:
	cargo fmt
	cargo clippy --fix --allow-dirty

kimi:
	VOID_API_KEY=$${FIREWORKS_API_KEY} cargo run -- \
		--host api.fireworks.ai \
		--port 443 \
		--model accounts/fireworks/models/kimi-k2p5 \
		--path-prefix /inference

install:
	cargo install --path .
