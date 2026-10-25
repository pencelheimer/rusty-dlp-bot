FROM lukemathwalker/cargo-chef:latest-rust-alpine AS chef
RUN apk add --no-cache musl-dev
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

RUN apk add --no-cache ffmpeg python3 py3-pip ca-certificates nodejs \
    && pip install --break-system-packages "yt-dlp[default,curl-cffi]"

ENV RUSTY_DLP_UPDATE_COMMAND='pip install --upgrade --break-system-packages "yt-dlp[default,curl-cffi]"'

RUN echo '--js-runtimes node' > /etc/yt-dlp.conf

COPY yt-dlp-musiclink-extractor.py /etc/yt-dlp/plugins/musiclink/yt_dlp_plugins/extractor/musiclink.py

WORKDIR /app
COPY --from=builder /app/target/release/rusty-dlp-bot /usr/local/bin/

CMD ["rusty-dlp-bot"]
