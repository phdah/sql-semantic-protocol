select
    o.amount,
    o.region
from {{ ref('enriched_orders') }} as o
where exists (
    select 1
    from {{ ref('stg_customers') }} as c
    where c.customer_id = o.customer_id
)
