select
    reason,
    count(*) as return_count
from {{ source('raw', 'returns') }}
group by reason
