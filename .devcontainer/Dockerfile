FROM mcr.microsoft.com/devcontainers/base:ubuntu-24.04

RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get install -y --no-install-recommends \
        ca-certificates ccache cmake device-tree-compiler dfu-util file \
        g++-multilib gcc-multilib git gperf libclang-dev libmagic1 libsdl2-dev make \
        ninja-build python3-dev python3-pip python3-tk python3-venv wget xz-utils \
    && rm -rf /var/lib/apt/lists/*