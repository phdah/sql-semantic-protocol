CREATE VIEW exists_result AS
SELECT 1 AS marker
FROM raw_exists_orders AS orders
WHERE orders.amount = 100
  AND EXISTS (
      SELECT 1
      FROM raw_exists_customers AS customers
      WHERE customers.customer_id = orders.customer_id
  );
