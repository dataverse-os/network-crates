use serde::Serialize;
use std::collections::HashMap;

/// Valid values for operation Filter
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum NumberFilter {
    /// I64 Value
    I64(i64),
    /// I32 Value
    I32(i32),
    /// F32 Value
    F32(f32),
    /// F64 Value
    F64(f64),
}

impl From<i64> for NumberFilter {
    fn from(value: i64) -> Self {
        Self::I64(value)
    }
}

impl From<i32> for NumberFilter {
    fn from(value: i32) -> Self {
        Self::I32(value)
    }
}

impl From<f64> for NumberFilter {
    fn from(value: f64) -> Self {
        Self::F64(value)
    }
}

impl From<f32> for NumberFilter {
    fn from(value: f32) -> Self {
        Self::F32(value)
    }
}

/// Valid values for operation Filter
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum ValueFilter {
    /// String value
    String(String),
    /// Number value
    Number(NumberFilter),
}

impl From<&str> for ValueFilter {
    fn from(value: &str) -> Self {
        Self::from(value.to_string())
    }
}

impl From<String> for ValueFilter {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<i64> for ValueFilter {
    fn from(value: i64) -> Self {
        Self::Number(value.into())
    }
}

impl From<i32> for ValueFilter {
    fn from(value: i32) -> Self {
        Self::Number(value.into())
    }
}

impl From<f64> for ValueFilter {
    fn from(value: f64) -> Self {
        Self::Number(value.into())
    }
}

impl From<f32> for ValueFilter {
    fn from(value: f32) -> Self {
        Self::Number(value.into())
    }
}

/// Valid values for operation Filter
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum EqualValueFilter {
    /// Boolean value
    Boolean(bool),
    /// Number value
    Value(ValueFilter),
}

impl From<bool> for EqualValueFilter {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

impl From<&str> for EqualValueFilter {
    fn from(value: &str) -> Self {
        Self::from(value.to_string())
    }
}

impl From<String> for EqualValueFilter {
    fn from(value: String) -> Self {
        Self::Value(value.into())
    }
}

impl From<i64> for EqualValueFilter {
    fn from(value: i64) -> Self {
        Self::Value(value.into())
    }
}

impl From<i32> for EqualValueFilter {
    fn from(value: i32) -> Self {
        Self::Value(value.into())
    }
}

impl From<f64> for EqualValueFilter {
    fn from(value: f64) -> Self {
        Self::Value(value.into())
    }
}

impl From<f32> for EqualValueFilter {
    fn from(value: f32) -> Self {
        Self::Value(value.into())
    }
}

/// Operation Filter
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationFilter {
    /// Filter by null or not null
    IsNull(bool),
    /// Filter by equal to
    EqualTo(EqualValueFilter),
    /// Filter by not equal to
    NotEqualTo(EqualValueFilter),
    /// Filter against an array of values
    In(Vec<ValueFilter>),
    /// Filter against an array of values
    NotIn(Vec<ValueFilter>),
    /// Filter by less than
    LessThan(NumberFilter),
    /// Filter by less than or equal to
    LessThanOrEqualTo(NumberFilter),
    /// Filter by greater than
    GreaterThan(NumberFilter),
    /// Filter by greater than or equal to
    GreaterThanOrEqualTo(NumberFilter),
}

/// Combination query
#[derive(Clone, Debug, Serialize)]
pub struct CombinationQuery(Vec<FilterQuery>);

impl CombinationQuery {
    /// Create a new combination query, consisting of at least 2 filters
    pub fn new(a: FilterQuery, b: FilterQuery, rest: Vec<FilterQuery>) -> Self {
        Self(vec![a, b].into_iter().chain(rest).collect())
    }
}

/// Create an 'and' query
#[macro_export]
macro_rules! and {
    ($a:expr, $b:expr, $($x:expr),*) => {
        FilterQuery::And(CombinationQuery::new($a, $b, vec![$($x),*]))
    };
}

/// Create an 'or' query
#[macro_export]
macro_rules! or {
    ($a:expr, $b:expr, $($x:expr),*) => {
        FilterQuery::Or(CombinationQuery::new($a, $b, vec![$($x),*]))
    };
}

/// Filter Query
#[derive(Clone, Debug, Serialize)]
pub enum FilterQuery {
    /// Filter by where
    #[serde(rename = "where")]
    Where(HashMap<String, OperationFilter>),
    /// Filter by and
    #[serde(rename = "and")]
    And(CombinationQuery),
    /// Filter by or
    #[serde(rename = "or")]
    Or(CombinationQuery),
    /// Filter by not
    #[serde(rename = "not")]
    Not(Box<FilterQuery>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_serializer_where() {
        let mut where_filter = HashMap::new();
        where_filter.insert(
            "id".to_string(),
            OperationFilter::EqualTo("1".to_string().into()),
        );
        let filter = FilterQuery::Where(where_filter);
        let serialized = serde_json::to_string(&filter).unwrap();
        assert_eq!(serialized, r#"{"where":{"id":{"equalTo":"1"}}}"#);
    }

    #[test]
    fn should_serialize_and() {
        let mut where_filter1 = HashMap::new();
        where_filter1.insert("id".to_string(), OperationFilter::LessThan(1i64.into()));
        let mut where_filter2 = HashMap::new();
        where_filter2.insert(
            "id2".to_string(),
            OperationFilter::GreaterThanOrEqualTo(2i32.into()),
        );
        let filter = and!(
            FilterQuery::Where(where_filter1),
            FilterQuery::Where(where_filter2),
        );
        let serialized = serde_json::to_string(&filter).unwrap();
        assert_eq!(
            serialized,
            r#"{"and":[{"where":{"id":{"lessThan":1}}},{"where":{"id2":{"greaterThanOrEqualTo":2}}}]}"#
        );
    }

    #[test]
    fn should_serialize_or() {
        let mut where_filter1 = HashMap::new();
        where_filter1.insert("id".to_string(), OperationFilter::GreaterThan(1i64.into()));
        let mut where_filter2 = HashMap::new();
        where_filter2.insert(
            "id2".to_string(),
            OperationFilter::LessThanOrEqualTo(2i32.into()),
        );
        let filter = or!(
            FilterQuery::Where(where_filter1),
            FilterQuery::Where(where_filter2),
        );
        let serialized = serde_json::to_string(&filter).unwrap();
        assert_eq!(
            serialized,
            r#"{"or":[{"where":{"id":{"greaterThan":1}}},{"where":{"id2":{"lessThanOrEqualTo":2}}}]}"#
        );
    }

    #[test]
    fn should_serialize_in() {
        let mut where_filter = HashMap::new();
        where_filter.insert(
            "id".to_string(),
            OperationFilter::In(vec!["a".into(), "b".into()]),
        );
        let filter = FilterQuery::Not(Box::new(FilterQuery::Where(where_filter)));
        let serialized = serde_json::to_string(&filter).unwrap();
        assert_eq!(serialized, r#"{"not":{"where":{"id":{"in":["a","b"]}}}}"#);
    }

    #[test]
    fn should_serialize_nested() {
        let mut where_filter1 = HashMap::new();
        where_filter1.insert("id".to_string(), OperationFilter::IsNull(false));
        let mut where_filter2 = HashMap::new();
        where_filter2.insert("id2".to_string(), OperationFilter::NotEqualTo(2i32.into()));
        let mut where_filter3 = HashMap::new();
        where_filter3.insert(
            "id3".to_string(),
            OperationFilter::NotIn(vec![3f32.into(), 4f32.into()]),
        );
        let filter = and!(
            or!(
                FilterQuery::Not(Box::new(FilterQuery::Where(where_filter1.clone()))),
                FilterQuery::Where(where_filter1),
            ),
            or!(
                FilterQuery::Not(Box::new(FilterQuery::Where(where_filter2.clone()))),
                FilterQuery::Where(where_filter2),
            ),
            or!(
                FilterQuery::Not(Box::new(FilterQuery::Where(where_filter3.clone()))),
                FilterQuery::Where(where_filter3),
            )
        );
        let serialized = serde_json::to_string(&filter).unwrap();
        assert_eq!(
            serialized,
            r#"{"and":[{"or":[{"not":{"where":{"id":{"isNull":false}}}},{"where":{"id":{"isNull":false}}}]},{"or":[{"not":{"where":{"id2":{"notEqualTo":2}}}},{"where":{"id2":{"notEqualTo":2}}}]},{"or":[{"not":{"where":{"id3":{"notIn":[3.0,4.0]}}}},{"where":{"id3":{"notIn":[3.0,4.0]}}}]}]}"#
        );
    }
}
