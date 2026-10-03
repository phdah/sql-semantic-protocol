select
    order_id,
    customer_id,
    amount,
    status,
    created_at,
    region,
    row_number() over (
        partition by customer_id
        order by created_at
    ) as rn,
    sum(amount) over (
        partition by customer_id
        order by created_at
        rows between 1 preceding and current row
    ) as rolling_amount
from {{ ref('enriched_orders') }}
qualify rn <= 2
