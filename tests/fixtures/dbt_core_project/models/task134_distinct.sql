select distinct k
from {{ source('raw', 'task134_source') }}
order by k
limit 3
