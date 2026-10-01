# sql-semantic-protocol

A protocal with a built in SQL parser.

## Core idé

Take any SQL string/file, from any supported dialect from the [sqlparser](https://docs.rs/sqlparser/latest/sqlparser/) crate, and output a opinionated protocal format.

It represents the SQL query in a more human/AI readable way, with what's important for the output of the query after execution:
- What is the allowed interval for a given column?
    - E.g., column A (integer) must be within interval `10<=A<=20`
- What are the remaining column after the entire query?
    - E.g., only column A is kept from query: `select A from (select A, B from table);`
- What tables/views does the query depend on?
    - E.g., it depends on table T1 and T2 from query: `select * from T1 join T2 using A`
