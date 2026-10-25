select k
from {{ source('raw', 'task133_source') }}
order by k
limit 2 offset 1
