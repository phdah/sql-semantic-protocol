CREATE TABLE snapshot AS
SELECT customer_id
FROM raw.customers
WHERE customer_id > 100;
