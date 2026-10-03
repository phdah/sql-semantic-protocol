CREATE VIEW final_orders AS
SELECT id, amount
FROM stage_orders
WHERE amount <= 10;
