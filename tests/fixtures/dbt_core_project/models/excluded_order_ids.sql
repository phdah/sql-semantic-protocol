select order_id
from {{ ref('unioned_orders') }}
except
select order_id
from {{ source('raw', 'returns') }}
