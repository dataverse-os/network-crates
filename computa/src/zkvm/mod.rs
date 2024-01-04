pub mod types;

pub use computa_derive::*;
pub use types::*;

use std::io::Read;

use anyhow::Context;
use ethabi::{Address, ParamType, Token};
use risc0_zkvm::guest::env;

#[derive(Debug)]
pub struct Input<T>
where
	T: Query,
{
	pub query_id: Vec<u8>,
	pub query_data: Vec<u8>,
	pub query: T,
	pub payload: Vec<u8>,
}

pub fn get_input<T>() -> anyhow::Result<Input<T>>
where
	T: Query,
{
	let mut input_bytes = Vec::<u8>::new();
	env::stdin()
		.read_to_end(&mut input_bytes)
		.context("failed to read input bytes")?;

	let (query_id, query_data, payload) = decode_query_input(input_bytes)?;
	let query = decode_query_data(query_data.clone())?;

	Ok(Input {
		query_id,
		query_data,
		query,
		payload,
	})
}

pub fn decode_query_input(input_bytes: Vec<u8>) -> anyhow::Result<(Vec<u8>, Vec<u8>, Vec<u8>)> {
	let types = [
		ParamType::FixedBytes(32),
		ParamType::Bytes,
		ParamType::Bytes,
	];
	let tokens = ethabi::decode(&types, &input_bytes).map_err(|err| {
		anyhow::anyhow!(
			"failed to decode input bytes: {:?}, error: {:?}",
			input_bytes,
			err
		)
	})?;

	if let (Some(query_id), Some(query_data), Some(validation_data)) = (
		tokens[0].clone().into_fixed_bytes(),
		tokens[1].clone().into_bytes(),
		tokens[2].clone().into_bytes(),
	) {
		return Ok((query_id, query_data, validation_data));
	};

	anyhow::bail!("failed to decode input bytes: {:?}", input_bytes)
}

pub trait Query: TryFrom<Vec<Token>> {
	fn types() -> Vec<ParamType>;
}

impl From<Vec<Token>> for FileQuery {
	fn from(value: Vec<Token>) -> Self {
		Self {
			owner: value[0].clone().into_address().unwrap(),
			file_id: value[1].clone().into_string().unwrap(),
			commit_id: value[2].clone().into_string().unwrap(),
		}
	}
}

pub fn decode_query_data<T>(data: Vec<u8>) -> anyhow::Result<T>
where
	T: Query,
{
	let tokens = ethabi::decode(&T::types(), &data).map_err(|err| {
		anyhow::anyhow!(
			"failed to decode query data bytes: {:?}, error: {:?}",
			data,
			err
		)
	})?;

	tokens
		.try_into()
		.map_err(|_err| anyhow::anyhow!("failed to convert query data tokens to query data"))

	// anyhow::bail!("failed to decode query data bytes: {:?}", data)
}

#[derive(Debug)]
pub struct FileQuery {
	pub owner: Address,
	pub file_id: String,
	pub commit_id: String,
}

#[macro_export]
macro_rules! handle_with_entry {
	($name:ident, $input:ident) => {
		risc0_zkvm::guest::entry!(main);
		fn main() {
			let input = get_input::<$input>().expect("failed to get input");
			let data = input.query;
			let result = $name(data);
			risc0_zkvm::guest::env::commit_slice(&ethabi::encode(&result.tokens()));
		}
	};
}
