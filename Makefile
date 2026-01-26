run:
	cargo run

build:
	cargo build --release

docker-build:
	docker build -t deductible-tracker .

test:
	cargo test

fmt:
	cargo fmt