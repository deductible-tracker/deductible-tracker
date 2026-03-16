run:
	cargo run

run-dev: tailwind-build
	RUST_ENV=development cargo run

build: tailwind-build
	cargo build --release

docker-build: tailwind-build
	docker build -t deductible-tracker .

test:
	cargo test

fmt:
	cargo fmt

tailwind-build:
	source "$$HOME/.nvm/nvm.sh" && nvm use 24.13.1 && npm run tailwind:build

tailwind-watch:
	source "$$HOME/.nvm/nvm.sh" && nvm use 24.13.1 && npm run tailwind:watch