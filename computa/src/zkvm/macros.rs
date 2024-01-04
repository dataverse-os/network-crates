use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Query, attributes(param_type))]
pub fn derive_query(input: TokenStream) -> TokenStream {
	// Parse the input tokens into a syntax tree
	let input = parse_macro_input!(input as DeriveInput);

	// Get the name of the struct
	let name = input.ident;

	// Initialize an empty vector to hold the ParamTypes
	let mut param_types = Vec::new();

	// If the struct has fields, iterate over them
	if let syn::Data::Struct(data_struct) = input.data {
		for field in data_struct.fields {
			// Get the type of the field
			let field_type = field.ty;

			// Match the type of the field to the corresponding ParamType
			let param_type = match quote! {#field_type}.to_string().as_str() {
				"H160" => quote! { ParamType::FixedBytes(32) },
				"String" => quote! { ParamType::String },
				_ => panic!("Unsupported type for Query derive macro"),
			};

			// Push the ParamType to the vector
			param_types.push(param_type);
		}
	}

	// Generate the implementation
	let expanded = quote! {
		impl Query for #name {
			fn types() -> Vec<ParamType> {
				vec![#(#param_types),*]
			}
		}
	};

	// Return the generated implementation
	TokenStream::from(expanded)
}
