select
    id,
    amount
from {{ ref('stg_orders') }}
where amount <= 50
