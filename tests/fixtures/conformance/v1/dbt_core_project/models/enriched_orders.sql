select
    o.order_id,
    o.customer_id,
    o.amount,
    o.status,
    o.created_at,
    o.region,
    o.amount_bucket,
    o.paid_flag,
    c.score
from {{ ref('stg_orders') }} as o
join {{ ref('stg_customers') }} as c
  on o.customer_id = c.customer_id
