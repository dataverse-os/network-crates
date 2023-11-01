use ceramic_event::StreamId;
use schemars::schema::RootSchema;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type of account relation, whether single instance per account or multiple (list)
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ModelAccountRelation {
    /// Multiple instances of model for account
    List,
    /// Single instead of model for account
    Single,
}

/// How a model is related, whether by account or document
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ModelRelationDefinition {
    /// Related to the account
    Account,
    /// Related to a document (instance)
    Document {
        /// Model related to
        model: StreamId,
    },
}

/// Describe how model views are created
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ModelViewDefinition {
    /// View at account level
    DocumentAccount,
    /// View at version level
    DocumentVersion,
    /// View at document relation level
    RelationDocument {
        /// Related model
        model: StreamId,
        /// Property related to
        property: String,
    },
    /// View from relation
    RelationFrom {
        /// model related to
        model: StreamId,
        /// property related to
        property: String,
    },
    /// Count of relations from model
    RelationCountFrom {
        /// model related to
        model: StreamId,
        /// property related to
        property: String,
    },
}

/// Schema encoded as Cbor
#[derive(Debug, Deserialize, Serialize)]
#[repr(transparent)]
pub struct CborSchema(serde_json::Value);

/// Definition of a model for use when creating instances
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDefinition {
    version: &'static str,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    schema: CborSchema,
    account_relation: ModelAccountRelation,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    relations: HashMap<String, ModelRelationDefinition>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    views: HashMap<String, ModelViewDefinition>,
}

impl ModelDefinition {
    /// Create a new definition for a type that implements `GetRootSchema`
    pub fn new<T: GetRootSchema>(
        name: &str,
        account_relation: ModelAccountRelation,
    ) -> anyhow::Result<Self> {
        let schema = T::root_schema();
        let schema = serde_json::to_value(&schema)?;
        Ok(Self {
            version: "1.0",
            name: name.to_string(),
            description: None,
            schema: CborSchema(schema),
            account_relation,
            relations: HashMap::default(),
            views: HashMap::default(),
        })
    }

    /// Schema of this definition
    pub fn schema(&self) -> anyhow::Result<RootSchema> {
        let s = serde_json::from_value(self.schema.0.clone())?;
        Ok(s)
    }

    /// Apply description to this definition
    pub fn with_description(&mut self, description: String) -> &mut Self {
        self.description = Some(description);
        self
    }

    /// Apply a relation to this definition
    pub fn with_relation(&mut self, key: String, relation: ModelRelationDefinition) -> &mut Self {
        self.relations.insert(key, relation);
        self
    }

    /// Apply a view to this definition
    pub fn with_view(&mut self, key: String, view: ModelViewDefinition) -> &mut Self {
        self.views.insert(key, view);
        self
    }
}

/// A trait which helps convert a type that implements `JsonSchema` into a `RootSchema` with
/// appropriate attributes
pub trait GetRootSchema: JsonSchema {
    /// Convert this object into a `RootSchema` with appropriate attributes
    fn root_schema() -> RootSchema {
        let settings = schemars::gen::SchemaSettings::default().with(|s| {
            s.meta_schema = Some("https://json-schema.org/draft/2020-12/schema".to_string());
            s.option_nullable = true;
            s.option_add_null_type = false;
        });
        let gen = settings.into_generator();
        gen.into_root_schema_for::<Self>()
    }
}
