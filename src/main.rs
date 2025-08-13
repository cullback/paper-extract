use base64::{Engine as _, engine::general_purpose};
use csv::Reader;
use serde::Deserialize;
use serde_json::{Value, json};
use std::env;
use std::fs::{self, File};
use std::process::exit;

#[derive(Debug, Deserialize)]
struct SchemaField {
    field_name: String,
    description: String,
    kind: String,
    infer: bool,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: {} <pdf_path> <schema_path> <output_path>", args[0]);
        exit(1);
    }

    let pdf_path = &args[1];
    let schema_path = &args[2];
    let output_path = &args[3];

    println!("PDF: {pdf_path}");
    println!("Schema: {schema_path}");
    println!("Output: {output_path}");

    let schema = read_schema(schema_path);
    println!("Loaded {} fields from schema", schema.len());
    for field in &schema {
        println!(
            "  - {}: {} ({}) [infer: {}]",
            field.field_name, field.description, field.kind, field.infer
        );
    }

    let json_schema = build_json_schema(&schema);
    println!("Built JSON schema for structured output:");
    println!("{}", serde_json::to_string_pretty(&json_schema).unwrap());

    let pdf_base64 = pdf_to_base64(pdf_path);
    println!(
        "PDF encoded to base64 data URL ({} chars)",
        pdf_base64.len()
    );
}

fn read_schema(path: &str) -> Vec<SchemaField> {
    let file = File::open(path).expect("Failed to open schema file");
    let mut reader = Reader::from_reader(file);

    let mut fields = Vec::new();
    for result in reader.deserialize() {
        let field: SchemaField = result.expect("Failed to parse schema row");
        fields.push(field);
    }

    fields
}

fn pdf_to_base64(path: &str) -> String {
    let pdf_data = fs::read(path).expect("Failed to read PDF file");
    let base64_data = general_purpose::STANDARD.encode(pdf_data);
    format!("data:application/pdf;base64,{base64_data}")
}

fn build_json_schema(fields: &[SchemaField]) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for field in fields {
        let field_type = match field.kind.as_str() {
            "number" => "number",
            _ => "string",
        };

        properties.insert(
            field.field_name.clone(),
            json!({
                "type": field_type,
                "description": field.description
            }),
        );

        required.push(field.field_name.clone());
    }

    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}
