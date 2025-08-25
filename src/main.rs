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
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the schema CSV file
    schema: String,

    /// Path to the PDF file to extract data from
    pdf: String,

    /// Path to the output CSV file (defaults to PDF filename with .csv extension)
    output: Option<String>,

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
use std::sync::Arc;
use tokio::task::JoinHandle;
type ExtractionResult = HashMap<String, ExtractedField>;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Determine output path - use provided path or default to PDF name with .csv extension
    let output_path = args.output.unwrap_or_else(|| {
        let mut path = PathBuf::from(&args.pdf);
        path.set_extension("csv");
        path.to_string_lossy().into_owned()
    });

    println!("Processing {} -> {}", args.pdf, output_path);

    let schema = read_schema(&args.schema);

    // Split schema into batches
    let batches: Vec<Vec<SchemaField>> = schema
        .chunks(args.batch)
        .map(<[SchemaField]>::to_vec)
        .collect();

    let pdf_base64 = pdf_to_base64(&args.pdf);

    let api_key = env::var("OPENROUTER_API_KEY")
        .expect("OPENROUTER_API_KEY environment variable not set");

    // Process each batch in parallel
    let pdf_base64_arc = Arc::new(pdf_base64);
    let api_key_arc = Arc::new(api_key);

    println!("Processing {} batches in parallel...", batches.len());

    let mut tasks: Vec<JoinHandle<(usize, ExtractionResult)>> = Vec::new();

    for (batch_idx, batch_fields) in batches.into_iter().enumerate() {
        let pdf_base64_clone = Arc::clone(&pdf_base64_arc);
        let api_key_clone = Arc::clone(&api_key_arc);
        let batch_fields_owned = batch_fields.clone();

        let task = tokio::spawn(async move {
            println!(
                "Starting batch {} ({} fields)",
                batch_idx.saturating_add(1),
                batch_fields_owned.len()
            );

            let response = call_openrouter(
                (*pdf_base64_clone).clone(),
                &batch_fields_owned,
                &api_key_clone,
            )
            .await;

            // Extract results from response
            let content = &response["choices"][0]["message"]["content"];
            let content_str = if content.is_string() {
                content.as_str().unwrap()
            } else {
                panic!("Expected string content in response");
            };

            let batch_results: ExtractionResult = serde_json::from_str(
                content_str,
            )
            .expect("Failed to parse extracted data into ExtractionResult");

            println!(
                "Completed batch {} ({} fields extracted)",
                batch_idx.saturating_add(1),
                batch_results.len()
            );

            (batch_idx, batch_results)
        });

        tasks.push(task);
    }

    // Wait for all tasks to complete and merge results
    let mut all_results = HashMap::new();
    for task in tasks {
        let (_batch_idx, batch_results) = task.await.expect("Task failed");
        all_results.extend(batch_results);
    }

    println!("All batches completed. Writing results to CSV...");

    write_csv(&output_path, &all_results, &schema);
    println!("Done! Results written to {output_path}");
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
        "model": "openai/gpt-5-mini",
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
