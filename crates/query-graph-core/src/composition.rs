use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
  MappedQueryGraph, OperationIssue, OperationIssueCode, OperationIssues, OracleCompiler,
  ParameterShape, QueryOperation, RelationCardinality, SqlCompileError, SqlServerCompiler,
  SqlStatement,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchRelationDefinition {
  pub name: String,
  pub from: String,
  pub to: String,
  pub parameter: String,
  pub cardinality: RelationCardinality,
  #[serde(default)]
  pub parameters: HashMap<String, Value>,
  #[serde(default)]
  pub ordering: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionIssue {
  pub code: CompositionIssueCode,
  pub location: String,
  pub message: String,
}

impl CompositionIssue {
  fn new(
    code: CompositionIssueCode,
    location: impl Into<String>,
    message: impl Into<String>,
  ) -> Self {
    Self {
      code,
      location: location.into(),
      message: message.into(),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CompositionIssueCode {
  EmptyRelationName,
  InvalidRelationName,
  DuplicateRelationName,
  ConflictingProjectionPath,
  UnknownParentKey,
  UnknownChildKey,
  UnknownKeyParameter,
  KeyParameterNotList,
  IncompatibleKeyTypes,
  KeyParameterIsStatic,
  UnknownStaticParameter,
  MissingStaticParameter,
  InvalidStaticParameterType,
  UnknownChildOrdering,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CompositionIssues(Vec<CompositionIssue>);

impl CompositionIssues {
  pub fn as_slice(&self) -> &[CompositionIssue] {
    &self.0
  }

  pub fn into_vec(self) -> Vec<CompositionIssue> {
    self.0
  }
}

impl fmt::Display for CompositionIssues {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      formatter,
      "query graph composition contains {} issue(s)",
      self.0.len()
    )?;

    for issue in &self.0 {
      write!(
        formatter,
        "\n- {:?} at {}: {}",
        issue.code, issue.location, issue.message
      )?;
    }

    Ok(())
  }
}

impl Error for CompositionIssues {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchPlanMetadata {
  pub name: String,
  pub parent_key: String,
  pub child_key: String,
  pub key_parameter: String,
  pub parameters: HashMap<String, Value>,
  pub cardinality: RelationCardinality,
  pub parent_key_injected: bool,
  pub child_key_injected: bool,
}

#[derive(Debug)]
pub enum ComposedCompileError {
  Operation(OperationIssues),
  Sql(SqlCompileError),
  UnknownSelectedBatchRelation(String),
}

impl fmt::Display for ComposedCompileError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Operation(error) => error.fmt(formatter),
      Self::Sql(error) => error.fmt(formatter),
      Self::UnknownSelectedBatchRelation(name) => {
        write!(
          formatter,
          "batch relation {name:?} is not selected by the query plan"
        )
      }
    }
  }
}

impl Error for ComposedCompileError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    match self {
      Self::Operation(error) => Some(error),
      Self::Sql(error) => Some(error),
      Self::UnknownSelectedBatchRelation(_) => None,
    }
  }
}

impl From<OperationIssues> for ComposedCompileError {
  fn from(error: OperationIssues) -> Self {
    Self::Operation(error)
  }
}

impl From<SqlCompileError> for ComposedCompileError {
  fn from(error: SqlCompileError) -> Self {
    Self::Sql(error)
  }
}

#[derive(Debug, Clone)]
struct BatchRelation {
  definition: BatchRelationDefinition,
  graph: MappedQueryGraph,
}

#[derive(Debug, Clone)]
pub struct ComposedQueryGraph {
  root: MappedQueryGraph,
  relations: Vec<BatchRelation>,
}

impl ComposedQueryGraph {
  pub fn new(root: MappedQueryGraph) -> Self {
    Self {
      root,
      relations: Vec::new(),
    }
  }

  pub fn root(&self) -> &MappedQueryGraph {
    &self.root
  }

  pub fn with_batch_relation(
    mut self,
    graph: MappedQueryGraph,
    definition: BatchRelationDefinition,
  ) -> Result<Self, CompositionIssues> {
    self.validate_relation(&graph, &definition)?;
    self.relations.push(BatchRelation { definition, graph });
    Ok(self)
  }

  pub fn compile_oracle_plan(
    &self,
    operation: &QueryOperation,
  ) -> Result<CompiledQueryPlan, ComposedCompileError> {
    self.compile_oracle_plan_with(operation, &OracleCompiler::default())
  }

  pub fn compile_oracle_plan_with(
    &self,
    operation: &QueryOperation,
    compiler: &OracleCompiler,
  ) -> Result<CompiledQueryPlan, ComposedCompileError> {
    self.compile_plan(operation, PlanCompiler::Oracle(*compiler))
  }

  pub fn compile_sql_server_plan(
    &self,
    operation: &QueryOperation,
  ) -> Result<CompiledQueryPlan, ComposedCompileError> {
    self.compile_sql_server_plan_with(operation, &SqlServerCompiler::default())
  }

  pub fn compile_sql_server_plan_with(
    &self,
    operation: &QueryOperation,
    compiler: &SqlServerCompiler,
  ) -> Result<CompiledQueryPlan, ComposedCompileError> {
    self.compile_plan(operation, PlanCompiler::SqlServer(*compiler))
  }

  fn validate_relation(
    &self,
    graph: &MappedQueryGraph,
    definition: &BatchRelationDefinition,
  ) -> Result<(), CompositionIssues> {
    let relation_index = self.relations.len();
    let location = format!("relations[{relation_index}]");
    let mut issues = Vec::new();

    if definition.name.trim().is_empty() {
      issues.push(CompositionIssue::new(
        CompositionIssueCode::EmptyRelationName,
        format!("{location}.name"),
        "batch relation name must not be empty",
      ));
    } else if definition.name.contains('.') {
      issues.push(CompositionIssue::new(
        CompositionIssueCode::InvalidRelationName,
        format!("{location}.name"),
        "batch relation name must be a single projection path segment",
      ));
    }

    if self
      .relations
      .iter()
      .any(|relation| relation.definition.name == definition.name)
    {
      issues.push(CompositionIssue::new(
        CompositionIssueCode::DuplicateRelationName,
        format!("{location}.name"),
        format!(
          "batch relation {:?} is defined more than once",
          definition.name
        ),
      ));
    }

    let namespace = format!("{}.", definition.name);
    if self
      .root
      .graph()
      .definition()
      .projection
      .fields
      .iter()
      .map(|field| field.path.join("."))
      .any(|path| path == definition.name || path.starts_with(&namespace))
    {
      issues.push(CompositionIssue::new(
        CompositionIssueCode::ConflictingProjectionPath,
        format!("{location}.name"),
        format!(
          "batch relation {:?} conflicts with a root projection path",
          definition.name
        ),
      ));
    }

    let parent_type = self.root.graph().projection_type(&definition.from);
    if parent_type.is_none() {
      issues.push(CompositionIssue::new(
        CompositionIssueCode::UnknownParentKey,
        format!("{location}.from"),
        format!("projection field {:?} is not defined", definition.from),
      ));
    }

    let child_type = graph.graph().projection_type(&definition.to);
    if child_type.is_none() {
      issues.push(CompositionIssue::new(
        CompositionIssueCode::UnknownChildKey,
        format!("{location}.to"),
        format!("projection field {:?} is not defined", definition.to),
      ));
    }

    let parameter = graph.graph().parameter(&definition.parameter);
    match parameter {
      None => issues.push(CompositionIssue::new(
        CompositionIssueCode::UnknownKeyParameter,
        format!("{location}.parameter"),
        format!("parameter {:?} is not defined", definition.parameter),
      )),
      Some(parameter) if parameter.shape != ParameterShape::List => {
        issues.push(CompositionIssue::new(
          CompositionIssueCode::KeyParameterNotList,
          format!("{location}.parameter"),
          "batch key parameter must have list shape",
        ));
      }
      Some(_) => {}
    }

    if let (Some(parent_type), Some(child_type), Some(parameter)) =
      (parent_type, child_type, parameter)
    {
      let types = [
        parent_type.scalar_type,
        child_type.scalar_type,
        parameter.scalar_type,
      ];
      if types.iter().any(|scalar_type| *scalar_type != types[0]) {
        issues.push(CompositionIssue::new(
          CompositionIssueCode::IncompatibleKeyTypes,
          location.clone(),
          "parent key, child key, and key parameter must have the same scalar type",
        ));
      }
    }

    if definition.parameters.contains_key(&definition.parameter) {
      issues.push(CompositionIssue::new(
        CompositionIssueCode::KeyParameterIsStatic,
        format!("{location}.parameters.{}", definition.parameter),
        "the batch key parameter is supplied automatically",
      ));
    }

    self.validate_child_operation(graph, definition, &location, &mut issues);

    if issues.is_empty() {
      Ok(())
    } else {
      Err(CompositionIssues(issues))
    }
  }

  fn validate_child_operation(
    &self,
    graph: &MappedQueryGraph,
    definition: &BatchRelationDefinition,
    location: &str,
    issues: &mut Vec<CompositionIssue>,
  ) {
    let selected_path = graph
      .graph()
      .projection(&definition.to)
      .map(|_| definition.to.clone())
      .or_else(|| {
        graph
          .graph()
          .definition()
          .projection
          .fields
          .first()
          .map(|field| field.path.join("."))
      });
    let Some(selected_path) = selected_path else {
      return;
    };

    let mut parameters = definition.parameters.clone();
    if graph
      .graph()
      .parameter(&definition.parameter)
      .is_some_and(|parameter| parameter.shape == ParameterShape::List)
    {
      parameters.insert(definition.parameter.clone(), Value::Array(Vec::new()));
    }

    let operation = QueryOperation {
      select: Some(vec![selected_path]),
      ordering: definition.ordering.clone(),
      parameters,
      ..QueryOperation::default()
    };

    let Err(operation_issues) = operation.validate(graph.graph()) else {
      return;
    };

    let key_location = format!("parameters.{}", definition.parameter);
    for issue in operation_issues.into_vec() {
      if issue.location == key_location || issue.location.starts_with(&format!("{key_location}[")) {
        continue;
      }

      let (code, issue_location) = match issue.code {
        OperationIssueCode::UnknownOrdering => (
          CompositionIssueCode::UnknownChildOrdering,
          format!("{location}.ordering"),
        ),
        OperationIssueCode::UnknownParameter => (
          CompositionIssueCode::UnknownStaticParameter,
          prefix_operation_location(location, &issue.location),
        ),
        OperationIssueCode::MissingParameter => (
          CompositionIssueCode::MissingStaticParameter,
          prefix_operation_location(location, &issue.location),
        ),
        OperationIssueCode::InvalidParameterType => (
          CompositionIssueCode::InvalidStaticParameterType,
          prefix_operation_location(location, &issue.location),
        ),
        _ => continue,
      };
      issues.push(CompositionIssue::new(code, issue_location, issue.message));
    }
  }

  fn compile_plan(
    &self,
    operation: &QueryOperation,
    compiler: PlanCompiler,
  ) -> Result<CompiledQueryPlan, ComposedCompileError> {
    let Some(selected_paths) = operation.select.as_ref() else {
      let root = compiler.compile(&self.root, operation)?;
      return Ok(CompiledQueryPlan {
        root,
        batches: Vec::new(),
        compiler,
      });
    };

    let mut issues = Vec::new();
    let mut seen = HashSet::new();
    let mut root_select = Vec::new();
    let mut requested_root_paths = HashSet::new();
    let mut child_select: HashMap<usize, Vec<String>> = HashMap::new();

    for (index, path) in selected_paths.iter().enumerate() {
      if !seen.insert(path.as_str()) {
        issues.push(OperationIssue::new(
          OperationIssueCode::DuplicateSelection,
          format!("select[{index}]"),
          format!("projection field {path:?} is selected more than once"),
        ));
        continue;
      }

      if self.root.graph().projection(path).is_some() {
        root_select.push(path.clone());
        requested_root_paths.insert(path.as_str());
        continue;
      }

      let relation = self
        .relations
        .iter()
        .enumerate()
        .find_map(|(relation_index, relation)| {
          let prefix = format!("{}.", relation.definition.name);
          let child_path = path.strip_prefix(&prefix)?;
          relation
            .graph
            .graph()
            .projection(child_path)
            .map(|_| (relation_index, child_path))
        });

      if let Some((relation_index, child_path)) = relation {
        child_select
          .entry(relation_index)
          .or_default()
          .push(child_path.to_owned());
      } else {
        issues.push(OperationIssue::new(
          OperationIssueCode::UnknownSelection,
          format!("select[{index}]"),
          format!("projection field {path:?} is not defined"),
        ));
      }
    }

    if !issues.is_empty() {
      return Err(OperationIssues::from_vec(issues).into());
    }

    let mut batches = Vec::new();
    for (relation_index, relation) in self.relations.iter().enumerate() {
      let Some(mut selection) = child_select.remove(&relation_index) else {
        continue;
      };
      let parent_key_injected = !requested_root_paths.contains(relation.definition.from.as_str());
      let child_key_injected = !selection.contains(&relation.definition.to);

      if !root_select.contains(&relation.definition.from) {
        root_select.push(relation.definition.from.clone());
      }
      if child_key_injected {
        selection.push(relation.definition.to.clone());
      }

      batches.push(CompiledBatchStep {
        metadata: BatchPlanMetadata {
          name: relation.definition.name.clone(),
          parent_key: relation.definition.from.clone(),
          child_key: relation.definition.to.clone(),
          key_parameter: relation.definition.parameter.clone(),
          parameters: relation.definition.parameters.clone(),
          cardinality: relation.definition.cardinality,
          parent_key_injected,
          child_key_injected,
        },
        graph: relation.graph.clone(),
        parameter: relation.definition.parameter.clone(),
        operation: QueryOperation {
          select: Some(selection),
          ordering: relation.definition.ordering.clone(),
          parameters: relation.definition.parameters.clone(),
          ..QueryOperation::default()
        },
      });
    }

    let root_operation = QueryOperation {
      select: Some(root_select),
      ordering: operation.ordering.clone(),
      parameters: operation.parameters.clone(),
      offset: operation.offset,
      limit: operation.limit,
    };
    let root = compiler.compile(&self.root, &root_operation)?;

    Ok(CompiledQueryPlan {
      root,
      batches,
      compiler,
    })
  }
}

fn prefix_operation_location(relation_location: &str, operation_location: &str) -> String {
  operation_location.strip_prefix("parameters").map_or_else(
    || format!("{relation_location}.{operation_location}"),
    |suffix| format!("{relation_location}.parameters{suffix}"),
  )
}

#[derive(Debug, Clone, Copy)]
enum PlanCompiler {
  Oracle(OracleCompiler),
  SqlServer(SqlServerCompiler),
}

impl PlanCompiler {
  fn compile(
    self,
    graph: &MappedQueryGraph,
    operation: &QueryOperation,
  ) -> Result<SqlStatement, SqlCompileError> {
    match self {
      Self::Oracle(compiler) => graph.compile_oracle_with(operation, &compiler),
      Self::SqlServer(compiler) => graph.compile_sql_server_with(operation, &compiler),
    }
  }
}

#[derive(Debug, Clone)]
struct CompiledBatchStep {
  metadata: BatchPlanMetadata,
  graph: MappedQueryGraph,
  parameter: String,
  operation: QueryOperation,
}

#[derive(Debug, Clone)]
pub struct CompiledQueryPlan {
  root: SqlStatement,
  batches: Vec<CompiledBatchStep>,
  compiler: PlanCompiler,
}

impl CompiledQueryPlan {
  pub fn root(&self) -> &SqlStatement {
    &self.root
  }

  pub fn batches(&self) -> impl ExactSizeIterator<Item = &BatchPlanMetadata> {
    self.batches.iter().map(|step| &step.metadata)
  }

  pub fn compile_batch(
    &self,
    name: &str,
    keys: &[Value],
  ) -> Result<SqlStatement, ComposedCompileError> {
    let step = self
      .batches
      .iter()
      .find(|step| step.metadata.name == name)
      .ok_or_else(|| ComposedCompileError::UnknownSelectedBatchRelation(name.to_owned()))?;
    let mut operation = step.operation.clone();
    operation
      .parameters
      .insert(step.parameter.clone(), Value::Array(keys.to_vec()));
    self
      .compiler
      .compile(&step.graph, &operation)
      .map_err(Into::into)
  }
}
