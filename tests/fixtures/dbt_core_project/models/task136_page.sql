select k
from {{ source('raw', 'task136_source') }}
order by k
limit 2
