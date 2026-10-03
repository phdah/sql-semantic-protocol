select
    region,
    sum(amount) as total_amount
from {{ ref('unioned_orders') }}
group by rollup(region)
