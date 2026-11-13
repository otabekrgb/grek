FROM rust:1-slim AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=build /app/target/release/grek ./grek
COPY assets ./assets
COPY voice/lines ./voice/lines
ENV GREK_HOSTED=1
EXPOSE 8080
CMD ./grek serve "${PORT:-8080}" 0.0.0.0
