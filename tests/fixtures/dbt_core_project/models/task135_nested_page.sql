with page as (
    select k
    from {{ source('raw', 'task135_source') }}
    order by k
    limit 4
)
select k
from page
order by k
limit 1 offset 2
