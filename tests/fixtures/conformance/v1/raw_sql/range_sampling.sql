CREATE VIEW range_result AS
SELECT value
FROM raw_range
WHERE value >= 10
  AND value <= 1000;
