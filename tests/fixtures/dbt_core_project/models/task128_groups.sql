select id, sum(x) over (
    partition by k order by x nulls last
    groups between 1 preceding and current row
) as total
from {{ source('raw', 'task128_window_source') }}
