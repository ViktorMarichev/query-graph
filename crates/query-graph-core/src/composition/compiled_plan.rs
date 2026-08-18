use serde_json::Value;

use crate::{
  MappedQueryGraph, OracleCompiler, QueryOperation, SqlCompileError, SqlServerCompiler,
  SqlStatement,
};

use super::model::{BatchPlanMetadata, ComposedCompileError};

#[derive(Debug, Clone, Copy)]
pub(super) enum PlanCompiler {
  Oracle(OracleCompiler),
  SqlServer(SqlServerCompiler),
}

impl PlanCompiler {
  pub(super) fn compile(
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
pub(super) struct CompiledBatchStep {
  pub(super) metadata: BatchPlanMetadata,
  pub(super) graph: MappedQueryGraph,
  pub(super) parameter: String,
  pub(super) operation: QueryOperation,
}

#[derive(Debug, Clone)]
pub struct CompiledQueryPlan {
  root: SqlStatement,
  batches: Vec<CompiledBatchStep>,
  compiler: PlanCompiler,
}

impl CompiledQueryPlan {
  pub(super) fn new(
    root: SqlStatement,
    batches: Vec<CompiledBatchStep>,
    compiler: PlanCompiler,
  ) -> Self {
    Self {
      root,
      batches,
      compiler,
    }
  }

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
