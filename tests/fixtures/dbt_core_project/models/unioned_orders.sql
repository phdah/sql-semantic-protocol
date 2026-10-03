select
    order_id,
    customer_id,
    amount,
    status,
    region
from {{ ref('ranked_orders') }}
union all
select
    id as order_id,
    customer_id,
    amount,
    status,
    region
from {{ source('raw', 'legacy_orders') }}
where amount >= 20
  and amount <= 80
