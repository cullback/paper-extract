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

const PROMPT_TEMPLATE: &str = include_str!("prompt.md");

fn build_json_schema(fields: &[SchemaField]) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for field in fields {
        let field_type = match field.kind.as_str() {
            "number" => "number",
            _ => "string",
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
                    "type": "string"
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

async fn call_openrouter(
    pdf_base64: String,
    fields: &[SchemaField],
    api_key: &str,
) -> Value {
    let client = Client::new();

    let json_schema = build_json_schema(fields);

    let mut fields_list = String::new();
    for field in fields {
        fields_list.push_str(&format!(
            "- **{}**: {}\n",
            field.field_name, field.description
        ));
        if field.infer {
            fields_list.push_str(
                "  (This field should be inferred if not explicitly found)\n",
            );
        }
    }

    let prompt = PROMPT_TEMPLATE.replace("{{FIELDS_LIST}}", &fields_list);

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

    let headers = vec![
        "field_name",
        "value",
        "match_type",
        "comment",
        "page",
        "xmin",
        "ymin",
        "xmax",
        "ymax",
    ];
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

    for field in fields {
        let field_data = &extracted_data[&field.field_name];

        let value = if let Some(s) = field_data["value"].as_str() {
            s.to_string()
        } else if let Some(n) = field_data["value"].as_f64() {
            n.to_string()
        } else if let Some(b) = field_data["value"].as_bool() {
            b.to_string()
        } else {
            String::new()
        };

        let match_type =
            field_data["match_type"].as_str().unwrap_or("not_found");
        let comment = field_data["comment"].as_str().unwrap_or("");
        let page = field_data["page"].as_i64().unwrap_or(0).to_string();
        let xmin = field_data["xmin"].as_f64().unwrap_or(0.0).to_string();
        let ymin = field_data["ymin"].as_f64().unwrap_or(0.0).to_string();
        let xmax = field_data["xmax"].as_f64().unwrap_or(0.0).to_string();
        let ymax = field_data["ymax"].as_f64().unwrap_or(0.0).to_string();

        let row = vec![
            field.field_name.clone(),
            value,
            match_type.to_string(),
            comment.to_string(),
            page,
            xmin,
            ymin,
            xmax,
            ymax,
        ];

        writer.write_record(&row).expect("Failed to write data row");
    }

    writer.flush().expect("Failed to flush CSV writer");
}
