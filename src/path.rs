#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ProjectionPath(Box<[String]>);

impl ProjectionPath {
  pub(crate) fn from_segments(segments: &[String]) -> Self {
    Self(segments.to_vec().into_boxed_slice())
  }

  pub(crate) fn parse(path: &str) -> Self {
    Self(
      path
        .split('.')
        .map(str::to_owned)
        .collect::<Vec<_>>()
        .into_boxed_slice(),
    )
  }
}
