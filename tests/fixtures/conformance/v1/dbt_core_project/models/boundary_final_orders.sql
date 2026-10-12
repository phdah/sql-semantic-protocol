select
    amount
from {{ ref('stg_orders') }}
where amount >= 30
  and amount < 70
