ALTER SESSION SET CONTAINER = FREEPDB1;
ALTER SESSION SET CURRENT_SCHEMA = pdbadmin;
SET DEFINE ON
@/opt/oracle/scripts/bootstrap/03-seed-valuations-base.sql
EXIT;