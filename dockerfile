FROM rust

ADD . /app
WORKDIR /app

EXPOSE 6454/udp

RUN cargo build
CMD ["cargo", "run"]