use dataverse_computa::{handle_with_entry, zkvm::*};
use ethabi::{ParamType, Token};

#[derive(Debug, ComputaInput)]
pub struct Input {
	pub owner: H160,

	#[computa(payload = "Stream")]
	pub stream1: StreamId,

	#[computa(payload = "Stream")]
	pub stream2: StreamId,

	pub wanted: U256,
}

#[derive(Debug, ComputaOutput)]
pub struct Output {
	pub result: U256,
}

handle_with_entry!(query_and_sum, Input);
fn query_and_sum(input: Input) -> Output {
	if input.owner == H160::zero() {
		panic!("invalid owner");
	}

	let stream1 = input.stream1_data().unwrap();
	let stream2 = input.stream2_data().unwrap();

	let result = stream1.num + stream2.num;

	Output {
		result: U256(result.into()),
	}
}

pub struct Stream {
	pub id: String,
	pub num: i128,
}
