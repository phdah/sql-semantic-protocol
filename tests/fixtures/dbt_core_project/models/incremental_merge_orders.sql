{{ config(materialized='incremental', incremental_strategy='merge', unique_key='order_id') }}

select
    order_id,
    customer_id,
    amount
from {{ ref('stg_orders') }}
