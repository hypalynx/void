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

gemini:
	VOID_API_KEY=$${GEMINI_API_KEY} cargo run -- \
		--host generativelanguage.googleapis.com \
		--port 443 \
		--model gemini-3-flash-preview \
		--path-prefix /v1beta/openai

install:
	cargo install --path .
