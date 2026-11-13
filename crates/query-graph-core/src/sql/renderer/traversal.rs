use std::collections::BTreeSet;

use crate::traversal::TraversalRef;
use crate::{
  ConstraintCondition, Expression, PlanError, ReachableExpression, SourceDefinition, TraversalSide,
};

use super::{Renderer, SqlCompileError, SqlDialect, TraversalScope};

impl<D: SqlDialect> Renderer<'_, D> {
  pub(super) fn render_reachable(
    &mut self,
    traversal: &ReachableExpression,
  ) -> Result<String, SqlCompileError> {
    if !self.dialect.supports_traversal() {
      return Err(SqlCompileError::UnsupportedDialectFeature {
        dialect: self.dialect.name(),
        version: self.dialect.version(),
        feature: "reachable traversal",
      });
    }

    let prefix = format!("r{}_", self.traversal_count);
    self.traversal_count += 1;
    let query = self.render_traversal(traversal.into(), &prefix, None, &[], &[])?;

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

  fn render_traversal(
    &mut self,
    traversal: TraversalRef<'_>,
    prefix: &str,
    output: Option<&SourceDefinition>,
    root_fields: &[String],
    path_parameters: &[String],
  ) -> Result<String, SqlCompileError> {
    let graph = self.graph;
    self.graph = graph.traversal_input(traversal.scope);
    let previous_prefix = std::mem::replace(&mut self.alias_prefix, prefix.to_owned());
    let previous_root = self.root_alias.take();
    let previous_scope = self.traversal_scope.take();

    // Only the source context changes; bindings and expansion budgets remain statement-wide.
    let query =
      self.render_traversal_input(traversal, prefix, output, root_fields, path_parameters);
    self.graph = graph;
    self.alias_prefix = previous_prefix;
    self.root_alias = previous_root;
    self.traversal_scope = previous_scope;
    query
  }

  fn render_traversal_input(
    &mut self,
    traversal: TraversalRef<'_>,
    prefix: &str,
    output: Option<&SourceDefinition>,
    root_fields: &[String],
    path_parameters: &[String],
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
    let mut start = self.render_predicate(traversal.start)?;
    self.traversal_scope = Some(TraversalScope {
      scope: traversal.scope.to_owned(),
      source: root.clone(),
      alias: alias.clone(),
    });
    let mut step = self.render_predicate(traversal.step)?;
    let key = self.render_field(root, traversal.key)?;
    let source = self
      .dialect
      .render_table_reference(&derived(&eligible), &alias);
    let projection = if let Some(output) = output {
      let mut columns = Vec::new();
      for (index, field) in output.fields.iter().enumerate() {
        let value = self.render_field(root, &field.name)?;
        columns.push(format!(
          "{value} AS {}",
          self.dialect.quote_identifier(&format!("qg_field_{index}"))
        ));
        if root_fields.contains(&field.name) {
          columns.push(format!(
            "{} AS {}",
            self.dialect.render_traversal_root(&value)?,
            self.dialect.quote_identifier(&format!("qg_root_{index}"))
          ));
        }
      }
      columns.push(format!(
        "{} AS {}",
        self.dialect.render_traversal_level()?,
        self.dialect.quote_identifier("qg_level")
      ));
      if let Some(path) = output
        .traversal
        .as_ref()
        .and_then(|traversal| traversal.path.as_ref())
      {
        self.render_traversal_path(path, path_parameters, &mut start, &mut step)?;
      }
      columns.join(", ")
    } else {
      format!("{}.*", self.dialect.quote_identifier(&alias))
    };
    self
      .dialect
      .render_traversal_query(&projection, &source, &start, &step, &key)
  }

  pub(super) fn render_traversal_source(
    &mut self,
    source: &str,
  ) -> Result<String, SqlCompileError> {
    if !self.dialect.supports_traversal() {
      return Err(SqlCompileError::UnsupportedDialectFeature {
        dialect: self.dialect.name(),
        version: self.dialect.version(),
        feature: "traversal source",
      });
    }
    let graph = self.graph;
    let output = graph.source(source).expect("validated source");
    let traversal = output.traversal.as_deref().expect("traversal source");
    let mut root_fields = BTreeSet::new();
    let mut collect = |expression: &Expression| {
      expression.walk(&mut |node| {
        if let Expression::TraversalRoot {
          source: owner,
          field,
        } = node
        {
          if owner == source {
            root_fields.insert(field.clone());
          }
        }
        true
      })
    };
    for field in &graph.definition().projection.fields {
      collect(&field.expression);
    }
    for object in &graph.definition().projection.objects {
      collect(&object.presence);
    }
    for relation in &graph.definition().relations {
      collect(&relation.on);
      if let Some(selection) = &relation.selection {
        for order in selection.order_by() {
          collect(&order.expression);
        }
      }
    }
    for ordering in &graph.definition().orderings {
      for order in &ordering.order_by {
        collect(&order.expression);
      }
    }
    let mut parameters = BTreeSet::new();
    for constraint in &graph.definition().constraints {
      collect(&constraint.predicate);
      if self.in_exists {
        continue;
      }
      if matches!(&constraint.when, ConstraintCondition::ParameterPresent { parameter } if !self.operation.parameters.contains_key(parameter))
      {
        continue;
      }
      constraint.predicate.walk(&mut |node| {
        if let Expression::TraversalPathEquals {
          source: owner,
          parameter,
        } = node
        {
          if owner == source {
            parameters.insert(parameter.clone());
          }
        }
        true
      });
    }
    let prefix = format!("r{}_", self.traversal_count);
    self.traversal_count += 1;
    let query = self.render_traversal(
      traversal.into(),
      &prefix,
      Some(output),
      &root_fields.into_iter().collect::<Vec<_>>(),
      &parameters.into_iter().collect::<Vec<_>>(),
    )?;
    Ok(
      self
        .dialect
        .render_table_reference(&derived(&query), &self.source_alias(source)?),
    )
  }

  pub(super) fn render_traversal_column(
    &self,
    source: &str,
    field: Option<&str>,
  ) -> Result<String, SqlCompileError> {
    let column = match field {
      Some(field) => {
        let index = self
          .graph
          .field_index(source, field)
          .expect("validated root field");
        format!("qg_root_{index}")
      }
      None => "qg_level".to_owned(),
    };
    Ok(format!(
      "{}.{}",
      self.dialect.quote_identifier(&self.source_alias(source)?),
      self.dialect.quote_identifier(&column)
    ))
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
