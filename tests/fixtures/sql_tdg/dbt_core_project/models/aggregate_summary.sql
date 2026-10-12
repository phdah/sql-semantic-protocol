select
    region,
    count(*) as order_count,
    max(amount) as max_amount
from {{ ref('enriched_orders') }}
group by region
having count(*) >= 1
