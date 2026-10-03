select
    o.order_id,
    o.customer_id,
    o.amount,
    o.status,
    o.created_at,
    o.region,
    o.high_value,
    o.amount_bucket,
    o.paid_flag,
    c.score,
    r.refund_amount,
    o.amount + c.score as combined_value
from {{ ref('stg_orders') }} as o
join {{ ref('stg_customers') }} as c
  on o.customer_id = c.customer_id
left join {{ source('raw', 'returns') }} as r
  on o.order_id = r.order_id
