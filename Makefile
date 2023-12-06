# Install all dependencies
install:
	# update all packets	
	sudo apt update -y
	sudo apt upgrade -y
	
	# install Rust
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
	source "$HOME/.cargo/env" 

	# install dependencies
	sudo apt install build-essential
	sudo apt install openssl -y
	sudo apt-get install -y libclang-dev
	sudo apt install pkg-config
	sudo apt-get install libssl-dev
	sudo apt-get install sqlite3 libsqlite3-dev

	# install aws cli
	curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip"
	unzip awscliv2.zip
	sudo ./aws/install

# Run the crawler with a string argument for the ws RPC
run:
	cargo build --release -p reth-crawler
	cd target/release
	./reth-crawler crawl --eth-rpc-url $(ARG)