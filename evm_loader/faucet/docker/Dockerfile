FROM ubuntu:21.04

MAINTAINER NeonLabs Maintainers <maintainers@neonlabs.org>

USER root
WORKDIR /root
ENV PATH="/root:${PATH}"

RUN apt-get update && \
    apt-get -y install libssl-dev && \
    rm -rf /var/lib/apt/lists/* /var/lib/apt/cache/*

COPY tmp/faucet .
COPY tmp/faucet.toml .
COPY entrypoint.sh .

RUN chmod +x /root/faucet
RUN chmod +x /root/entrypoint.sh

ENTRYPOINT ["/root/entrypoint.sh"]
