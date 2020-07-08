FROM rust:1.44-stretch as builder

RUN apt-get update \
    && apt-get install -y pkg-config \
    && apt-get install -y libjq1 libjq-dev libonig4 libonig-dev \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*
ENV JQ_LIB_DIR=/usr/lib/x86_64-linux-gnu
RUN USER=root cargo new --lib bragi_elasticsearch_probe
WORKDIR ./bragi_elasticsearch_probe
COPY ./Cargo.toml ./Cargo.toml
RUN cargo build --release --lib
RUN rm src/*.rs

ADD . ./

RUN rm ./target/release/deps/bragi_elasticsearch_probe*
RUN cargo build --release

FROM debian:stretch-slim
ARG APP=/usr/src/app

RUN apt-get update \
    && apt-get install -y ca-certificates tzdata \
    && apt-get install -y pkg-config \
    && apt-get install -y libjq1 libjq-dev libonig4 libonig-dev \
    && rm -rf /var/lib/apt/lists/*

EXPOSE 8080

ENV TZ=Etc/UTC \
    APP_USER=appuser

RUN groupadd $APP_USER \
    && useradd -g $APP_USER $APP_USER \
    && mkdir -p ${APP}

COPY --from=builder /bragi_elasticsearch_probe/target/release/server ${APP}/server
COPY --from=builder /bragi_elasticsearch_probe/env.json ${APP}/env.json

RUN chown -R $APP_USER:$APP_USER ${APP}

USER $APP_USER
WORKDIR ${APP}

CMD ["./server", "-h", "0.0.0.0", "-p", "8080"]
