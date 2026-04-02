#!/bin/bash
set -euo pipefail

sqlplus -s "${APP_USER:?}/${APP_USER_PASSWORD:?}@//localhost:1521/${ORACLE_PDB:-FREEPDB1}" <<'SQL'
WHENEVER SQLERROR EXIT SQL.SQLCODE
@/container-entrypoint-initdb.d/init.sql
@/container-entrypoint-initdb.d/seed_valuations.sql
EXIT;
SQL