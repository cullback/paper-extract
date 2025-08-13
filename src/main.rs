use base64::{Engine as _, engine::general_purpose};
use csv::{Reader, Writer};
use reqwest::Client;
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

    println!("\nDebug: Full request will be sent to OpenRouter...");

    let pdf_base64 = pdf_to_base64(pdf_path);
    println!(
        "PDF encoded to base64 data URL ({} chars)",
        pdf_base64.len()
    );

    let api_key = env::var("OPENROUTER_API_KEY")
        .expect("OPENROUTER_API_KEY environment variable not set");

    let response = call_openrouter(pdf_base64, &schema, &api_key).await;
    println!("OpenRouter response:");
    println!("{}", serde_json::to_string_pretty(&response).unwrap());

    write_csv(output_path, &response, &schema);
    println!("\nData written to {output_path}");
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

async fn call_openrouter(
    pdf_base64: String,
    fields: &[SchemaField],
    api_key: &str,
) -> Value {
    let client = Client::new();

    let json_schema = build_json_schema(fields);

    let mut prompt =
        String::from("Extract the following fields from this document:\n\n");
    for field in fields {
        prompt.push_str("- ");
        prompt.push_str(&field.field_name);
        prompt.push_str(": ");
        prompt.push_str(&field.description);
        if field.infer {
            prompt.push_str(" (infer if not explicitly stated)");
        }
        prompt.push('\n');
    }
    prompt.push_str(
        "\nReturn the data in the exact JSON format specified by the schema.",
    );

    let request_body = json!({
        "model": "google/gemini-2.5-flash",
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": prompt
                    },
                    {
                        "type": "file",
                        "file": {
                            "filename": "document.pdf",
                            "file_data": pdf_base64,
                        }
                    }
                ]
            }
        ],
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "extraction",
                "strict": true,
                "schema": json_schema
            }
        }
    });

    let response = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .expect("Failed to send request to OpenRouter");

    let response_text = response.text().await.expect("Failed to read response");
    serde_json::from_str(&response_text).expect("Failed to parse JSON response")
}

fn write_csv(output_path: &str, response: &Value, fields: &[SchemaField]) {
    let file = File::create(output_path).expect("Failed to create output file");
    let mut writer = Writer::from_writer(file);

    let mut headers = Vec::new();
    for field in fields {
        headers.push(field.field_name.clone());
    }
    writer
        .write_record(&headers)
        .expect("Failed to write headers");

    let content = &response["choices"][0]["message"]["content"];
    let extracted_data: Value = if content.is_string() {
        serde_json::from_str(content.as_str().unwrap())
            .expect("Failed to parse extracted data")
    } else {
        content.clone()
    };

    let mut row = Vec::new();
    for field in fields {
        let value = &extracted_data[&field.field_name];
        let cell_value = match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => String::new(),
            _ => serde_json::to_string(value).unwrap_or_default(),
        };
        row.push(cell_value);
    }
    writer.write_record(&row).expect("Failed to write data row");

    writer.flush().expect("Failed to flush CSV writer");
}
