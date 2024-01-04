use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{parse_macro_input, Ident};
use syn::{DeriveInput, Fields, Meta, NestedMeta};

#[proc_macro_derive(ComputaInput, attributes(computa))]
pub fn derive_computa_input(input: TokenStream) -> TokenStream {
	let DeriveInput {
		ident: name, data, ..
	} = parse_macro_input!(input as DeriveInput);

	let fields = match &data {
		syn::Data::Struct(s) => &s.fields,
		_ => panic!("Expected a struct"),
	};

	let types: Vec<_> = match fields {
		Fields::Named(fields) => fields.named.iter().map(|f| &f.ty).collect(),
		_ => panic!("Expected named fields"),
	};

	let mut payloads = Vec::new();

	for field in fields {
		for attr in &field.attrs {
			if attr.path.is_ident("computa") {
				if let Meta::List(meta) = attr.parse_meta().unwrap() {
					for nested_meta in meta.nested {
						if let NestedMeta::Meta(Meta::NameValue(meta)) = nested_meta {
							if meta.path.is_ident("payload") {
								if let syn::Lit::Str(lit) = meta.lit {
									// type of the payload data
									let payload_ident =
										Ident::new(lit.value().as_str(), Span::call_site());
									// name of the field
									let field_ident = field.ident.clone().unwrap();
									payloads.push((field_ident, payload_ident));
								}
							}
						}
					}
				}
			}
		}
	}

	let mut payload_method = TokenStream2::default();

	for (_idx, (field, payload)) in payloads.iter().enumerate() {
		let method_name = format_ident!("{}_data", field);
		payload_method.extend(quote! {
				pub fn #method_name(&self) -> anyhow::Result<#payload> {
						unimplemented!()
				}
		});
	}

	let _field_indices: Vec<_> = (0..fields.len()).collect();
	let _field_names: Vec<_> = fields.iter().map(|f| &f.ident).collect();

	let field_count = fields.len();

	let expanded = quote! {
		impl #name {
			#payload_method

			fn field_length() -> usize {
				#field_count
			}
		}

		impl TryFrom<Vec<Token>> for #name {
			type Error = anyhow::Error;

			fn try_from(value: Vec<Token>) -> Result<Self, Self::Error> {
				if value.len() != Self::field_length() {
					return Err(anyhow::anyhow!("Expected {} tokens, got {}", Self::field_length(), value.len()));
				}
				unimplemented!()
				// Ok(Self {
				// 	#(#field_names: value[#field_indices].clone().into_token().into()),*
				// })
			}
		}

		impl Query for #name {
			fn types() -> Vec<ParamType> {
				vec![
					#(#types::to_param_type()),*
				]
			}
		}
	};

	TokenStream::from(expanded)
}

#[proc_macro_derive(ComputaOutput, attributes(computa))]
pub fn derive_computa_output(input: TokenStream) -> TokenStream {
	let DeriveInput {
		ident: name, data, ..
	} = parse_macro_input!(input as DeriveInput);

	let fields = match &data {
		syn::Data::Struct(s) => &s.fields,
		_ => panic!("Expected a struct"),
	};

	let _types: Vec<_> = match fields {
		Fields::Named(fields) => fields.named.iter().map(|f| &f.ty).collect(),
		_ => panic!("Expected named fields"),
	};

	let tokens = fields.iter().map(|f| {
		let name = &f.ident;
		quote! { self.#name.into_token() }
	});

	let expanded = quote! {
		impl #name {
			fn tokens(self) -> Vec<Token> {
				vec![
					#(#tokens),*
				]
			}
		}
	};
	TokenStream::from(expanded)
}
