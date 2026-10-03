INSERT INTO order_audit (id, amount)
SELECT id, amount
FROM stage_orders
WHERE amount <= 20;
