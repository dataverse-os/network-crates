use std::str::FromStr;

use base64::Engine;
use dataverse_ceramic::StreamId;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessControl {
	pub encryption_provider: Option<EncryptionProvider>,
	pub monetization_provider: Option<MonetizationProvider>,
}

impl FromStr for AccessControl {
	type Err = anyhow::Error;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let v = base64::engine::general_purpose::STANDARD_NO_PAD.decode(&s)?;
		Ok(serde_json::from_slice::<Self>(&v)?)
	}
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptionProvider {
	pub protocol: EncryptionProtocol,
	pub encrypted_symmetric_key: Option<String>,
	pub decryption_conditions: Option<Vec<DecryptionCondition>>,
	pub decryption_conditions_type: Option<DecryptionConditionsTypes>,
}

impl EncryptionProvider {
	pub fn linked_ceramic_models(&self) -> anyhow::Result<Vec<StreamId>> {
		let mut models = vec![];
		if let Some(conditions) = &self.decryption_conditions {
			for ele in conditions {
				match ele {
					DecryptionCondition::AccessControl(ele) => {
						let model_id: StreamId = ele
							.return_value_test
							.value
							.split("=")
							.last()
							.expect("failed to parse returnValueParse.value as ceramic streamId")
							.parse()?;
						models.push(model_id);
					}
					_ => {}
				}
			}
		}
		Ok(models)
	}
}

#[derive(Debug, Deserialize)]
pub enum DecryptionConditionsTypes {
	AccessControlCondition,
	UnifiedAccessControlCondition,
}

#[derive(Debug, Deserialize)]
pub enum EncryptionProtocol {
	Lit,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum DecryptionCondition {
	#[serde(rename_all = "camelCase")]
	AccessControl(AccessControlCondition),
	#[serde(rename_all = "camelCase")]
	Boolean(BooleanCondition),
	#[serde(rename_all = "camelCase")]
	UnifiedAccessControl(Vec<UnifiedAccessControlConditions>),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessControlCondition {
	pub condition_type: String,
	pub contract_address: String,
	pub standard_contract_type: String,
	pub chain: String,
	pub method: String,
	pub parameters: Vec<String>,
	pub return_value_test: ReturnValueTest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BooleanCondition {
	pub operator: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum UnifiedAccessControlConditions {
	#[serde(rename_all = "camelCase")]
	UnifiedAccessControl(UnifiedAccessControlCondition),
	#[serde(rename_all = "camelCase")]
	Boolean(BooleanCondition),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedAccessControlCondition {
	pub contract_address: String,
	pub condition_type: String,
	pub standard_contract_type: Option<String>,
	pub method: Option<String>,
	pub parameters: Option<Vec<String>>,
	pub function_name: Option<String>,
	pub function_params: Option<Vec<String>>,
	pub chain: String,
	pub return_value_test: ReturnValueTest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnValueTest {
	pub key: Option<String>,
	pub comparator: String,
	pub value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonetizationProvider {
	pub protocol: MonetizationProtocol,
	pub chain_id: u64,
	pub base_contract: String,
	pub union_contract: String,
	pub datatoken_id: Option<String>,
	pub data_union_id: Option<String>,
	pub data_union_ids: Option<Vec<String>>,
	pub unlocking_time_stamp: Option<String>,
}

#[derive(Debug, Deserialize)]
pub enum MonetizationProtocol {
	Lens,
}

#[cfg(test)]
mod tests {
	use serde_json::json;

	use super::*;

	#[test]
	fn test_decode_access_control_condition() {
		let data = serde_json::json!({
		  "conditionType": "evmBasic",
		  "contractAddress": "",
		  "standardContractType": "SIWE",
		  "chain": "ethereum",
		  "method": "",
		  "parameters": [
			":resources"
		  ],
		  "returnValueTest": {
			"comparator": "contains",
			"value": "ceramic://*?model=kjzl6hvfrbw6cagt694iim2wuecu7eumeds7qd0p6uzm8dnqsq69ll7kacm05gu"
		  }
		});

		let condition = serde_json::from_value::<AccessControlCondition>(data.clone());
		assert!(condition.is_ok());

		let condition = serde_json::from_value::<DecryptionCondition>(data);
		assert!(condition.is_ok());
	}

	#[test]
	fn test_decode_unified_access_control_condition() {
		let data = serde_json::json!({
		  "contractAddress": "0x8673f21B34319BD0709A7a501BD0fdB614A0a7A1",
		  "conditionType": "evmContract",
		  "functionName": "isCollected",
		  "functionParams": [
			":userAddress"
		  ],
		  "functionAbi": {
			"inputs": [
			  {
				"internalType": "address",
				"name": "user",
				"type": "address"
			  }
			],
			"name": "isCollected",
			"outputs": [
			  {
				"internalType": "bool",
				"name": "",
				"type": "bool"
			  }
			],
			"stateMutability": "view",
			"type": "function"
		  },
		  "chain": "mumbai",
		  "returnValueTest": {
			"key": "",
			"comparator": "=",
			"value": "true"
		  }
		});

		let condition = serde_json::from_value::<UnifiedAccessControlCondition>(data.clone());
		assert!(condition.is_ok());

		let condition = serde_json::from_value::<UnifiedAccessControlConditions>(data);
		assert!(condition.is_ok());
	}

	#[test]
	fn test_decode_unified_access_control_conditions() {
		let data = serde_json::json!([
		  {
			"conditionType": "evmBasic",
			"contractAddress": "",
			"standardContractType": "",
			"chain": "ethereum",
			"method": "",
			"parameters": [
			  ":userAddress"
			],
			"returnValueTest": {
			  "comparator": "=",
			  "value": "0x312eA852726E3A9f633A0377c0ea882086d66666"
			}
		  },
		  {
			"operator": "or"
		  },
		  {
			"contractAddress": "0x8673f21B34319BD0709A7a501BD0fdB614A0a7A1",
			"conditionType": "evmContract",
			"functionName": "isCollected",
			"functionParams": [
			  ":userAddress"
			],
			"functionAbi": {
			  "inputs": [
				{
				  "internalType": "address",
				  "name": "user",
				  "type": "address"
				}
			  ],
			  "name": "isCollected",
			  "outputs": [
				{
				  "internalType": "bool",
				  "name": "",
				  "type": "bool"
				}
			  ],
			  "stateMutability": "view",
			  "type": "function"
			},
			"chain": "mumbai",
			"returnValueTest": {
			  "key": "",
			  "comparator": "=",
			  "value": "true"
			}
		  }
		]);

		let condition = serde_json::from_value::<Vec<UnifiedAccessControlConditions>>(data);
		assert!(condition.is_ok());
	}

	#[test]
	fn test_decode_encryption_provider() {
		let data = serde_json::json!({
		  "protocol": "Lit",
		  "encryptedSymmetricKey": "587360f2772503abd24cf63da2250005cab77d9a668552e7bd3728e8a73e30d4c46279611d29d807bfeee58c0f8d1e0dc4ba9b91c1130ae11fedebd7ec7f8934bcecddd717e24a99245596728c501db9255b8ba1f7faab19ab996f6df03ab7940eef2eede4d31821a184f9c5cbad25e5ebb1497363462ede2fff555b9704438a0000000000000020dc503f11f7c6e70c4432ef9f7ab6a3a8d805facf169d2ae6f0b662facef3a4a95d730f991ee0f28bf997eb87102000b6",
		  "decryptionConditions": [
			{
			  "conditionType": "evmBasic",
			  "contractAddress": "",
			  "standardContractType": "SIWE",
			  "chain": "ethereum",
			  "method": "",
			  "parameters": [
				":resources"
			  ],
			  "returnValueTest": {
				"comparator": "contains",
				"value": "ceramic://*?model=kjzl6hvfrbw6cagt694iim2wuecu7eumeds7qd0p6uzm8dnqsq69ll7kacm05gu"
			  }
			},
			{
			  "operator": "and"
			},
			{
			  "conditionType": "evmBasic",
			  "contractAddress": "",
			  "standardContractType": "SIWE",
			  "chain": "ethereum",
			  "method": "",
			  "parameters": [
				":resources"
			  ],
			  "returnValueTest": {
				"comparator": "contains",
				"value": "ceramic://*?model=kjzl6hvfrbw6c7gu88g66z28n81lcpbg6hu2t8pu2pui0sfnpvsrhqn3kxh9xai"
			  }
			},
			{
			  "operator": "and"
			},
			{
			  "conditionType": "evmBasic",
			  "contractAddress": "",
			  "standardContractType": "SIWE",
			  "chain": "ethereum",
			  "method": "",
			  "parameters": [
				":resources"
			  ],
			  "returnValueTest": {
				"comparator": "contains",
				"value": "ceramic://*?model=kjzl6hvfrbw6c86gt9j415yw2x8stmkotcrzpeutrbkp42i4z90gp5ibptz4sso"
			  }
			},
			{
			  "operator": "and"
			},
			{
			  "conditionType": "evmBasic",
			  "contractAddress": "",
			  "standardContractType": "SIWE",
			  "chain": "ethereum",
			  "method": "",
			  "parameters": [
				":resources"
			  ],
			  "returnValueTest": {
				"comparator": "contains",
				"value": "ceramic://*?model=kjzl6hvfrbw6catek36h3pep09k9gymfnla9k6ojlgrmwjogvjqg8q3zpybl1yu"
			  }
			},
			{
			  "operator": "and"
			},
			[
			  {
				"conditionType": "evmBasic",
				"contractAddress": "",
				"standardContractType": "",
				"chain": "ethereum",
				"method": "",
				"parameters": [
				  ":userAddress"
				],
				"returnValueTest": {
				  "comparator": "=",
				  "value": "0x312eA852726E3A9f633A0377c0ea882086d66666"
				}
			  },
			  {
				"operator": "or"
			  },
			  {
				"contractAddress": "0x8673f21B34319BD0709A7a501BD0fdB614A0a7A1",
				"conditionType": "evmContract",
				"functionName": "isCollected",
				"functionParams": [
				  ":userAddress"
				],
				"functionAbi": {
				  "inputs": [
					{
					  "internalType": "address",
					  "name": "user",
					  "type": "address"
					}
				  ],
				  "name": "isCollected",
				  "outputs": [
					{
					  "internalType": "bool",
					  "name": "",
					  "type": "bool"
					}
				  ],
				  "stateMutability": "view",
				  "type": "function"
				},
				"chain": "mumbai",
				"returnValueTest": {
				  "key": "",
				  "comparator": "=",
				  "value": "true"
				}
			  }
			]
		  ],
		  "decryptionConditionsType": "UnifiedAccessControlCondition"
		});

		let encryption_provider = serde_json::from_value::<EncryptionProvider>(data);
		assert!(encryption_provider.is_ok());
	}

	#[test]
	fn decode_access_control() -> anyhow::Result<()> {
		let data = serde_json::json!({
		  "encryptionProvider": {
			"protocol": "Lit",
			"encryptedSymmetricKey": "587360f2772503abd24cf63da2250005cab77d9a668552e7bd3728e8a73e30d4c46279611d29d807bfeee58c0f8d1e0dc4ba9b91c1130ae11fedebd7ec7f8934bcecddd717e24a99245596728c501db9255b8ba1f7faab19ab996f6df03ab7940eef2eede4d31821a184f9c5cbad25e5ebb1497363462ede2fff555b9704438a0000000000000020dc503f11f7c6e70c4432ef9f7ab6a3a8d805facf169d2ae6f0b662facef3a4a95d730f991ee0f28bf997eb87102000b6",
			"decryptionConditions": [
			  {
				"conditionType": "evmBasic",
				"contractAddress": "",
				"standardContractType": "SIWE",
				"chain": "ethereum",
				"method": "",
				"parameters": [
				  ":resources"
				],
				"returnValueTest": {
				  "comparator": "contains",
				  "value": "ceramic://*?model=kjzl6hvfrbw6cagt694iim2wuecu7eumeds7qd0p6uzm8dnqsq69ll7kacm05gu"
				}
			  },
			  {
				"operator": "and"
			  },
			  {
				"conditionType": "evmBasic",
				"contractAddress": "",
				"standardContractType": "SIWE",
				"chain": "ethereum",
				"method": "",
				"parameters": [
				  ":resources"
				],
				"returnValueTest": {
				  "comparator": "contains",
				  "value": "ceramic://*?model=kjzl6hvfrbw6c7gu88g66z28n81lcpbg6hu2t8pu2pui0sfnpvsrhqn3kxh9xai"
				}
			  },
			  {
				"operator": "and"
			  },
			  {
				"conditionType": "evmBasic",
				"contractAddress": "",
				"standardContractType": "SIWE",
				"chain": "ethereum",
				"method": "",
				"parameters": [
				  ":resources"
				],
				"returnValueTest": {
				  "comparator": "contains",
				  "value": "ceramic://*?model=kjzl6hvfrbw6c86gt9j415yw2x8stmkotcrzpeutrbkp42i4z90gp5ibptz4sso"
				}
			  },
			  {
				"operator": "and"
			  },
			  {
				"conditionType": "evmBasic",
				"contractAddress": "",
				"standardContractType": "SIWE",
				"chain": "ethereum",
				"method": "",
				"parameters": [
				  ":resources"
				],
				"returnValueTest": {
				  "comparator": "contains",
				  "value": "ceramic://*?model=kjzl6hvfrbw6catek36h3pep09k9gymfnla9k6ojlgrmwjogvjqg8q3zpybl1yu"
				}
			  },
			  {
				"operator": "and"
			  },
			  [
				{
				  "conditionType": "evmBasic",
				  "contractAddress": "",
				  "standardContractType": "",
				  "chain": "ethereum",
				  "method": "",
				  "parameters": [
					":userAddress"
				  ],
				  "returnValueTest": {
					"comparator": "=",
					"value": "0x312eA852726E3A9f633A0377c0ea882086d66666"
				  }
				},
				{
				  "operator": "or"
				},
				{
				  "contractAddress": "0x8673f21B34319BD0709A7a501BD0fdB614A0a7A1",
				  "conditionType": "evmContract",
				  "functionName": "isCollected",
				  "functionParams": [
					":userAddress"
				  ],
				  "functionAbi": {
					"inputs": [
					  {
						"internalType": "address",
						"name": "user",
						"type": "address"
					  }
					],
					"name": "isCollected",
					"outputs": [
					  {
						"internalType": "bool",
						"name": "",
						"type": "bool"
					  }
					],
					"stateMutability": "view",
					"type": "function"
				  },
				  "chain": "mumbai",
				  "returnValueTest": {
					"key": "",
					"comparator": "=",
					"value": "true"
				  }
				}
			  ]
			],
			"decryptionConditionsType": "UnifiedAccessControlCondition"
		  },
		  "monetizationProvider": {
			"protocol": "Lens",
			"baseContract": "0x7582177F9E536aB0b6c721e11f383C326F2Ad1D5",
			"unionContract": "0x7582177F9E536aB0b6c721e11f383C326F2Ad1D5",
			"chainId": 80001,
			"datatokenId": "0x8673f21B34319BD0709A7a501BD0fdB614A0a7A1"
		  }
		});
		let access_control = serde_json::from_value::<AccessControl>(data);
		assert!(access_control.is_ok());

		let access_control = access_control.unwrap();
		let linked_ceramic_models = access_control
			.encryption_provider
			.unwrap()
			.linked_ceramic_models();
		assert!(linked_ceramic_models.is_ok());
		assert_eq!(
			linked_ceramic_models.unwrap(),
			vec![
				"kjzl6hvfrbw6cagt694iim2wuecu7eumeds7qd0p6uzm8dnqsq69ll7kacm05gu".parse()?,
				"kjzl6hvfrbw6c7gu88g66z28n81lcpbg6hu2t8pu2pui0sfnpvsrhqn3kxh9xai".parse()?,
				"kjzl6hvfrbw6c86gt9j415yw2x8stmkotcrzpeutrbkp42i4z90gp5ibptz4sso".parse()?,
				"kjzl6hvfrbw6catek36h3pep09k9gymfnla9k6ojlgrmwjogvjqg8q3zpybl1yu".parse()?,
			]
		);
		Ok(())
	}

	#[test]
	fn decode_access_control2() -> anyhow::Result<()> {
		let data = json!({
		  "encryptionProvider": {
			"protocol": "Lit",
			"encryptedSymmetricKey": "7b2263697068657274657874223a227239623862754d396e71756c2b4c2b656164526e775a7175412f3771572b546d64527942642f4a5275582b3945566776524a5a4143476b4e637145766971524b4f2f367377655948452b56476266444b34525853386d645572365448324b75706a414f6d32704d584d7435424a4643635043584a656e7439724e7343655a4369355030445951786679656a4f595457594845643853414c354565516c7a693148563878396d506562445032676a2f487169666c3537674776714d3964454f6d43336d4944222c2264617461546f456e637279707448617368223a2261663266313363323733353537353839383766313037356234356664663135383034353433386535333634383961323366653463386438306537626332336130227d",
			"decryptionConditions": [
			  {
				"contractAddress": "",
				"standardContractType": "",
				"chain": "ethereum",
				"method": "",
				"parameters": [
				  ":userAddress"
				],
				"returnValueTest": {
				  "comparator": "=",
				  "value": "0xCedf62df194542b3fb3E376848f87cE9afd3CdDe"
				}
			  },
			  {
				"operator": "and"
			  },
			  {
				"conditionType": "evmBasic",
				"contractAddress": "",
				"standardContractType": "SIWE",
				"chain": "ethereum",
				"method": "",
				"parameters": [
				  ":resources"
				],
				"returnValueTest": {
				  "comparator": "contains",
				  "value": "ceramic://*?model=kjzl6hvfrbw6c6q29co83n8av8krlst35wbojuhxxs2nfuykirbq4nndjjvdu3q"
				}
			  },
			  {
				"operator": "and"
			  },
			  {
				"conditionType": "evmBasic",
				"contractAddress": "",
				"standardContractType": "SIWE",
				"chain": "ethereum",
				"method": "",
				"parameters": [
				  ":resources"
				],
				"returnValueTest": {
				  "comparator": "contains",
				  "value": "ceramic://*?model=kjzl6hvfrbw6c887jhjyn9kz15x6jwna3kviup8x4ls2a0v6xb13qviipb8y7mg"
				}
			  },
			  {
				"operator": "and"
			  },
			  {
				"conditionType": "evmBasic",
				"contractAddress": "",
				"standardContractType": "SIWE",
				"chain": "ethereum",
				"method": "",
				"parameters": [
				  ":resources"
				],
				"returnValueTest": {
				  "comparator": "contains",
				  "value": "ceramic://*?model=kjzl6hvfrbw6c8p5coctwqf8fohkebtw0hixh34yzcwaohwmqnde0mhs7pvk44e"
				}
			  }
			],
			"decryptionConditionsType": "AccessControlCondition"
		  }
		});
		let access_control = serde_json::from_value::<AccessControl>(data);
		assert!(access_control.is_err());
		Ok(())
	}
}
