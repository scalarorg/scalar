FROM rust:1.73.0
ARG PROFILE=release
RUN apt update
RUN apt install -y cmake clang libclang-dev libpq5 libpq-dev libssl-dev software-properties-common
RUN apt install -y libprotobuf-dev libprotoc-dev protobuf-compiler
WORKDIR /scalar
ENTRYPOINT [ "sleep", "infinity"]