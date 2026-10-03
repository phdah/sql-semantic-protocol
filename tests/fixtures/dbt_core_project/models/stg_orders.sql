select
    id,
    amount
from {{ source('raw', 'orders') }}
where amount >= 10
  and amount <= 100
