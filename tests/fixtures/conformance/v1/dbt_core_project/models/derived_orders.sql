select
    amount_bucket,
    paid_flag
from {{ ref('enriched_orders') }}
