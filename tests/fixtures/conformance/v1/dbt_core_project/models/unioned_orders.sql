select
    amount,
    region
from {{ ref('enriched_orders') }}
union all
select
    amount,
    region
from {{ source('raw', 'legacy_orders') }}
where amount >= 30
  and amount < 60
  and region = 'north'
