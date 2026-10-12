select
    region,
    amount,
    row_number() over (
        partition by region
        order by amount desc
    ) as rn
from {{ ref('enriched_orders') }}
qualify rn <= 2
