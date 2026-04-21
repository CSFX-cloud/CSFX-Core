#!/bin/sh
set -eu

mkdir -p /etc/patroni

sed \
  -e "s|\${PATRONI_NAME}|${PATRONI_NAME}|g" \
  -e "s|\${PATRONI_RESTAPI_CONNECT_ADDRESS}|${PATRONI_RESTAPI_CONNECT_ADDRESS}|g" \
  -e "s|\${PATRONI_ETCD3_HOSTS}|${PATRONI_ETCD3_HOSTS}|g" \
  -e "s|\${PATRONI_POSTGRESQL_CONNECT_ADDRESS}|${PATRONI_POSTGRESQL_CONNECT_ADDRESS}|g" \
  -e "s|\${PATRONI_REPLICATION_PASSWORD}|${PATRONI_REPLICATION_PASSWORD}|g" \
  -e "s|\${PATRONI_SUPERUSER_PASSWORD}|${PATRONI_SUPERUSER_PASSWORD}|g" \
  -e "s|\${PATRONI_APP_PASSWORD}|${PATRONI_APP_PASSWORD}|g" \
  /etc/patroni/config.yml.tpl > /etc/patroni/config.yml

exec patroni /etc/patroni/config.yml
