FROM  rustlang/rust:nightly AS builder

RUN apt-get update -y && \
    apt-get satisfy --no-install-recommends -y "\
        libclang-dev, \
        ca-certificates, \
        curl, \
        lld, \
        pkg-config, \
        libssl-dev, \
        git, \
        libsqlite3-dev \
    "

WORKDIR /workdir                       
ENV CARGO_HOME=/workdir/.cargo                  
COPY ./Cargo.toml ./Cargo.lock ./
COPY ./bins ./bins
COPY ./db ./db
RUN cargo +nightly build --release

FROM debian:bullseye-20230202-slim as reth-crawler
RUN apt-get update && apt-get install -y sqlite3 libcurl4 && apt-get clean && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

COPY --from=0 /workdir/target/release/reth-crawler /usr/bin/reth-crawler
COPY --from=0 /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
RUN chmod +x /usr/bin/reth-crawler
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
ENV SSL_CERT_DIR=/etc/ssl/certs
ENV RUST_LOG=info
EXPOSE 30303
EXPOSE 30303/udp
CMD ["/usr/bin/reth-crawler", "crawl"]
LABEL service=reth-crawler

FROM debian:bullseye-20230202-slim as reth-api-server
RUN apt-get update && apt-get install -y sqlite3 libcurl4 && apt-get clean && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

COPY --from=0 /workdir/target/release/reth-crawler-api-server /usr/bin/reth-api-server
COPY --from=0 /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
RUN chmod +x /usr/bin/reth-api-server
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
ENV SSL_CERT_DIR=/etc/ssl/certs
ENV RUST_LOG=info
EXPOSE 3030
CMD ["/usr/bin/reth-api-server", "start-api-server"]
LABEL service=reth-api-server
