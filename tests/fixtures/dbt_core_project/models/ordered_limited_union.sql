select order_id
from {{ ref('stg_orders') }}
union all
select id as order_id
from {{ source('raw', 'legacy_orders') }}
order by order_id
limit 5
