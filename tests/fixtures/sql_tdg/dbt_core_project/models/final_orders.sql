select
    amount,
    status,
    region
from {{ ref('enriched_orders') }}
where amount >= 40
  and amount < 60
