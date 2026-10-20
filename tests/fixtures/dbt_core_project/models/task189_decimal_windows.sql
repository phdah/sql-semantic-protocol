select
    id,
    k,
    x,
    count(*) over (partition by k) as n,
    count(x) over (partition by k) as nn,
    sum(x) over (partition by k) as total,
    avg(x) over (partition by k) as mean,
    min(x) over (partition by k) as min_x,
    max(x) over (partition by k) as max_x
from {{ source('raw', 'task189_decimal_source') }}
