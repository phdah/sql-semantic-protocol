CREATE VIEW stage_orders AS
SELECT
    order_id,
    customer_id,
    amount,
    CASE WHEN amount >= 100 THEN 'high' ELSE 'standard' END AS amount_bucket
FROM raw_orders
WHERE amount = 100;

CREATE VIEW stage_customers AS
SELECT
    customer_id,
    active
FROM raw_customers
WHERE active = TRUE;

CREATE VIEW core_enriched AS
SELECT
    orders.customer_id,
    orders.amount,
    orders.amount_bucket
FROM stage_orders AS orders
JOIN stage_customers AS customers
  ON orders.customer_id = customers.customer_id;

CREATE VIEW mart_customer_summary AS
SELECT
    enriched.amount_bucket,
    COUNT(*) AS order_count,
    MAX(enriched.amount) AS max_amount
FROM core_enriched AS enriched
WHERE enriched.amount = 100
GROUP BY enriched.amount_bucket
HAVING COUNT(*) >= 1
ORDER BY enriched.amount_bucket;
