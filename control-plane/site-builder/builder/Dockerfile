FROM gcr.io/kaniko-project/executor:v1.23.2 AS kaniko

FROM node:22-bookworm-slim

ARG HUGO_VERSION=0.139.0
ARG TARGETARCH=amd64

RUN apt-get update \
    && apt-get install -y --no-install-recommends git curl jq ca-certificates \
    && npm install -g bun \
    && corepack enable \
    && curl -fsSL "https://github.com/gohugoio/hugo/releases/download/v${HUGO_VERSION}/hugo_extended_${HUGO_VERSION}_linux-${TARGETARCH}.tar.gz" \
        | tar -xz -C /usr/local/bin hugo \
    && npm cache clean --force \
    && rm -rf /var/lib/apt/lists/* /root/.npm /tmp/*

COPY --from=kaniko /kaniko/executor /usr/local/bin/kaniko-executor
COPY entrypoint.sh /usr/local/bin/csfx-site-build
RUN chmod +x /usr/local/bin/csfx-site-build

WORKDIR /workspace
ENTRYPOINT ["/usr/local/bin/csfx-site-build"]
