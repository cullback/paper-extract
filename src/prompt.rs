use crate::schema::SchemaField;
use std::fmt::Write as _;

const PROMPT_TEMPLATE: &str = include_str!("prompt.md");

pub fn build_prompt(fields: &[SchemaField]) -> String {
    let mut fields_list = String::new();
    for field in fields {
        writeln!(
            &mut fields_list,
            "- **{}**: {}",
            field.field_name, field.description
        )
        .unwrap();
        if field.infer {
            fields_list.push_str(
                "  (This field should be inferred if not explicitly found)\n",
            );
        }
    }

    PROMPT_TEMPLATE.replace("{{FIELDS_LIST}}", &fields_list)
}
