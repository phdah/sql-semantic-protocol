select order_id
from {{ ref('unioned_orders') }}
intersect
select id as order_id
from {{ source('raw', 'orders') }}
