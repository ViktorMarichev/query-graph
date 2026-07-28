use query_graph_core::{GraphDefinition, QueryOperation, RelationalMapping};
use serde_json::json;

#[test]
fn rejects_unknown_definition_fields() {
  let result = serde_json::from_value::<GraphDefinition>(json!({
    "schemaVersion": 4,
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
    "schemaVersion": 4,
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
    "schemaVersion": 4,
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
      "name": "child",
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
