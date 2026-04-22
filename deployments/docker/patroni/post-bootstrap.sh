#!/bin/sh
set -eu
psql -U postgres -c "CREATE ROLE csfx WITH LOGIN PASSWORD '${PATRONI_APP_PASSWORD}';"
psql -U postgres -c "CREATE DATABASE csfx OWNER csfx;"
