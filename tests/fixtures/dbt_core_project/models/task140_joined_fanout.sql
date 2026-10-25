SELECT t.v, COUNT(*) AS n FROM UNNEST([1,1,2]) AS t(v) JOIN (VALUES(1),(1),(2)) AS u(v) ON t.v = u.v GROUP BY t.v
