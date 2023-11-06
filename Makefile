NAME=book-searcher

PREFIX ?= /usr/local/bin
TARGET ?= debug

.PHONY: all frontend_preinstall frontend build clean
all: build

build: frontend
ifeq (${TARGET}, release)
	cargo build -p book-searcher --release
else
	cargo build -p book-searcher
endif

clean:
	cargo clean
	rm -rf release

test:
	cargo test --workspace --exclude ceramic-http-client

udeps:
	cargo +nightly udeps --all-targets

sort-deps:
	cargo sort -w