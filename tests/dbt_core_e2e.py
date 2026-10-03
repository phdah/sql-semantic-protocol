#!/usr/bin/env python3

import json
import sys
from pathlib import Path


def normalized_relation(name: str) -> str:
    return name.replace('"', "").replace("`", "").lower()


def relation_ends_with(name: str, suffix: str) -> bool:
    return normalized_relation(name).endswith(suffix.lower())


def produced_relation(layer: dict) -> str:
    relations = [
        dataset["name"]
        for dataset in layer["produces"]
        if dataset["kind"] == "relation"
    ]
    if len(relations) != 1:
        raise AssertionError(f"expected one produced relation, got {relations}")
    return relations[0]


def layer_for_suffix(protocol: dict, suffix: str) -> dict:
    matches = [
        layer
        for layer in protocol["layers"]
        if relation_ends_with(produced_relation(layer), suffix)
    ]
    if len(matches) != 1:
        raise AssertionError(f"expected one layer ending in {suffix}, got {len(matches)}")
    return matches[0]


def literal_value(bound: dict) -> str:
    return str(bound["value"]["value"])


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: dbt_core_e2e.py <manifest.json> <protocol.json>")

    manifest = json.loads(Path(sys.argv[1]).read_text())
    protocol = json.loads(Path(sys.argv[2]).read_text())

    schema_url = manifest["metadata"]["dbt_schema_version"]
    if not schema_url.endswith("/manifest/v12.json"):
        raise AssertionError(f"expected dbt manifest v12, got {schema_url}")

    nodes = manifest["nodes"]
    project = "sql_semantic_protocol_e2e"
    stage_id = f"model.{project}.stg_orders"
    final_id = f"model.{project}.final_orders"

    for node_id in (stage_id, final_id):
        node = nodes[node_id]
        compiled = node.get("compiled_code")
        if not compiled or "{{" in compiled or "{%" in compiled:
            raise AssertionError(f"{node_id} does not contain compiled SQL")
        if not node.get("relation_name"):
            raise AssertionError(f"{node_id} is missing relation_name")

    input_ids = [item["id"] for item in protocol["inputs"]]
    if input_ids != [stage_id, final_id]:
        raise AssertionError(f"unexpected protocol input ids: {input_ids}")

    stage = layer_for_suffix(protocol, ".stg_orders")
    final = layer_for_suffix(protocol, ".final_orders")

    stage_edges = [
        edge
        for edge in protocol["graph"]["edges"]
        if edge["consumer_layer_id"] == stage["id"]
    ]
    if len(stage_edges) != 1 or stage_edges[0]["resolution"] != "external":
        raise AssertionError(f"stage source edge was not external: {stage_edges}")
    if not relation_ends_with(stage_edges[0]["relation"], ".raw.orders"):
        raise AssertionError(
            f"unexpected source relation: {stage_edges[0]['relation']}"
        )

    final_edges = [
        edge
        for edge in protocol["graph"]["edges"]
        if edge["consumer_layer_id"] == final["id"]
    ]
    if len(final_edges) != 1 or final_edges[0]["resolution"] != "resolved":
        raise AssertionError(f"final model edge was not resolved: {final_edges}")
    if final_edges[0]["producer_layer_ids"] != [stage["id"]]:
        raise AssertionError(
            f"final model did not resolve to stage: {final_edges[0]}"
        )

    semantics = final["composed_semantics"]
    if semantics["status"] != "resolved":
        raise AssertionError(f"final semantics are not resolved: {semantics}")

    dependencies = semantics["dependencies"]
    if len(dependencies) != 1 or not relation_ends_with(
        dependencies[0], ".raw.orders"
    ):
        raise AssertionError(f"unexpected composed dependencies: {dependencies}")

    amount_columns = [
        column
        for column in semantics["output"]["columns"]
        if column["name"].lower() == "amount"
    ]
    if len(amount_columns) != 1:
        raise AssertionError(
            f"expected one amount output column, got {len(amount_columns)}"
        )

    domain = amount_columns[0]["domain"]
    if domain["kind"] != "ranges" or len(domain["ranges"]) != 1:
        raise AssertionError(f"unexpected amount domain: {domain}")

    value_range = domain["ranges"][0]
    lower = value_range["lower"]
    upper = value_range["upper"]
    if literal_value(lower) != "10" or not lower["inclusive"]:
        raise AssertionError(f"unexpected lower bound: {lower}")
    if literal_value(upper) != "50" or not upper["inclusive"]:
        raise AssertionError(f"unexpected upper bound: {upper}")

    print("dbt Core end-to-end manifest analysis passed")


if __name__ == "__main__":
    main()
