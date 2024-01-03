FROM rust:1.73.0
#FROM rust:1.73-bullseye
ARG PROFILE=release
ARG UID
ARG GID
ARG USER
RUN apt update
RUN apt install -y cmake clang libclang-dev libpq5 libpq-dev libssl-dev software-properties-common
RUN apt-get install -y libprotobuf-dev libprotoc-dev protobuf-compiler
RUN apt install -y sudo && \
    addgroup --gid $GID $USER && \
    adduser --uid $UID --gid $GID --disabled-password --gecos "" $USER && \
    echo "$USER ALL=(ALL) NOPASSWD: ALL" >> /etc/sudoers

# Set the non-root user as the default user
USER $USER
WORKDIR /scalar
ENTRYPOINT [ "sleep", "infinity"]