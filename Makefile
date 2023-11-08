clean:
	cargo clean
	rm -rf release

test:
	cargo test --workspace --exclude ceramic-http-client

udeps:
	cargo +nightly udeps --all-targets

sort-deps:
	cargo sort -w