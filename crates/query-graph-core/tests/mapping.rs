use query_graph_core::{
  CompiledGraph, FieldDefinition, GraphDefinition, MappingIssueCode, RelationalMapping, ScalarType,
  SourceDefinition, TableName,
};
use serde_json::{json, Value};

fn mapping(value: Value) -> RelationalMapping {
  serde_json::from_value(value).expect("mapping fixture should be valid")
}

fn users_graph() -> CompiledGraph {
  let mut definition = GraphDefinition::new("users", "users");
  definition.sources = vec![SourceDefinition::new(
    "users",
    vec![FieldDefinition::new("id", ScalarType::Int64)],
  )];
  definition.compile().unwrap()
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
      database_link: None,
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

#[test]
fn validates_database_link_names() {
  let graph = users_graph();
  for name in [
    "REMOTE",
    "remote_gate",
    "Remote.Example.Com",
    "REMOTE.123_DOMAIN@READ_ONLY",
    "R$1#_@C$1#_",
    &"A".repeat(128),
  ] {
    mapping(json!({
      "sources": { "users": { "table": { "name": "Users", "databaseLink": name } } }
    }))
    .compile(&graph)
    .unwrap_or_else(|error| panic!("expected valid database link {name:?}: {error}"));
  }

  for name in [
    "",
    " ",
    "@REMOTE",
    "1REMOTE",
    "_REMOTE",
    " REMOTE",
    "REMOTE ",
    "REMOTE\n",
    "REMOTE\0",
    "R\u{00e9}MOTE",
    ".REMOTE",
    "REMOTE.",
    "REMOTE..COM",
    "REMOTE@",
    "REMOTE@@CONNECTION",
    "REMOTE@1CONNECTION",
    "REMOTE@CONNECTION.COM",
    "REMOTE.@CONNECTION",
    "\"REMOTE\"",
    "REMOTE'",
    "REMOTE--comment",
    "REMOTE/*comment*/",
    "REMOTE;SELECT 1",
    "REMOTE JOIN Users",
    &"A".repeat(129),
  ] {
    let issues = mapping(json!({
      "sources": { "users": { "table": { "name": "Users", "databaseLink": name } } }
    }))
    .compile(&graph)
    .expect_err(&format!("expected invalid database link {name:?}"));
    assert_eq!(issues.as_slice().len(), 1, "{name:?}");
    assert_eq!(
      issues.as_slice()[0].code,
      MappingIssueCode::InvalidDatabaseLink
    );
    assert_eq!(
      issues.as_slice()[0].location,
      "sources.users.table.databaseLink"
    );
  }
}

#[test]
fn serializes_database_links_without_changing_local_table_wire_format() {
  let remote =
    json!({ "catalog": null, "schema": "APP", "name": "Users", "databaseLink": "REMOTE" });
  let table: TableName = serde_json::from_value(remote.clone()).unwrap();
  assert_eq!(serde_json::to_value(table).unwrap(), remote);

  let local = json!({ "catalog": null, "schema": "APP", "name": "Users" });
  let table: TableName = serde_json::from_value(local.clone()).unwrap();
  assert_eq!(serde_json::to_value(table).unwrap(), local);

  for value in [
    json!(42),
    json!(true),
    json!([]),
    json!({}),
    json!(["REMOTE"]),
  ] {
    assert!(serde_json::from_value::<TableName>(json!({
      "name": "Users", "databaseLink": value
    }))
    .is_err());
  }
}

#[test]
fn merges_database_link_names_case_insensitively() {
  let merged = RelationalMapping::merge(["remote.example@reader", "REMOTE.EXAMPLE@READER"].map(
    |name| {
      mapping(json!({
        "sources": { "users": { "table": { "name": "Users", "databaseLink": name } } }
      }))
    },
  ))
  .unwrap();
  assert_eq!(merged.sources.len(), 1);
}

#[test]
fn rejects_conflicting_database_link_mappings() {
  for other_table in [
    json!("Users"),
    json!({ "name": "Users" }),
    json!({ "name": "Users", "databaseLink": "OTHER_REMOTE" }),
    json!({ "name": "Users", "databaseLink": "REMOTE@READER" }),
    json!({ "name": "users", "databaseLink": "REMOTE" }),
  ] {
    for reverse in [false, true] {
      let mut mappings = [
        mapping(json!({
          "sources": { "users": { "table": { "name": "Users", "databaseLink": "REMOTE" } } }
        })),
        mapping(json!({ "sources": { "users": { "table": other_table } } })),
      ];
      if reverse {
        mappings.reverse();
      }
      let issues = RelationalMapping::merge(mappings).unwrap_err();
      assert_eq!(issues.as_slice().len(), 1);
      assert_eq!(
        issues.as_slice()[0].code,
        MappingIssueCode::ConflictingSourceTable
      );
    }
  }
}
