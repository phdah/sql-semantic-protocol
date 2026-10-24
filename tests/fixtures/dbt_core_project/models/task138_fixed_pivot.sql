SELECT * FROM (VALUES (1,1,10),(1,2,20),(2,1,30)) AS t(id,k,v) PIVOT(SUM(v) FOR k IN (1 AS one,2 AS two))
