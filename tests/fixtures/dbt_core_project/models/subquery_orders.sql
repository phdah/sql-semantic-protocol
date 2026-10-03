select
    o.order_id,
    o.customer_id,
    (
        select max(r.refund_amount)
        from {{ source('raw', 'returns') }} as r
        where r.order_id = o.order_id
    ) as max_refund
from {{ ref('unioned_orders') }} as o
where exists (
    select 1
    from {{ ref('stg_customers') }} as c
    where c.customer_id = o.customer_id
)
  and o.customer_id in (
    select a.customer_id
    from {{ ref('aggregated_orders') }} as a
  )
  and o.order_id not in (
    select r2.order_id
    from {{ source('raw', 'returns') }} as r2
    where r2.refund_amount >= 50
  )
