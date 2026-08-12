use query_graph_core::{MappingIssueCode, RelationalMapping, TableName};
use serde_json::{json, Value};

fn mapping(value: Value) -> RelationalMapping {
  serde_json::from_value(value).expect("mapping fixture should be valid")
}

#[test]
fn merges_compatible_source_and_column_fragments() {
  let merged = RelationalMapping::merge([
    mapping(json!({
      "sources": {
        "users": {
          "table": { "schema": "dbo", "name": "Users" },
          "columns": { "organisationId": "organisation_id" }
        }
      }
    })),
    mapping(json!({
      "sources": {
        "users": {
          "table": { "schema": "dbo", "name": "Users" },
          "columns": {
            "organisationId": "organisation_id",
            "displayName": "display_name"
          }
        },
        "profiles": { "table": "Profiles" }
      }
    })),
  ])
  .expect("compatible mappings should merge");

  assert_eq!(merged.sources.len(), 2);
  let users = &merged.sources["users"];
  assert_eq!(
    users.table,
    TableName::Qualified {
      catalog: None,
      schema: Some("dbo".to_owned()),
      name: "Users".to_owned(),
    }
  );
  assert_eq!(users.columns["organisationId"], "organisation_id");
  assert_eq!(users.columns["displayName"], "display_name");
}

#[test]
fn treats_unqualified_table_forms_as_equivalent() {
  let merged = RelationalMapping::merge([
    mapping(json!({
      "sources": { "users": { "table": "Users" } }
    })),
    mapping(json!({
      "sources": { "users": { "table": { "name": "Users" } } }
    })),
  ])
  .expect("equivalent unqualified tables should merge");

  assert_eq!(
    merged.sources["users"].table,
    TableName::Name("Users".to_owned())
  );
}
#[test]
fn rejects_conflicting_source_tables() {
  let issues = RelationalMapping::merge([
    mapping(json!({
      "sources": { "users": { "table": "Users" } }
    })),
    mapping(json!({
      "sources": { "users": { "table": "LegacyUsers" } }
    })),
  ])
  .expect_err("conflicting tables should be rejected");

  assert_eq!(issues.as_slice().len(), 1);
  assert_eq!(
    issues.as_slice()[0].code,
    MappingIssueCode::ConflictingSourceTable
  );
  assert_eq!(
    issues.as_slice()[0].location,
    "mappings[1].sources.users.table"
  );
}

#[test]
fn rejects_conflicting_column_names() {
  let issues = RelationalMapping::merge([
    mapping(json!({
      "sources": {
        "users": { "table": "Users", "columns": { "id": "user_id" } }
      }
    })),
    mapping(json!({
      "sources": {
        "users": { "table": "Users", "columns": { "id": "legacy_user_id" } }
      }
    })),
  ])
  .expect_err("conflicting columns should be rejected");

  assert_eq!(issues.as_slice().len(), 1);
  assert_eq!(
    issues.as_slice()[0].code,
    MappingIssueCode::ConflictingColumn
  );
  assert_eq!(
    issues.as_slice()[0].location,
    "mappings[1].sources.users.columns.id"
  );
}
