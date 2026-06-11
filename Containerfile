FROM rust:alpine AS chef
RUN apk add --no-cache musl-dev \
    && cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release

FROM alpine AS runtime

RUN apk add --no-cache ffmpeg python3 py3-pip ca-certificates \
    && pip install --break-system-packages yt-dlp

COPY --from=builder /app/target/release/rusty-dlp-bot /usr/local/bin/

CMD ["rusty-dlp-bot"]
