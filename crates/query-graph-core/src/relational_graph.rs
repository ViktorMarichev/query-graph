use std::sync::Arc;

use crate::{
  CompiledGraph, CompiledRelationalMapping, MappingIssues, OracleCompiler, QueryOperation,
  RelationalMapping, SqlCompileError, SqlServerCompiler, SqlStatement,
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
    self.compile_sql_server_with(operation, &SqlServerCompiler::default())
  }

  pub fn compile_sql_server_with(
    &self,
    operation: &QueryOperation,
    compiler: &SqlServerCompiler,
  ) -> Result<SqlStatement, SqlCompileError> {
    compiler.compile(self.graph.as_ref(), &self.mapping, operation)
  }

  pub fn compile_oracle(
    &self,
    operation: &QueryOperation,
  ) -> Result<SqlStatement, SqlCompileError> {
    self.compile_oracle_with(operation, &OracleCompiler::default())
  }

  pub fn compile_oracle_with(
    &self,
    operation: &QueryOperation,
    compiler: &OracleCompiler,
  ) -> Result<SqlStatement, SqlCompileError> {
    compiler.compile(self.graph.as_ref(), &self.mapping, operation)
  }
}
