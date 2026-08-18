use std::collections::HashSet;

use crate::{
  analysis::{DefinitionIndex, GraphTopology},
  ConstraintCondition, GraphDefinition, RelationCardinality, RelationSelection,
  GRAPH_DEFINITION_VERSION,
};

use super::{
  aggregation,
  expression::{self, ExpressionContext},
  projection, topology, DefinitionIssue, DefinitionIssueCode, DefinitionIssues,
};

pub(super) fn validate(
  definition: &GraphDefinition,
  index: &DefinitionIndex,
  graph_topology: &GraphTopology,
) -> Result<(), DefinitionIssues> {
  DefinitionValidator::new(definition, index, graph_topology).validate()
}

struct DefinitionValidator<'a> {
  definition: &'a GraphDefinition,
  index: &'a DefinitionIndex,
  graph_topology: &'a GraphTopology,
  issues: Vec<DefinitionIssue>,
}

impl<'a> DefinitionValidator<'a> {
  fn new(
    definition: &'a GraphDefinition,
    index: &'a DefinitionIndex,
    graph_topology: &'a GraphTopology,
  ) -> Self {
    Self {
      definition,
      index,
      graph_topology,
      issues: Vec::new(),
    }
  }

  fn validate(mut self) -> Result<(), DefinitionIssues> {
    self.validate_header();
    self.validate_sources();
    self.validate_root();
    self.validate_parameters();
    self.validate_relations();
    self.validate_constraints();
    self.validate_projection();
    self.validate_orderings();
    aggregation::validate(self.definition, &mut self.issues);
    topology::validate(
      self.definition,
      self.index,
      self.graph_topology,
      &mut self.issues,
    );

    if self.issues.is_empty() {
      Ok(())
    } else {
      Err(DefinitionIssues::from_vec(self.issues))
    }
  }

  fn validate_header(&mut self) {
    if self.definition.schema_version != GRAPH_DEFINITION_VERSION {
      self.issues.push(DefinitionIssue::new(
        DefinitionIssueCode::UnsupportedVersion,
        "schemaVersion",
        format!(
          "expected version {}, received {}",
          GRAPH_DEFINITION_VERSION, self.definition.schema_version
        ),
      ));
    }

    if self.definition.name.trim().is_empty() {
      self.issues.push(DefinitionIssue::new(
        DefinitionIssueCode::EmptyName,
        "name",
        "graph name must not be empty",
      ));
    }
  }

  fn validate_sources(&mut self) {
    let mut source_names = HashSet::new();
    for (source_index, source) in self.definition.sources.iter().enumerate() {
      let source_location = format!("sources[{source_index}]");
      if source.key.trim().is_empty() {
        self.issues.push(DefinitionIssue::new(
          DefinitionIssueCode::EmptySourceKey,
          format!("{source_location}.key"),
          "source key must not be empty",
        ));
        continue;
      }

      if !source_names.insert(source.key.clone()) {
        self.issues.push(DefinitionIssue::new(
          DefinitionIssueCode::DuplicateSource,
          format!("{source_location}.key"),
          format!("source {:?} is defined more than once", source.key),
        ));
        continue;
      }

      self.validate_source_fields(source_index);
    }
  }

  fn validate_source_fields(&mut self, source_index: usize) {
    let source = &self.definition.sources[source_index];
    let mut fields = HashSet::new();

    for (field_index, field) in source.fields.iter().enumerate() {
      let location = format!("sources[{source_index}].fields[{field_index}].name");
      if field.name.trim().is_empty() {
        self.issues.push(DefinitionIssue::new(
          DefinitionIssueCode::EmptyFieldName,
          location,
          "field name must not be empty",
        ));
        continue;
      }

      if !fields.insert(field.name.clone()) {
        self.issues.push(DefinitionIssue::new(
          DefinitionIssueCode::DuplicateField,
          location,
          format!(
            "field {:?} is defined more than once in source {:?}",
            field.name, source.key
          ),
        ));
      }
    }
  }

  fn validate_root(&mut self) {
    if self.index.root().is_none() {
      self.issues.push(DefinitionIssue::new(
        DefinitionIssueCode::UnknownRoot,
        "root",
        format!("root source {:?} is not defined", self.definition.root),
      ));
    }
  }

  fn validate_parameters(&mut self) {
    let mut parameter_names = HashSet::new();
    for (parameter_index, parameter) in self.definition.parameters.iter().enumerate() {
      let location = format!("parameters[{parameter_index}].name");
      if parameter.name.trim().is_empty() {
        self.issues.push(DefinitionIssue::new(
          DefinitionIssueCode::EmptyParameterName,
          location,
          "parameter name must not be empty",
        ));
        continue;
      }

      if !parameter_names.insert(parameter.name.clone()) {
        self.issues.push(DefinitionIssue::new(
          DefinitionIssueCode::DuplicateParameter,
          location,
          format!("parameter {:?} is defined more than once", parameter.name),
        ));
      }
    }
  }

  fn validate_relations(&mut self) {
    let mut names = HashSet::new();

    for (relation_index, relation) in self.definition.relations.iter().enumerate() {
      let location = format!("relations[{relation_index}]");
      self.validate_relation_name(relation_index, &mut names);

      if self.index.source(&relation.from).is_none() {
        self.issues.push(DefinitionIssue::new(
          DefinitionIssueCode::UnknownRelationSource,
          format!("{location}.from"),
          format!("relation source {:?} is not defined", relation.from),
        ));
      }

      if self.index.source(&relation.to).is_none() {
        self.issues.push(DefinitionIssue::new(
          DefinitionIssueCode::UnknownRelationTarget,
          format!("{location}.to"),
          format!("relation target {:?} is not defined", relation.to),
        ));
      }

      let allowed_sources = HashSet::from([relation.from.clone(), relation.to.clone()]);
      let context = ExpressionContext::relation_predicate(
        self.definition,
        self.index,
        self.graph_topology,
        &allowed_sources,
      );
      expression::validate(
        &relation.on,
        &format!("{location}.on"),
        &context,
        &mut self.issues,
      );
      self.validate_relation_selection(relation_index);
    }
  }

  fn validate_relation_name(&mut self, relation_index: usize, names: &mut HashSet<String>) {
    let relation = &self.definition.relations[relation_index];
    let location = format!("relations[{relation_index}].name");

    if relation.name.trim().is_empty() {
      self.issues.push(DefinitionIssue::new(
        DefinitionIssueCode::EmptyRelationName,
        location,
        "relation name must not be empty",
      ));
    } else if !names.insert(relation.name.clone()) {
      self.issues.push(DefinitionIssue::new(
        DefinitionIssueCode::DuplicateRelation,
        location,
        format!("relation {:?} is defined more than once", relation.name),
      ));
    }
  }

  fn validate_relation_selection(&mut self, relation_index: usize) {
    let relation = &self.definition.relations[relation_index];
    let Some(selection) = &relation.selection else {
      return;
    };
    let location = format!("relations[{relation_index}].selection");

    if relation.cardinality != RelationCardinality::One {
      self.issues.push(DefinitionIssue::new(
        DefinitionIssueCode::InvalidRelationSelection,
        &location,
        "relation selection is valid only for cardinality one",
      ));
    }

    match selection {
      RelationSelection::FirstBy { order_by } => {
        if order_by.is_empty() {
          self.issues.push(DefinitionIssue::new(
            DefinitionIssueCode::EmptyRelationSelectionOrder,
            format!("{location}.orderBy"),
            "firstBy selection must contain at least one order expression",
          ));
        }

        let allowed_sources = HashSet::from([relation.to.clone()]);
        let context = ExpressionContext::scoped(
          self.definition,
          self.index,
          self.graph_topology,
          &allowed_sources,
          DefinitionIssueCode::RelationSelectionExpressionScope,
        );
        for (order_index, order) in order_by.iter().enumerate() {
          expression::validate(
            &order.expression,
            &format!("{location}.orderBy[{order_index}].expression"),
            &context,
            &mut self.issues,
          );
        }
      }
    }
  }

  fn validate_constraints(&mut self) {
    for (constraint_index, constraint) in self.definition.constraints.iter().enumerate() {
      let location = format!("constraints[{constraint_index}]");

      if let ConstraintCondition::ParameterPresent { parameter } = &constraint.when {
        if self.index.parameter(parameter).is_none() {
          self.issues.push(DefinitionIssue::new(
            DefinitionIssueCode::UnknownParameter,
            format!("{location}.when.parameter"),
            format!("parameter {parameter:?} is not defined"),
          ));
        }
      }

      let context = ExpressionContext::constraint(self.definition, self.index, self.graph_topology);
      expression::validate(
        &constraint.predicate,
        &format!("{location}.predicate"),
        &context,
        &mut self.issues,
      );
    }
  }

  fn validate_projection(&mut self) {
    let mut paths: HashSet<Vec<String>> = HashSet::new();

    for (field_index, field) in self.definition.projection.fields.iter().enumerate() {
      let location = format!("projection.fields[{field_index}]");
      Self::validate_projection_path(&field.path, &format!("{location}.path"), &mut self.issues);

      let conflicting_path = paths
        .iter()
        .find(|existing| projection::paths_conflict(existing, &field.path))
        .cloned();

      if !paths.insert(field.path.clone()) {
        self.issues.push(DefinitionIssue::new(
          DefinitionIssueCode::DuplicateProjectionPath,
          format!("{location}.path"),
          format!("projection path {:?} is defined more than once", field.path),
        ));
      } else if let Some(conflicting_path) = conflicting_path {
        self.issues.push(DefinitionIssue::new(
          DefinitionIssueCode::ConflictingProjectionPath,
          format!("{location}.path"),
          format!(
            "projection path {:?} conflicts with {:?}",
            field.path, conflicting_path
          ),
        ));
      }

      let context =
        ExpressionContext::unrestricted(self.definition, self.index, self.graph_topology);
      expression::validate(
        &field.expression,
        &format!("{location}.expression"),
        &context,
        &mut self.issues,
      );
      projection::validate_visibility(
        self.definition,
        self.index,
        &field.expression,
        &format!("{location}.expression"),
        &mut self.issues,
      );
    }

    if self.definition.is_summary() && !self.definition.projection.objects.is_empty() {
      self.issues.push(DefinitionIssue::new(
        DefinitionIssueCode::ProjectionObjectInSummary,
        "projection.objects",
        "summary graph cannot define projection objects",
      ));
    }

    let mut object_paths = HashSet::new();
    for (object_index, object) in self.definition.projection.objects.iter().enumerate() {
      let location = format!("projection.objects[{object_index}]");
      Self::validate_projection_path(&object.path, &format!("{location}.path"), &mut self.issues);

      if !object_paths.insert(object.path.clone()) {
        self.issues.push(DefinitionIssue::new(
          DefinitionIssueCode::DuplicateProjectionObjectPath,
          format!("{location}.path"),
          format!(
            "projection object path {:?} is defined more than once",
            object.path
          ),
        ));
      }

      let has_descendant =
        self.definition.projection.fields.iter().any(|field| {
          field.path.len() > object.path.len() && field.path.starts_with(&object.path)
        });
      if !has_descendant {
        self.issues.push(DefinitionIssue::new(
          DefinitionIssueCode::ProjectionObjectWithoutFields,
          format!("{location}.path"),
          format!(
            "projection object path {:?} has no descendant projection fields",
            object.path
          ),
        ));
      }

      let context =
        ExpressionContext::unrestricted(self.definition, self.index, self.graph_topology);
      expression::validate(
        &object.presence,
        &format!("{location}.presence"),
        &context,
        &mut self.issues,
      );
    }
  }

  fn validate_projection_path(path: &[String], location: &str, issues: &mut Vec<DefinitionIssue>) {
    if path.is_empty() {
      issues.push(DefinitionIssue::new(
        DefinitionIssueCode::EmptyProjectionPath,
        location,
        "projection path must contain at least one segment",
      ));
      return;
    }

    for (segment_index, segment) in path.iter().enumerate() {
      let segment_location = format!("{location}[{segment_index}]");
      if segment.trim().is_empty() {
        issues.push(DefinitionIssue::new(
          DefinitionIssueCode::EmptyProjectionPathSegment,
          segment_location,
          "projection path segment must not be empty",
        ));
      } else if segment.contains('.') {
        issues.push(DefinitionIssue::new(
          DefinitionIssueCode::InvalidProjectionPathSegment,
          segment_location,
          "projection path segment must not contain '.'",
        ));
      }
    }
  }

  fn validate_orderings(&mut self) {
    let mut names = HashSet::new();
    let mut default_count = 0;

    for (ordering_index, ordering) in self.definition.orderings.iter().enumerate() {
      let location = format!("orderings[{ordering_index}]");

      if ordering.name.trim().is_empty() {
        self.issues.push(DefinitionIssue::new(
          DefinitionIssueCode::EmptyOrderingName,
          format!("{location}.name"),
          "ordering name must not be empty",
        ));
      } else if !names.insert(ordering.name.clone()) {
        self.issues.push(DefinitionIssue::new(
          DefinitionIssueCode::DuplicateOrdering,
          format!("{location}.name"),
          format!("ordering {:?} is defined more than once", ordering.name),
        ));
      }

      if ordering.selected_by_default {
        default_count += 1;
      }

      if ordering.order_by.is_empty() {
        self.issues.push(DefinitionIssue::new(
          DefinitionIssueCode::EmptyOrdering,
          format!("{location}.orderBy"),
          "ordering must contain at least one order item",
        ));
      }

      for (order_index, order) in ordering.order_by.iter().enumerate() {
        let context =
          ExpressionContext::unrestricted(self.definition, self.index, self.graph_topology);
        expression::validate(
          &order.expression,
          &format!("{location}.orderBy[{order_index}].expression"),
          &context,
          &mut self.issues,
        );
      }
    }

    if default_count > 1 {
      self.issues.push(DefinitionIssue::new(
        DefinitionIssueCode::MultipleDefaultOrderings,
        "orderings",
        "only one ordering can be selected by default",
      ));
    }
  }
}
