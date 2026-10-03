CREATE TABLE stage_orders AS
SELECT id, amount
FROM raw.orders
WHERE amount >= 5;
