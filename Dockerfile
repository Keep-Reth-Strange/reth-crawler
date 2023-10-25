FROM  rustlang/rust:nightly AS builder

RUN apt-get update -y && \
    apt-get satisfy --no-install-recommends -y "\
        libclang-dev, \
        ca-certificates (>= 20210119), \
        curl (>= 7.74), curl (<< 7.75), \
        lld (>= 1:11.0), lld (<< 1:11.1), \
        pkg-config (>= 0.29), pkg-config (<< 0.30), \
        libssl-dev (>= 1.1), libssl-dev (<< 1.2), \
        git (>= 1:2.30), git (<< 1:2.31) \
    "

WORKDIR /workdir                       
ENV CARGO_HOME=/workdir/.cargo                  
COPY ./Cargo.toml ./Cargo.lock ./
COPY ./bins ./bins
COPY ./db ./db
RUN cargo +nightly build --release

FROM debian:bullseye-20230202-slim
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install --yes \
    libcurl4 \
    && apt-get clean && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

COPY --from=0 /workdir/target/release/reth-crawler /usr/bin/reth-crawler
COPY --from=0 /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
RUN ls /usr/bin
RUN chmod +x /usr/bin/reth-crawler
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
ENV SSL_CERT_DIR=/etc/ssl/certs
ENV RUST_LOG=info
EXPOSE 30303
EXPOSE 30303/udp
ENTRYPOINT ["/usr/bin/reth-crawler", "crawl"]
