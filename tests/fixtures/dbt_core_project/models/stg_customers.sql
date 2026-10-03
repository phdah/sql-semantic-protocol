select
    id as customer_id,
    score,
    active
from {{ source('raw', 'customers') }}
where score >= 0
  and active = true
