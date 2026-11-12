use crate::{ConstraintCondition, PlanError, ReachableExpression, TraversalSide};

use super::{Renderer, SqlCompileError, SqlDialect, TraversalScope};

impl<D: SqlDialect> Renderer<'_, D> {
  pub(super) fn render_reachable(
    &mut self,
    traversal: &ReachableExpression,
  ) -> Result<String, SqlCompileError> {
    if !self.dialect.supports_reachable() {
      return Err(SqlCompileError::UnsupportedDialectFeature {
        dialect: self.dialect.name(),
        version: self.dialect.version(),
        feature: "reachable traversal",
      });
    }

    let prefix = format!("r{}_", self.traversal_count);
    self.traversal_count += 1;
    let graph = self.graph;
    self.graph = graph.traversal_input(&traversal.scope);
    let previous_prefix = std::mem::replace(&mut self.alias_prefix, prefix.clone());
    let previous_root = self.root_alias.take();
    let previous_scope = self.traversal_scope.take();

    // Only the source context changes; bindings and expansion budgets remain statement-wide.
    let query = self.render_reachable_input(traversal, &prefix);
    self.graph = graph;
    self.alias_prefix = previous_prefix;
    self.root_alias = previous_root;
    self.traversal_scope = previous_scope;
    let query = query?;

    let alias = format!("{prefix}match");
    let previous_scope = self.traversal_scope.replace(TraversalScope {
      scope: traversal.scope.clone(),
      source: traversal.input.root.clone(),
      alias: alias.clone(),
    });
    let predicate = self.render_predicate(&traversal.predicate);
    self.traversal_scope = previous_scope;
    let source = self
      .dialect
      .render_table_reference(&derived(&query), &alias);
    Ok(format!(
      "EXISTS (\n  SELECT 1\n  FROM {source}\n  WHERE {}\n)",
      predicate?
    ))
  }

  fn render_reachable_input(
    &mut self,
    traversal: &ReachableExpression,
    prefix: &str,
  ) -> Result<String, SqlCompileError> {
    let graph = self.graph;
    let root = &graph.root().key;
    let projection = format!(
      "{}.*",
      self.dialect.quote_identifier(&self.source_alias(root)?)
    );
    let mut eligible = format!("SELECT {projection}\nFROM {}", self.render_source(root)?);
    let mut predicates = Vec::new();
    for constraint in &graph.definition().constraints {
      let active = match &constraint.when {
        ConstraintCondition::Always => true,
        ConstraintCondition::ParameterPresent { parameter } => {
          self.operation.parameters.contains_key(parameter)
        }
      };
      if active {
        predicates.push(self.render_predicate(&constraint.predicate)?);
      }
    }
    if !predicates.is_empty() {
      eligible.push_str(&format!("\nWHERE {}", predicates.join("\n  AND ")));
    }

    let alias = format!("{prefix}walk");
    self.root_alias = Some(alias.clone());
    let start = self.render_predicate(&traversal.start)?;
    self.traversal_scope = Some(TraversalScope {
      scope: traversal.scope.clone(),
      source: root.clone(),
      alias: alias.clone(),
    });
    let step = self.render_predicate(&traversal.step)?;
    let key = self.render_field(root, &traversal.key)?;
    let source = self
      .dialect
      .render_table_reference(&derived(&eligible), &alias);
    self.dialect.render_reachable_query(
      &format!("{}.*", self.dialect.quote_identifier(&alias)),
      &source,
      &start,
      &step,
      &key,
    )
  }

  pub(super) fn render_traversal_field(
    &self,
    scope: &str,
    side: TraversalSide,
    field: &str,
  ) -> Result<String, SqlCompileError> {
    let context = self
      .traversal_scope
      .as_ref()
      .filter(|context| context.scope == scope)
      .ok_or_else(|| {
        SqlCompileError::Plan(PlanError::InvalidCompiledGraph {
          message: "traversal field is outside its scope".to_owned(),
        })
      })?;
    let column = self
      .mapping
      .column(&context.source, field)
      .ok_or_else(|| SqlCompileError::MissingSourceMapping(context.source.clone()))?;
    let prior = if side == TraversalSide::Current {
      "PRIOR "
    } else {
      ""
    };
    Ok(format!(
      "{prior}{}.{}",
      self.dialect.quote_identifier(&context.alias),
      self.dialect.quote_identifier(column)
    ))
  }
}

fn derived(query: &str) -> String {
  format!(
    "(\n{}\n)",
    query
      .lines()
      .map(|line| format!("  {line}"))
      .collect::<Vec<_>>()
      .join("\n")
  )
}
