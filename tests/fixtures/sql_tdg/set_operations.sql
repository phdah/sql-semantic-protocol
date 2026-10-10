CREATE VIEW set_a AS
SELECT 1 AS marker
FROM raw_a
WHERE value = 10;

CREATE VIEW set_b AS
SELECT 2 AS marker
FROM raw_b
WHERE value = 20;

CREATE VIEW set_c AS
SELECT 1 AS marker
FROM raw_c
WHERE value = 30;

CREATE VIEW union_all_result AS
SELECT marker FROM set_a
UNION ALL
SELECT marker FROM set_b
ORDER BY marker
LIMIT 2;

CREATE VIEW union_result AS
SELECT marker FROM set_a
UNION
SELECT marker FROM set_c
ORDER BY marker;

CREATE VIEW intersect_result AS
SELECT marker FROM set_a
INTERSECT
SELECT marker FROM set_c
ORDER BY marker;

CREATE VIEW except_result AS
SELECT marker FROM set_a
EXCEPT
SELECT marker FROM set_b
ORDER BY marker;

CREATE VIEW distinct_result AS
SELECT DISTINCT marker
FROM set_a
ORDER BY marker;

CREATE VIEW limited_result AS
SELECT marker FROM set_a
UNION ALL
SELECT marker FROM set_b
ORDER BY marker
LIMIT 1;
