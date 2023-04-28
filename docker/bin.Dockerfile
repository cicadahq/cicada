ARG CICADA_VERSION=0.1.50

FROM buildpack-deps:20.04-curl AS download

ARG CICADA_VERSION
RUN set -eux; \
    curl -fsSL https://github.com/cicadahq/cicada/releases/download/v${CICADA_VERSION}/cicada-x86_64-unknown-linux-musl.tar.gz --output cicada.tar.gz; \
    tar -xzf cicada.tar.gz; \
    rm cicada.tar.gz; \
    chmod 755 cicada

FROM scratch

ARG CICADA_VERSION
ENV CICADA_VERSION=${CICADA_VERSION}

COPY --from=download /cicada /cicada
