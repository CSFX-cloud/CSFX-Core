FROM postgres:16-alpine

RUN apk add --no-cache python3 py3-pip curl && \
    pip3 install --no-cache-dir --break-system-packages patroni[etcd3]==3.3.2 psycopg2-binary

ENV PATRONI_POSTGRESQL_DATA_DIR=/data/pgdata \
    PATRONI_POSTGRESQL_BIN_DIR=/usr/local/bin \
    PATH="/usr/local/bin:$PATH"

EXPOSE 5432 8008

CMD ["patroni", "/etc/patroni/config.yml"]
