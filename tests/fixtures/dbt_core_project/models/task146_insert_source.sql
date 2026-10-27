WITH staged AS (
    SELECT id, score
    FROM {{ ref('task146_source') }}
    WHERE id > 1
)
SELECT score, id FROM staged
