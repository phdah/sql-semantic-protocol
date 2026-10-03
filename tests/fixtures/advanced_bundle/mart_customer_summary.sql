CREATE VIEW mart.customer_summary AS
SELECT
    ranked.customer_id,
    COUNT(*) AS order_count,
    MAX(ranked.amount) AS max_amount
FROM core.ranked_orders AS ranked
WHERE EXISTS (
    SELECT 1
    FROM raw.allowed_customers AS allowed
    WHERE allowed.customer_id = ranked.customer_id
)
GROUP BY ranked.customer_id
