#!/bin/sh
set -eu
psql -U postgres -c "CREATE DATABASE csfx OWNER csfx;"
