use csv::Reader;
use serde::Deserialize;
use serde::de::Error as DeError;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::fs;

#[derive(Debug, Clone)]
pub enum SchemaKind {
    Categorical,
    Number,
    Text,
}

#[derive(Debug, Clone)]
pub struct SchemaField {
    pub field_name: String,
    pub description: String,
    pub kind: SchemaKind,
    pub infer: bool,
}

impl<'de> Deserialize<'de> for SchemaField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawSchemaField {
            field_name: String,
            description: String,
            kind: String,
            infer: String,
        }

        let raw = RawSchemaField::deserialize(deserializer)?;

        // Validate field_name
        if raw.field_name.len() > 40 {
            return Err(DeError::custom(format!(
                "Field name '{}' exceeds 16 characters (length: {})",
                raw.field_name,
                raw.field_name.len()
            )));
        }

        if !raw.field_name.is_ascii() {
            return Err(DeError::custom(format!(
                "Field name '{}' contains non-ASCII characters",
                raw.field_name
            )));
        }

        // Validate description
        if raw.description.len() > 120 {
            return Err(DeError::custom(format!(
                "Description for field '{}' exceeds 100 characters (length: {})",
                raw.field_name,
                raw.description.len()
            )));
        }

        if !raw.description.is_ascii() {
            return Err(DeError::custom(format!(
                "Description for field '{}' contains non-ASCII characters",
                raw.field_name
            )));
        }

        // Parse kind with error reporting (must be lowercase)
        let kind = match raw.kind.as_str() {
            "categorical" => SchemaKind::Categorical,
            "number" => SchemaKind::Number,
            "text" => SchemaKind::Text,
            _ => {
                return Err(DeError::custom(format!(
                    "Invalid schema kind '{}' for field '{}'. Must be one of: categorical, number, text (lowercase only)",
                    raw.kind, raw.field_name
                )));
            }
        };

        // Parse infer with error reporting (must be lowercase)
        let infer = match raw.infer.as_str() {
            "true" => true,
            "false" => false,
            _ => {
                return Err(DeError::custom(format!(
                    "Invalid infer value '{}' for field '{}'. Must be true or false (lowercase only)",
                    raw.infer, raw.field_name
                )));
            }
        };

        Ok(Self {
            field_name: raw.field_name,
            description: raw.description,
            kind,
            infer,
        })
    }
}

pub fn parse_schema_csv(csv_content: &str) -> Result<Vec<SchemaField>, String> {
    let mut reader = Reader::from_reader(csv_content.as_bytes());
    let mut fields = Vec::new();
    let mut seen_names = HashSet::new();

    for (index, result) in reader.deserialize().enumerate() {
        let row_num = index.saturating_add(2);
        let field: SchemaField = result.map_err(|e| {
            format!("Failed to parse schema row {row_num}: {e}")
        })?;

        // Check for duplicate field names
        if !seen_names.insert(field.field_name.clone()) {
            return Err(format!(
                "Duplicate field name '{}' found in schema at row {}",
                field.field_name, row_num
            ));
        }

        fields.push(field);
    }

    Ok(fields)
}

pub fn read_schema(path: &str) -> Vec<SchemaField> {
    let file_content =
        fs::read_to_string(path).expect("Failed to read schema file");

    parse_schema_csv(&file_content).unwrap_or_else(|e| panic!("{}", e))
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
            "required": ["value", "match_type", "comment", "page", "xmin", "ymin", "xmax", "ymax"],
            "additionalProperties": false
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_schema() {
        let csv = "field_name,description,kind,infer\n\
                   title,Paper title,text,false\n\
                   year,Publication year,number,true";

        let result = parse_schema_csv(csv);
        assert!(result.is_ok());
        let fields = result.unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].field_name, "title");
        assert_eq!(fields[1].field_name, "year");
    }

    #[test]
    fn test_field_name_too_long() {
        let csv = "field_name,description,kind,infer\n\
                   this_field_name_17,Valid description,text,true";

        let result = parse_schema_csv(csv);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceeds 16 characters"));
    }

    #[test]
    fn test_field_name_non_ascii() {
        let csv = "field_name,description,kind,infer\n\
                   field_émoji,Valid description,text,true";

        let result = parse_schema_csv(csv);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("non-ASCII"));
    }

    #[test]
    fn test_description_too_long() {
        let csv = "field_name,description,kind,infer\n\
                   field,This description is way too long and exceeds one hundred characters which should trigger a validation error,text,false";

        let result = parse_schema_csv(csv);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceeds 100 characters"));
    }

    #[test]
    fn test_description_non_ascii() {
        let csv = "field_name,description,kind,infer\n\
                   field,Description with émoji,text,true";

        let result = parse_schema_csv(csv);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("non-ASCII"));
    }

    #[test]
    fn test_duplicate_field_names() {
        let csv = "field_name,description,kind,infer\n\
                   duplicate,First description,text,true\n\
                   duplicate,Second description,number,false";

        let result = parse_schema_csv(csv);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Duplicate field name"));
    }

    #[test]
    fn test_invalid_kind() {
        let csv = "field_name,description,kind,infer\n\
                   field,Valid description,invalid_type,true";

        let result = parse_schema_csv(csv);
        assert!(result.is_err());
        let error_msg = result.unwrap_err();
        assert!(error_msg.contains("Invalid schema kind"));
        assert!(error_msg.contains("categorical, number, text"));
    }

    #[test]
    fn test_invalid_infer() {
        let csv = "field_name,description,kind,infer\n\
                   field,Valid description,text,maybe";

        let result = parse_schema_csv(csv);
        assert!(result.is_err());
        let error_msg = result.unwrap_err();
        assert!(error_msg.contains("Invalid infer value"));
        assert!(error_msg.contains("Must be true or false (lowercase only)"));
    }

    #[test]
    fn test_valid_infer_values() {
        let csv = "field_name,description,kind,infer\n\
                   field1,Desc,text,true\n\
                   field2,Desc,text,false";

        let result = parse_schema_csv(csv);
        assert!(result.is_ok());
        let fields = result.unwrap();
        assert_eq!(fields[0].infer, true);
        assert_eq!(fields[1].infer, false);
    }

    #[test]
    fn test_invalid_infer_values() {
        let test_cases = vec![
            "yes", "no", "1", "0", "y", "n", "on", "off", "TRUE", "FALSE",
            "True", "False",
        ];

        for invalid_value in test_cases {
            let csv = format!(
                "field_name,description,kind,infer\nfield,Desc,text,{}",
                invalid_value
            );
            let result = parse_schema_csv(&csv);
            assert!(
                result.is_err(),
                "Should reject infer value: {}",
                invalid_value
            );
            let error_msg = result.unwrap_err();
            assert!(error_msg.contains("Invalid infer value"));
            assert!(
                error_msg.contains("Must be true or false (lowercase only)")
            );
        }
    }

    #[test]
    fn test_invalid_uppercase_kind() {
        let test_cases = vec!["TEXT", "Number", "CATEGORICAL", "Categorical"];

        for invalid_kind in test_cases {
            let csv = format!(
                "field_name,description,kind,infer\nfield,Desc,{},true",
                invalid_kind
            );
            let result = parse_schema_csv(&csv);
            assert!(
                result.is_err(),
                "Should reject kind value: {}",
                invalid_kind
            );
            let error_msg = result.unwrap_err();
            assert!(error_msg.contains("Invalid schema kind"));
            assert!(
                error_msg
                    .contains("categorical, number, text (lowercase only)")
            );
        }
    }

    #[test]
    fn test_valid_lowercase_kind() {
        let csv = "field_name,description,kind,infer\n\
                   field1,Desc,text,true\n\
                   field2,Desc,number,false\n\
                   field3,Desc,categorical,true";

        let result = parse_schema_csv(csv);
        assert!(result.is_ok());
        let fields = result.unwrap();
        assert!(matches!(fields[0].kind, SchemaKind::Text));
        assert!(matches!(fields[1].kind, SchemaKind::Number));
        assert!(matches!(fields[2].kind, SchemaKind::Categorical));
    }
}
