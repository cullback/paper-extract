mod prompt;
mod schema;

use base64::{Engine as _, engine::general_purpose};
use clap::Parser;
use csv::Writer;
use prompt::build_prompt;
use reqwest::Client;
use schema::{SchemaField, build_json_schema, read_schema};
use serde::Deserialize;
use serde_json::{Value, json};
use std::env;
use std::fs::{self, File};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the schema CSV file
    schema: String,

    /// Path to the PDF file to extract data from
    pdf: String,

    /// Path to the output CSV file
    output: String,

    /// Number of fields to process in each batch
    #[arg(long, default_value_t = 20)]
    batch: usize,
}

#[derive(Debug, Deserialize)]
struct ExtractedField {
    value: Option<serde_json::Value>,
    match_type: String,
    comment: Option<String>,
    page: i64,
    xmin: f64,
    ymin: f64,
    xmax: f64,
    ymax: f64,
}

use std::collections::HashMap;
type ExtractionResult = HashMap<String, ExtractedField>;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let schema = read_schema(&args.schema);

    // Split schema into batches
    let batches: Vec<Vec<SchemaField>> = schema
        .chunks(args.batch)
        .map(<[SchemaField]>::to_vec)
        .collect();

    let pdf_base64 = pdf_to_base64(&args.pdf);

    let api_key = env::var("OPENROUTER_API_KEY")
        .expect("OPENROUTER_API_KEY environment variable not set");

    // Process each batch and collect results
    let mut all_results = HashMap::new();

    for batch_fields in &batches {
        let response =
            call_openrouter(pdf_base64.clone(), batch_fields, &api_key).await;

        // Extract results from response and add to all_results
        let content = &response["choices"][0]["message"]["content"];
        let content_str = if content.is_string() {
            content.as_str().unwrap()
        } else {
            panic!("Expected string content in response");
        };

        let batch_results: ExtractionResult = serde_json::from_str(content_str)
            .expect("Failed to parse extracted data into ExtractionResult");

        // Merge batch results into all_results
        all_results.extend(batch_results);
    }

    write_csv(&args.output, &all_results, &schema);
}

fn pdf_to_base64(path: &str) -> String {
    let pdf_data = fs::read(path).expect("Failed to read PDF file");
    let base64_data = general_purpose::STANDARD.encode(pdf_data);
    format!("data:application/pdf;base64,{base64_data}")
}

async fn call_openrouter(
    pdf_base64: String,
    fields: &[SchemaField],
    api_key: &str,
) -> Value {
    let client = Client::new();

    let json_schema = build_json_schema(fields);
    let prompt = build_prompt(fields);

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

fn write_csv(
    output_path: &str,
    extracted_data: &ExtractionResult,
    fields: &[SchemaField],
) {
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

    for field in fields {
        let field_data =
            extracted_data.get(&field.field_name).unwrap_or_else(|| {
                panic!(
                    "Field {} not found in extraction result",
                    field.field_name
                )
            });

        let value = match field_data.value.clone() {
            Some(Value::String(string_val)) => string_val,
            Some(Value::Number(number_val)) => number_val.to_string(),
            Some(Value::Bool(bool_val)) => bool_val.to_string(),
            Some(Value::Null) | None => String::new(),
            Some(value_obj) => {
                serde_json::to_string(&value_obj).unwrap_or_default()
            }
        };

        let row = vec![
            field.field_name.clone(),
            value,
            field_data.match_type.clone(),
            field_data.comment.clone().unwrap_or_default(),
            field_data.page.to_string(),
            field_data.xmin.to_string(),
            field_data.ymin.to_string(),
            field_data.xmax.to_string(),
            field_data.ymax.to_string(),
        ];

        writer.write_record(&row).expect("Failed to write data row");
    }

    writer.flush().expect("Failed to flush CSV writer");
}
