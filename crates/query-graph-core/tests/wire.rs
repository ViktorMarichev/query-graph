use query_graph_core::{GraphDefinition, LiteralValue, QueryOperation, RelationalMapping};
use serde_json::json;

#[test]
fn integer_wire_fields_keep_rust_ranges_and_reject_floats() {
  for value in [i64::MIN, -5_000_000_000, 5_000_000_000, i64::MAX] {
    assert_eq!(
      serde_json::from_value::<LiteralValue>(json!({ "kind": "integer", "value": value })).unwrap(),
      LiteralValue::Integer(value)
    );
  }

  for name in ["offset", "limit"] {
    let operation: QueryOperation = serde_json::from_value(json!({ name: u64::MAX })).unwrap();
    assert_eq!(
      if name == "offset" {
        operation.offset
      } else {
        operation.limit
      },
      Some(u64::MAX)
    );
  }

  // The Node adapter repairs JS number representation; core wire decoding stays strict.
  for value in [json!(5_000_000_000.0), json!(1.5), json!("5000000000")] {
    assert!(serde_json::from_value::<LiteralValue>(json!({
      "kind": "integer", "value": value
    }))
    .is_err());
    for name in ["offset", "limit"] {
      assert!(serde_json::from_value::<QueryOperation>(json!({ name: value })).is_err());
    }
  }
}

#[test]
fn rejects_unknown_definition_fields() {
  let result = serde_json::from_value::<GraphDefinition>(json!({
    "schemaVersion": 12,
    "name": "strictDefinition",
    "root": "root",
    "sources": [{
      "key": "root",
      "fields": [{
        "name": "id",
        "scalarType": "int64",
        "nullable": false,
        "selectabel": true
      }]
    }],
    "projection": {
      "fields": [{
        "path": ["id"],
        "expression": {
          "kind": "field",
          "source": "root",
          "field": "id"
        }
      }]
    }
  }));

  let error = result.unwrap_err().to_string();
  assert!(error.contains("selectabel"));
}

#[test]
fn rejects_legacy_constraint_names() {
  let result = serde_json::from_value::<GraphDefinition>(json!({
    "schemaVersion": 12,
    "name": "strictConstraints",
    "root": "root",
    "sources": [{
      "key": "root",
      "fields": [{
        "name": "id",
        "scalarType": "int64"
      }]
    }],
    "constraints": [{
      "name": "active",
      "predicate": {
        "kind": "literal",
        "value": {
          "kind": "boolean",
          "value": true
        }
      }
    }]
  }));

  assert!(result.unwrap_err().to_string().contains("name"));
}

#[test]
fn rejects_unknown_semantic_functions_during_deserialization() {
  let result = serde_json::from_value::<GraphDefinition>(json!({
    "schemaVersion": 12,
    "name": "strictFunctions",
    "root": "root",
    "sources": [{
      "key": "root",
      "fields": [{
        "name": "name",
        "scalarType": "string"
      }]
    }],
    "projection": {
      "fields": [{
        "path": ["name"],
        "expression": {
          "kind": "function",
          "name": "databaseSpecificFunction",
          "arguments": [{
            "kind": "field",
            "source": "root",
            "field": "name"
          }]
        }
      }]
    }
  }));

  let error = result.unwrap_err().to_string();
  assert!(error.contains("databaseSpecificFunction"));
}

#[test]
fn rejects_unknown_exists_fields_during_deserialization() {
  let result = serde_json::from_value::<GraphDefinition>(json!({
    "schemaVersion": 12,
    "name": "strictExists",
    "root": "root",
    "sources": [
      {
        "key": "root",
        "fields": [{
          "name": "id",
          "scalarType": "int64"
        }]
      },
      {
        "key": "child",
        "fields": [{
          "name": "idRoot",
          "scalarType": "int64"
        }]
      }
    ],
    "relations": [{
      "name": "child",
      "from": "root",
      "to": "child",
      "on": {
        "kind": "eq",
        "left": {
          "kind": "field",
          "source": "root",
          "field": "id"
        },
        "right": {
          "kind": "field",
          "source": "child",
          "field": "idRoot"
        }
      }
    }],
    "constraints": [{
      "predicate": {
        "kind": "exists",
        "source": "child",
        "predciate": {
          "kind": "literal",
          "value": {
            "kind": "boolean",
            "value": true
          }
        }
      }
    }]
  }));

  let error = result.unwrap_err().to_string();
  assert!(error.contains("predciate"));
}

#[test]
fn rejects_unknown_operation_fields() {
  let result = serde_json::from_value::<QueryOperation>(json!({
    "parameters": {},
    "limt": 10
  }));

  let error = result.unwrap_err().to_string();
  assert!(error.contains("limt"));
}

#[test]
fn rejects_unknown_mapping_fields() {
  let result = serde_json::from_value::<RelationalMapping>(json!({
    "sources": {
      "root": {
        "table": "Root",
        "colums": {
          "id": "root_id"
        }
      }
    }
  }));

  let error = result.unwrap_err().to_string();
  assert!(error.contains("colums"));
}
