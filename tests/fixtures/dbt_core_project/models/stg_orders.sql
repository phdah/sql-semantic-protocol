select
    id as order_id,
    customer_id,
    amount,
    status,
    created_at,
    region,
    amount >= 50 as high_value,
    case
        when amount >= 75 then 'high'
        when amount >= 40 then 'medium'
        else 'standard'
    end as amount_bucket,
    case status
        when 'paid' then 1
        else 0
    end as paid_flag,
    amount + 5 as adjusted_amount
from {{ source('raw', 'orders') }}
where amount between 10 and 100
  and status in ('paid', 'pending')
