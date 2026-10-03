CREATE TABLE stage.orders AS
SELECT
    order_id,
    customer_id,
    amount,
    created_at,
    CASE WHEN amount >= 100 THEN TRUE ELSE FALSE END AS high_value
FROM raw.orders
WHERE amount > 0
