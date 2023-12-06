# Install all dependencies
install:
	# update all packets	
	sudo apt update -y
	sudo apt upgrade -y
	
	# install Rust
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

	# install dependencies
	sudo apt install build-essential -y
	sudo apt install openssl -y
	sudo apt-get install -y libclang-dev
	sudo apt install pkg-config -y
	sudo apt-get install libssl-dev -y
	sudo apt-get install sqlite3 libsqlite3-dev -y

	# install aws cli
	curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip"
	sudo apt install unzip -y
	unzip awscliv2.zip
	sudo ./aws/install

# Run the crawler with a string argument for the ws RPC
run:
	ulimit -n 25000
	source "$HOME/.cargo/env"
	cargo build --release -p reth-crawler
	cd target/release
	RUST_LOG=INFO ./reth-crawler crawl --eth-rpc-url $(ARG)
