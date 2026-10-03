select derived.order_id, derived.amount
from (
    select order_id, amount
    from {{ ref('stg_orders') }}
    where amount >= 20
) as derived
