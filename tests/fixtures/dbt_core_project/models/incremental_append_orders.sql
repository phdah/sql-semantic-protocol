{{ config(materialized='incremental', incremental_strategy='append') }}

select
    order_id,
    customer_id,
    amount
from {{ ref('stg_orders') }}
