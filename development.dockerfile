FROM rust

ADD . /app
WORKDIR /app

EXPOSE 6454/udp

RUN cargo install cargo-watch
CMD ["cargo", "watch", "-x", "run"]