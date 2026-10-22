select id, ntile(3) over (partition by k order by id nulls last) as tile
from {{ source('raw', 'task128_window_source') }}
qualify ntile(3) over (partition by k order by id nulls last) = 1
