mod compiled_plan;
mod model;
mod partition;
mod validation;

use crate::{MappedQueryGraph, OracleCompiler, QueryOperation, SqlServerCompiler};

pub use compiled_plan::CompiledQueryPlan;
pub use model::{
  BatchPlanMetadata, BatchRelationDefinition, ComposedCompileError, CompositionIssue,
  CompositionIssueCode, CompositionIssues,
};

use compiled_plan::{CompiledBatchStep, PlanCompiler};
use model::BatchRelation;
use partition::partition_operation;

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
    validation::validate_relation(&self.root, &self.relations, &graph, &definition)?;
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

  fn compile_plan(
    &self,
    operation: &QueryOperation,
    compiler: PlanCompiler,
  ) -> Result<CompiledQueryPlan, ComposedCompileError> {
    let partitioned = partition_operation(&self.root, &self.relations, operation)?;
    let root = compiler.compile(&self.root, &partitioned.root)?;
    let batches = partitioned
      .batches
      .into_iter()
      .map(|batch| {
        let relation = &self.relations[batch.relation_index];
        CompiledBatchStep {
          metadata: batch.metadata,
          graph: relation.graph.clone(),
          parameter: relation.definition.parameter.clone(),
          operation: batch.operation,
        }
      })
      .collect();

    Ok(CompiledQueryPlan::new(root, batches, compiler))
  }
}
