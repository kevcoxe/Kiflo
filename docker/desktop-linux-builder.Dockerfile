FROM node:23-bookworm

ENV DEBIAN_FRONTEND=noninteractive
ENV CARGO_HOME=/usr/local/cargo
ENV RUSTUP_HOME=/usr/local/rustup
ENV PATH=/usr/local/cargo/bin:${PATH}

RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    curl \
    file \
    libappindicator3-dev \
    librsvg2-dev \
    libwebkit2gtk-4.1-dev \
    patchelf \
    pkg-config \
    python3 \
    python-is-python3 \
  && rm -rf /var/lib/apt/lists/*

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal \
  && rustup default stable

WORKDIR /workspace
