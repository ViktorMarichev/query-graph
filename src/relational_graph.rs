use std::sync::Arc;

use crate::{
  oracle, sql_server, CompiledGraph, CompiledRelationalMapping, MappingIssues, QueryOperation,
  RelationalMapping, SqlCompileError, SqlStatement,
};

#[derive(Debug, Clone)]
pub struct MappedQueryGraph {
  graph: Arc<CompiledGraph>,
  mapping: CompiledRelationalMapping,
}

impl MappedQueryGraph {
  pub fn new(
    graph: impl Into<Arc<CompiledGraph>>,
    mapping: RelationalMapping,
  ) -> Result<Self, MappingIssues> {
    let graph = graph.into();
    let mapping = mapping.compile(graph.as_ref())?;
    Ok(Self { graph, mapping })
  }

  pub fn graph(&self) -> &CompiledGraph {
    self.graph.as_ref()
  }

  pub fn mapping(&self) -> &CompiledRelationalMapping {
    &self.mapping
  }

  pub fn compile_sql_server(
    &self,
    operation: &QueryOperation,
  ) -> Result<SqlStatement, SqlCompileError> {
    sql_server::compile(self.graph.as_ref(), &self.mapping, operation)
  }

  pub fn compile_oracle(
    &self,
    operation: &QueryOperation,
  ) -> Result<SqlStatement, SqlCompileError> {
    oracle::compile(self.graph.as_ref(), &self.mapping, operation)
  }
}
