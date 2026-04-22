FROM postgres:16-alpine

RUN apk add --no-cache python3 py3-pip curl && \
    pip3 install --no-cache-dir --break-system-packages patroni[etcd3]==3.3.2 psycopg2-binary

RUN mkdir -p /etc/patroni /data && \
    addgroup -S patroni && \
    adduser -S -G patroni -h /data patroni && \
    chown -R patroni:patroni /data /etc/patroni

COPY config.yml /etc/patroni/config.yml.tpl
COPY entrypoint.sh /entrypoint.sh
COPY post-bootstrap.sh /etc/patroni/post-bootstrap.sh.tpl
RUN chmod +x /entrypoint.sh /etc/patroni/post-bootstrap.sh.tpl && \
    chown patroni:patroni /etc/patroni/config.yml.tpl /etc/patroni/post-bootstrap.sh.tpl

ENV PATRONI_POSTGRESQL_DATA_DIR=/data/pgdata \
    PATRONI_POSTGRESQL_BIN_DIR=/usr/local/bin \
    PATH="/usr/local/bin:$PATH"

USER patroni

EXPOSE 5432 8008

ENTRYPOINT ["/entrypoint.sh"]
