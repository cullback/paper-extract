use csv::Reader;
use serde::Deserialize;
use serde_json::{Value, json};
use std::fmt::{self, Display};
use std::fs::File;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum SchemaKind {
    Categorical,
    Number,
    Text,
}

impl Display for SchemaKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::Categorical => write!(f, "Categorical"),
            Self::Number => write!(f, "Number"),
            Self::Text => write!(f, "Text"),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct SchemaField {
    pub field_name: String,
    pub description: String,
    pub kind: SchemaKind,
    /// Whether the field can be inferred
    pub infer: bool,
}

pub fn read_schema(path: &str) -> Vec<SchemaField> {
    let file = File::open(path).expect("Failed to open schema file");
    let mut reader = Reader::from_reader(file);

    let mut fields = Vec::new();
    for result in reader.deserialize() {
        let field: SchemaField = result.expect("Failed to parse schema row");
        fields.push(field);
    }

    fields
}

pub fn build_json_schema(fields: &[SchemaField]) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for field in fields {
        let field_type = match field.kind {
            SchemaKind::Number => "number",
            SchemaKind::Categorical | SchemaKind::Text => "string",
        };

        let field_schema = json!({
            "type": "object",
            "properties": {
                "value": {
                    "type": [field_type, "null"],
                    "description": field.description
                },
                "match_type": {
                    "type": "string",
                    "enum": ["found", "not_found", "inferred"]
                },
                "comment": {
                    "type": ["string", "null"]
                },
                "page": {
                    "type": "integer"
                },
                "xmin": {
                    "type": "number"
                },
                "ymin": {
                    "type": "number"
                },
                "xmax": {
                    "type": "number"
                },
                "ymax": {
                    "type": "number"
                }
            },
            "required": ["value", "match_type", "comment", "page", "xmin", "ymin", "xmax", "ymax"]
        });

        properties.insert(field.field_name.clone(), field_schema);
        required.push(field.field_name.clone());
    }

    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}
