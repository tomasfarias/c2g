FROM rust:1.50 as builder

WORKDIR /build

COPY . .

RUN cargo build --release

FROM scratch

COPY --from=builder /build/target/release/c2g /c2g

CMD ["/c2g"]
