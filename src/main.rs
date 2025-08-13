use serde::Deserialize;
use std::env;
use std::process::exit;

#[derive(Debug, Deserialize)]
#[expect(dead_code, reason = "Will be used in later steps")]
struct SchemaField {
    field_name: String,
    description: String,
    kind: String, // "text", "number", "category"
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
}
