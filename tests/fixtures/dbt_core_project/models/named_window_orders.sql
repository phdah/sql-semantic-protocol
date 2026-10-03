select
    order_id,
    customer_id,
    amount,
    sum(amount) over customer_window as running_amount
from {{ ref('stg_orders') }}
window customer_window as (
    partition by customer_id
    order by created_at
    rows between unbounded preceding and current row
)
