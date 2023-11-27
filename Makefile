clean:
	cargo clean
	rm -rf release

test:
	cargo test --workspace

doc:
	cargo doc --workspace --no-deps --document-private-items
	cd target/doc && tree -H '.' -T 'Dataverse Crates' -i -d -L 1 --noreport -P '*/index.html' -I . -I src -I implementors -I static.files --charset utf-8 | sed -e '/<hr>/,+7d' > index.html

udeps:
	cargo +nightly udeps --all-targets

sort-deps:
	cargo sort -w