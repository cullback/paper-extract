use csv::Reader;
use serde::Deserialize;
use std::env;
use std::fs::File;
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
