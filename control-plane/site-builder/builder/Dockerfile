FROM gcr.io/kaniko-project/executor:v1.23.2 AS kaniko

FROM node:22-bookworm

ARG HUGO_VERSION=0.139.0

RUN apt-get update \
    && apt-get install -y --no-install-recommends git curl jq ca-certificates ruby-full build-essential \
    && gem install bundler --no-document \
    && npm install -g bun \
    && corepack enable \
    && curl -fsSL "https://github.com/gohugoio/hugo/releases/download/v${HUGO_VERSION}/hugo_extended_${HUGO_VERSION}_linux-amd64.tar.gz" \
        | tar -xz -C /usr/local/bin hugo \
    && rm -rf /var/lib/apt/lists/*

COPY --from=kaniko /kaniko/executor /usr/local/bin/kaniko-executor
COPY entrypoint.sh /usr/local/bin/csfx-site-build
RUN chmod +x /usr/local/bin/csfx-site-build

WORKDIR /workspace
ENTRYPOINT ["/usr/local/bin/csfx-site-build"]
