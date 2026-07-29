use std::collections::{HashMap, HashSet};

use crate::{
  ConstraintCondition, GraphDefinition, ParameterShape, RelationCardinality, RelationSelection,
  GRAPH_DEFINITION_VERSION,
};

use super::{
  aggregation,
  expression::{self, ExpressionContext},
  projection, topology, DefinitionIssue, DefinitionIssueCode, DefinitionIssues,
};

type SourceFields = HashMap<String, HashSet<String>>;

pub(super) fn validate(definition: &GraphDefinition) -> Result<(), DefinitionIssues> {
  DefinitionValidator::new(definition).validate()
}

struct DefinitionValidator<'a> {
  definition: &'a GraphDefinition,
  issues: Vec<DefinitionIssue>,
  sources: SourceFields,
  parameters: HashMap<String, ParameterShape>,
  source_scopes: HashMap<String, HashSet<String>>,
}

impl<'a> DefinitionValidator<'a> {
  fn new(definition: &'a GraphDefinition) -> Self {
    Self {
      definition,
      issues: Vec::new(),
      sources: HashMap::new(),
      parameters: HashMap::new(),
      source_scopes: HashMap::new(),
    }
  }

  fn validate(mut self) -> Result<(), DefinitionIssues> {
    self.validate_header();
    self.index_sources();
    self.validate_root();
    self.index_parameters();
    self.validate_relations();
    self.source_scopes = topology::infer_source_scopes(self.definition, &self.sources);
    self.validate_constraints();
    self.validate_projection();
    self.validate_orderings();
    aggregation::validate(self.definition, &mut self.issues);
    topology::validate(self.definition, &self.sources, &mut self.issues);

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

  fn index_sources(&mut self) {
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

      if self.sources.contains_key(&source.key) {
        self.issues.push(DefinitionIssue::new(
          DefinitionIssueCode::DuplicateSource,
          format!("{source_location}.key"),
          format!("source {:?} is defined more than once", source.key),
        ));
        continue;
      }

      let fields = self.validate_source_fields(source_index);
      self.sources.insert(source.key.clone(), fields);
    }
  }

  fn validate_source_fields(&mut self, source_index: usize) -> HashSet<String> {
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

    fields
  }

  fn validate_root(&mut self) {
    if !self.sources.contains_key(&self.definition.root) {
      self.issues.push(DefinitionIssue::new(
        DefinitionIssueCode::UnknownRoot,
        "root",
        format!("root source {:?} is not defined", self.definition.root),
      ));
    }
  }

  fn index_parameters(&mut self) {
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

      if self
        .parameters
        .insert(parameter.name.clone(), parameter.shape)
        .is_some()
      {
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

      if !self.sources.contains_key(&relation.from) {
        self.issues.push(DefinitionIssue::new(
          DefinitionIssueCode::UnknownRelationSource,
          format!("{location}.from"),
          format!("relation source {:?} is not defined", relation.from),
        ));
      }

      if !self.sources.contains_key(&relation.to) {
        self.issues.push(DefinitionIssue::new(
          DefinitionIssueCode::UnknownRelationTarget,
          format!("{location}.to"),
          format!("relation target {:?} is not defined", relation.to),
        ));
      }

      let allowed_sources = HashSet::from([relation.from.clone(), relation.to.clone()]);
      let context = ExpressionContext::scoped(
        &self.sources,
        &self.parameters,
        &self.source_scopes,
        &allowed_sources,
        DefinitionIssueCode::RelationExpressionScope,
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
          &self.sources,
          &self.parameters,
          &self.source_scopes,
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
        if !self.parameters.contains_key(parameter) {
          self.issues.push(DefinitionIssue::new(
            DefinitionIssueCode::UnknownParameter,
            format!("{location}.when.parameter"),
            format!("parameter {parameter:?} is not defined"),
          ));
        }
      }

      let context =
        ExpressionContext::constraint(&self.sources, &self.parameters, &self.source_scopes);
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
      self.validate_projection_path(field_index);

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
        ExpressionContext::unrestricted(&self.sources, &self.parameters, &self.source_scopes);
      expression::validate(
        &field.expression,
        &format!("{location}.expression"),
        &context,
        &mut self.issues,
      );
      projection::validate_visibility(self.definition, field, &location, &mut self.issues);
    }
  }

  fn validate_projection_path(&mut self, field_index: usize) {
    let field = &self.definition.projection.fields[field_index];
    let location = format!("projection.fields[{field_index}].path");

    if field.path.is_empty() {
      self.issues.push(DefinitionIssue::new(
        DefinitionIssueCode::EmptyProjectionPath,
        location,
        "projection path must contain at least one segment",
      ));
      return;
    }

    for (segment_index, segment) in field.path.iter().enumerate() {
      let segment_location = format!("projection.fields[{field_index}].path[{segment_index}]");
      if segment.trim().is_empty() {
        self.issues.push(DefinitionIssue::new(
          DefinitionIssueCode::EmptyProjectionPathSegment,
          segment_location,
          "projection path segment must not be empty",
        ));
      } else if segment.contains('.') {
        self.issues.push(DefinitionIssue::new(
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
          ExpressionContext::unrestricted(&self.sources, &self.parameters, &self.source_scopes);
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
