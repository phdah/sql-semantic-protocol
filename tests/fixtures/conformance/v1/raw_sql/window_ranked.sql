CREATE VIEW ranked_result AS
SELECT
    1 AS marker,
    ROW_NUMBER() OVER (
        PARTITION BY category
        ORDER BY score
    ) AS rn
FROM raw_window
WHERE category = 'keep'
  AND score = 10
QUALIFY rn <= 1
ORDER BY rn
LIMIT 1;
