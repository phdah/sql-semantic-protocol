select
    customer_id,
    count(*) as order_count,
    sum(amount) as total_amount,
    sum(amount) filter (where status = 'paid') as paid_amount
from {{ ref('unioned_orders') }}
group by customer_id
having sum(amount) > 20
