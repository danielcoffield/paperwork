mod config;

use acroform::{AcroFormDocument, FieldValue};
use chrono::Datelike;
use dialoguer::{Input, MultiSelect, Select, theme::ColorfulTheme};
use open;
use std::collections::HashMap;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = config::load("config.toml")?;
    let theme = ColorfulTheme::default();

    let choices = vec![
        "New Base Document",
        "Make Branch Document",
        "Exit Paperwork",
    ];

    loop {
        let selection = Select::with_theme(&theme)
            .with_prompt("Menu Options")
            .items(&choices)
            .default(0)
            .interact()?;

        match selection {
            0 => make_base(&config, &theme)?,
            1 => make_branch(&config, &theme)?,
            2 => break,
            _ => unreachable!(),
        }
    }

    Ok(())
}

fn make_base(
    config: &config::Config,
    theme: &ColorfulTheme,
) -> Result<(), Box<dyn std::error::Error>> {
    let base_doc = config
        .paperwork
        .get("base")
        .ok_or("No base document found in config.toml!")?;

    let mut fields: HashMap<String, String> = HashMap::new();

    for field_name in base_doc.fields.keys() {
        if is_user_data(field_name) {
            continue;
        }
        let value: String = Input::with_theme(theme)
            .with_prompt(field_name)
            .interact_text()?;
        fields.insert(field_name.to_string(), value);
    }

    let client_name = fields
        .get("client_name")
        .ok_or("needs client_name!")?
        .clone();

    fs::create_dir_all("clients")?;
    let file_name = format!("clients/{}.json", client_name.replace(" ", "_"));
    let json = serde_json::to_string_pretty(&fields)?;
    fs::write(file_name, json)?;

    fill_pdf(base_doc, &fields, &config.user)?;

    println!("\n✓ Created base document for {}", client_name);

    Ok(())
}

fn make_branch(
    config: &config::Config,
    theme: &ColorfulTheme,
) -> Result<(), Box<dyn std::error::Error>> {
    let clients = list_clients()?;
    if clients.is_empty() {
        println!("No client data! Create a base document first!");
        return Ok(());
    }

    let client_idx = Select::with_theme(theme)
        .with_prompt("Select a client")
        .items(&clients)
        .default(0)
        .interact()?;
    let client_name = &clients[client_idx];

    let mut client_fields = load_client(client_name)?;

    let branch_docs: Vec<&String> = config
        .paperwork
        .keys()
        .filter(|k| k.as_str() != "base")
        .collect();

    if branch_docs.is_empty() {
        println!("No branch documents defined in config.toml!");
        return Ok(());
    }

    let selections = MultiSelect::with_theme(theme)
        .with_prompt("Select branch documents to make (space to select, enter to confirm)")
        .items(&branch_docs)
        .interact()?;

    if selections.is_empty() {
        println!("No branch documents selected!");
        return Ok(());
    }

    for idx in selections {
        let branch_name = branch_docs[idx];
        let branch = config.paperwork.get(branch_name).unwrap();

        let missing_fields: Vec<String> = branch
            .fields
            .keys()
            .filter(|f| !is_user_data(f) && !client_fields.contains_key(f.as_str()))
            .map(|f| f.to_string())
            .collect();

        if !missing_fields.is_empty() {
            println!("More information needed for {}:", branch_name);
            for field_name in missing_fields {
                let value: String = Input::with_theme(theme)
                    .with_prompt(&field_name)
                    .interact_text()?;
                client_fields.insert(field_name, value);
            }
        }

        fill_pdf(branch, &client_fields, &config.user)?;
    }

    println!("\n✓ Made Branch Document(s).");

    Ok(())
}

fn fill_pdf(
    paper: &config::PaperMapping,
    client_fields: &HashMap<String, String>,
    user: &config::UserData,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all("output")?;
    let date = chrono::Local::now();
    let current_date = format!("{}/{}/{}", date.month(), date.day(), date.year());

    let mut pdf_values: HashMap<String, FieldValue> = HashMap::new();

    for (true_name, pdf_field_names) in &paper.fields {
        let value = resolve_field(true_name, &current_date, client_fields, user);
        if let Some(value) = value {
            for pdf_field_name in pdf_field_names {
                pdf_values.insert(pdf_field_name.clone(), FieldValue::Text(value.clone()));
            }
        } else {
            println!("Warning: no value for field '{}'", true_name);
        }
    }

    let mut doc = AcroFormDocument::from_pdf(&paper.template)?;

    let client_name = client_fields
        .get("client_name")
        .map(|s| s.as_str())
        .unwrap_or("Unknown");
    let output_name = resolve_output_name(&paper.output_name, client_name);

    doc.fill_and_save(pdf_values, &output_name)?;

    println!("Created {}...", &output_name);

    open::that(&output_name)?;

    Ok(())
}

fn list_clients() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let dir = "clients";
    if !std::path::Path::new(dir).exists() {
        return Ok(Vec::new());
    }
    let mut clients = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                clients.push(stem.replace("_", " "));
            }
        }
    }
    clients.sort();
    Ok(clients)
}

fn load_client(client_name: &str) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let file_name = format!("clients/{}.json", client_name.replace(" ", "_"));
    let json = fs::read_to_string(&file_name)?;
    let fields = serde_json::from_str(&json)?;
    Ok(fields)
}

fn resolve_field(
    true_name: &str,
    current_date: &str,
    client_fields: &HashMap<String, String>,
    user: &config::UserData,
) -> Option<String> {
    match true_name {
        "current_date" => Some(current_date.to_string()),
        "staff_name" => Some(user.name.clone()),
        "staff_email" => Some(user.email.clone()),
        "staff_phone" => Some(user.phone.clone()),
        "organization" => Some(user.organization.clone()),
        other => client_fields.get(other).cloned(),
    }
}

fn resolve_output_name(template: &str, client_name: &str) -> String {
    let surname = client_name.split_whitespace().last().unwrap_or("Unknown");
    let filename = template.replace("{client_surname}", surname);
    format!("output/{}", filename)
}

fn is_user_data(field_name: &str) -> bool {
    matches!(
        field_name,
        "current_date" | "staff_name" | "staff_email" | "staff_phone" | "organization"
    )
}
