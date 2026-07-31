# mtl-dev: the environment that builds and runs the Metal toolchain
# (Sylvain Huet's mtl compiler and simulator, 32-bit C on amd64).
# garenne/build.sh builds this image on first use:
#   docker build --platform linux/amd64 -t mtl-dev vendor/metal
FROM --platform=linux/amd64 debian:bookworm-slim
RUN apt-get update -qq && apt-get install -y -qq \
    gcc-multilib g++-multilib make perl python3 xxd curl \
    && rm -rf /var/lib/apt/lists/*
