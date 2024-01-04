pub enum QueryFilter {
	Model(String),
	CommitId(String),
	Last(i32),
	First(i32),
	UserAddress(String),
	Timestamp(),
}

trait Queryable {
	fn queryStream<T>(&self, filter: Vec<QueryFilter>, input: T) -> Vec<String>;
}
