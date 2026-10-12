select
    id as customer_id,
    score,
    active
from {{ source('raw', 'customers') }}
where score >= 5
  and score < 15
  and active = true
