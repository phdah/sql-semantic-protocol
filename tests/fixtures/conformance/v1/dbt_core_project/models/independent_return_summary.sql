select
    reason,
    count(*) as return_count,
    max(refund_amount) as max_refund
from {{ source('raw', 'returns') }}
where refund_amount >= 50
group by reason
