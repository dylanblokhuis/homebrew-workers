FROM rust:latest

WORKDIR /usr/src/homebrew-workers

COPY . .

RUN cargo install --path .

EXPOSE 3000

CMD ["homebrew-workers"]