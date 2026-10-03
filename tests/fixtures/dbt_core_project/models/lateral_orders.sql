select
    o.order_id,
    derived.adjusted_amount
from {{ ref('stg_orders') }} as o
join lateral (
    select o.amount + 1 as adjusted_amount
) as derived on true
