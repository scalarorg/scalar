FROM rust:1.73.0
#FROM rust:1.73-bullseye
ARG PROFILE=release
RUN apt update
RUN apt install -y cmake clang libclang-dev libpq5 libpq-dev libssl-dev software-properties-common
#RUN apt-get update && apt-get install -y cmake clang 
RUN apt-get install -y libprotobuf-dev libprotoc-dev protobuf-compiler
WORKDIR /scalar
ENTRYPOINT [ "sleep", "infinity"]