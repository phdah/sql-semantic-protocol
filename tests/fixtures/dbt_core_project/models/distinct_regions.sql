select distinct region
from {{ ref('unioned_orders') }}
