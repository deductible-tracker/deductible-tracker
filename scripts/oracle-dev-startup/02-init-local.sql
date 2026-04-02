ALTER SESSION SET CONTAINER = FREEPDB1;
ALTER SESSION SET CURRENT_SCHEMA = pdbadmin;
SET HEADING OFF
SET FEEDBACK OFF
SET VERIFY OFF
SET PAGESIZE 0
COLUMN init_dispatch NEW_VALUE init_dispatch NOPRINT
SELECT CASE
				 WHEN EXISTS (
					 SELECT 1
					 FROM all_tables
					 WHERE owner = 'PDBADMIN'
						 AND table_name = 'USERS'
				 ) THEN '1'
				 ELSE '0'
			 END AS init_dispatch
FROM dual;

@/opt/oracle/scripts/bootstrap/02-init-dispatch-&init_dispatch..sql
EXIT;