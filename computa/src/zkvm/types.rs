use ethabi::{ParamType, Token};
pub use primitive_types::H160;

pub trait ToParamType {
	fn to_param_type() -> ParamType;
}

macro_rules! input_type {
	($name:ident, $type:ty, $param_type:expr) => {
		#[derive(Debug)]
		pub struct $name(pub $type);

		impl Into<ParamType> for $name {
			fn into(self) -> ParamType {
				$param_type
			}
		}

		impl ToParamType for $name {
			fn to_param_type() -> ParamType {
				$param_type
			}
		}
	};
}

input_type!(StreamId, String, ParamType::String);
input_type!(InputModel, String, ParamType::String);
input_type!(I256, primitive_types::U256, ParamType::Int(4));
input_type!(U256, primitive_types::U256, ParamType::Uint(4));

impl ToParamType for H160 {
	fn to_param_type() -> ParamType {
		ParamType::Address
	}
}

macro_rules! into_token_impl {
	($type:ty, $conversion:expr) => {
		impl IntoToken for $type {
			fn into_token(self) -> Token {
				$conversion(self)
			}
		}
	};
}

pub trait IntoToken {
	fn into_token(self) -> Token;
}

// into_token_impl!(H160, |x: H160| Token::Address(x));
into_token_impl!(I256, |x: I256| Token::Int(x.0));
into_token_impl!(U256, |x: U256| Token::Uint(x.0));
into_token_impl!(StreamId, |x: StreamId| Token::String(x.0));
