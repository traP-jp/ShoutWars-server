FROM rust:1.98-slim-bookworm AS build

WORKDIR /app
COPY . .
# 依存だけを先に構築する層は設けない。lto が有効なので、依存が変わらなくても最終の構築はやり直しになる。
RUN cargo build --release --locked

# シェルもパッケージマネージャも持たない。設定の確認は起動時のログで足り、
# CPU とメモリは NeoShowcase の管理画面から見られる。
FROM gcr.io/distroless/cc-debian12
USER nonroot

COPY --from=build /app/target/release/shoutwars-server /usr/local/bin/

EXPOSE 7468
ENTRYPOINT ["/usr/local/bin/shoutwars-server"]
