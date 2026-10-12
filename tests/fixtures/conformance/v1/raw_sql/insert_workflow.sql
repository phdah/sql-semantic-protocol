INSERT INTO insert_sink (value)
SELECT value
FROM raw_insert
WHERE value = 10;
