use query_graph_core::{GraphDefinition, QueryOperation, RelationalMapping};
use serde_json::json;

#[test]
fn rejects_unknown_definition_fields() {
  let result = serde_json::from_value::<GraphDefinition>(json!({
    "schemaVersion": 3,
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
fn rejects_unknown_semantic_functions_during_deserialization() {
  let result = serde_json::from_value::<GraphDefinition>(json!({
    "schemaVersion": 3,
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
