use std::path::PathBuf;
use clap::{Parser, Subcommand};
use conch_api::conch::{parse_conch, validate_conch, write_conch, ConchBuilder, FieldType};

// ── CLI definition ────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "conch",
    about = "CONCH CLI — validate, inspect, and create .conch files",
    version = "0.1"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate a .conch file against its own schema
    Validate {
        /// Path to the .conch file
        file: PathBuf,
    },
    /// Pretty-print the structure of a .conch file
    Inspect {
        /// Path to the .conch file
        file: PathBuf,
    },
    /// Create a new .conch from a template and print it to stdout
    New {
        /// Template name: knowledge | memory | artifact | note
        template: String,
        /// Creator public key (hex Ed25519). Defaults to "anonymous"
        #[arg(long, default_value = "anonymous")]
        creator: String,
    },
}

// ── Templates ─────────────────────────────────────────────────────────────────

struct TemplateField {
    name: &'static str,
    field_type: FieldType,
    required: bool,
    description: &'static str,
}

struct Template {
    name: &'static str,
    description: &'static str,
    fields: &'static [TemplateField],
}

static KNOWLEDGE_FIELDS: &[TemplateField] = &[
    TemplateField { name: "title",      field_type: FieldType::String, required: true,  description: "Human-readable title" },
    TemplateField { name: "body",       field_type: FieldType::String, required: true,  description: "Full explanation of the concept" },
    TemplateField { name: "source",     field_type: FieldType::String, required: false, description: "Origin or reference" },
    TemplateField { name: "tags",       field_type: FieldType::Array,  required: false, description: "Topic tags" },
    TemplateField { name: "confidence", field_type: FieldType::Number, required: false, description: "Confidence score 0–10" },
];

static MEMORY_FIELDS: &[TemplateField] = &[
    TemplateField { name: "title",       field_type: FieldType::String, required: true,  description: "Short label for this memory" },
    TemplateField { name: "observation", field_type: FieldType::String, required: true,  description: "What was observed or experienced" },
    TemplateField { name: "context",     field_type: FieldType::String, required: false, description: "Surrounding context" },
    TemplateField { name: "tags",        field_type: FieldType::Array,  required: false, description: "Topic tags" },
    TemplateField { name: "importance",  field_type: FieldType::Number, required: false, description: "Importance score 0–10" },
];

static ARTIFACT_FIELDS: &[TemplateField] = &[
    TemplateField { name: "name",        field_type: FieldType::String, required: true,  description: "Artifact name" },
    TemplateField { name: "description", field_type: FieldType::String, required: true,  description: "What this artifact does" },
    TemplateField { name: "language",    field_type: FieldType::String, required: false, description: "Programming language" },
    TemplateField { name: "snippet",     field_type: FieldType::String, required: false, description: "Key code or content snippet" },
    TemplateField { name: "tags",        field_type: FieldType::Array,  required: false, description: "Topic tags" },
];

static NOTE_FIELDS: &[TemplateField] = &[
    TemplateField { name: "title",   field_type: FieldType::String, required: true,  description: "Note title" },
    TemplateField { name: "content", field_type: FieldType::String, required: true,  description: "Note content" },
    TemplateField { name: "tags",    field_type: FieldType::Array,  required: false, description: "Topic tags" },
];

static TEMPLATES: &[Template] = &[
    Template { name: "knowledge", description: "A knowledge article or concept explanation", fields: KNOWLEDGE_FIELDS },
    Template { name: "memory",    description: "An agent or human memory / observation",     fields: MEMORY_FIELDS },
    Template { name: "artifact",  description: "A code snippet, tool, or reusable artifact", fields: ARTIFACT_FIELDS },
    Template { name: "note",      description: "A simple short-form note",                   fields: NOTE_FIELDS },
];

fn find_template(name: &str) -> Option<&'static Template> {
    TEMPLATES.iter().find(|t| t.name == name)
}

/// Return a placeholder value for a field type so required fields pass validation.
fn default_value(ft: &FieldType) -> serde_json::Value {
    match ft {
        FieldType::String  => serde_json::Value::String(String::new()),
        FieldType::Number  => serde_json::Value::Number(serde_json::Number::from(0)),
        FieldType::Boolean => serde_json::Value::Bool(false),
        FieldType::Array   => serde_json::Value::Array(vec![]),
        FieldType::Object  => serde_json::Value::Object(Default::default()),
    }
}

// ── Commands ──────────────────────────────────────────────────────────────────

fn cmd_validate(file: &PathBuf) {
    let json = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => { eprintln!("error: could not read '{}': {e}", file.display()); std::process::exit(1); }
    };

    let obj = match parse_conch(&json) {
        Ok(o) => o,
        Err(e) => {
            println!("✗  {}", file.display());
            println!("   Parse error: {e}");
            std::process::exit(1);
        }
    };

    match validate_conch(&obj) {
        Ok(()) => println!("✓  {} — valid .conch", file.display()),
        Err(errors) => {
            println!("✗  {} — {} validation error{}", file.display(), errors.len(), if errors.len() == 1 { "" } else { "s" });
            for e in &errors {
                println!("   · {e}");
            }
            std::process::exit(1);
        }
    }
}

fn cmd_inspect(file: &PathBuf) {
    let json = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => { eprintln!("error: could not read '{}': {e}", file.display()); std::process::exit(1); }
    };

    let obj = match parse_conch(&json) {
        Ok(o) => o,
        Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
    };

    let valid = validate_conch(&obj).is_ok();
    let valid_label = if valid { "✓ valid" } else { "✗ invalid" };
    let div = "─".repeat(54);

    println!("\n{div}");
    println!("  {}  [{}]", file.display(), valid_label);
    println!("{div}");

    println!("\nMETA");
    println!("  id            {}", obj.meta.id);
    println!("  version       {}", obj.meta.version);
    println!("  conch_version {}", obj.meta.conch_version);
    println!("  creator       {}", truncate(&obj.meta.creator, 32));
    println!("  created_at    {}", obj.meta.created_at);

    println!("\nSCHEMA  ({} field{})", obj.schema.fields.len(), plural(obj.schema.fields.len()));
    for (name, field) in &obj.schema.fields {
        let req = if field.required { "required" } else { "optional" };
        println!("  {:<18} {:<8}  {}", name, field.field_type.as_str(), req);
    }

    println!("\nDATA");
    for (key, value) in &obj.data {
        let display = match value {
            serde_json::Value::String(s) => format!("\"{}\"", truncate(s, 56)),
            serde_json::Value::Array(a)  => format!("[{} item{}]", a.len(), plural(a.len())),
            other                        => truncate(&other.to_string(), 56),
        };
        println!("  {:<18} {}", key, display);
    }

    println!("\nPERMISSIONS");
    println!("  read   {}", fmt_perm_list(&obj.permissions.read));
    println!("  write  {}", fmt_perm_list(&obj.permissions.write));
    println!("  admin  {}", fmt_perm_list(&obj.permissions.admin));

    println!("\nHISTORY  ({} entr{})", obj.history.len(), if obj.history.len() == 1 { "y" } else { "ies" });
    for entry in &obj.history {
        println!("  {}  {}  by {}", entry.timestamp, entry.action, truncate(&entry.actor, 16));
    }

    println!("\n{div}\n");
}

fn cmd_new(template_name: &str, creator: &str) {
    let tmpl = match find_template(template_name) {
        Some(t) => t,
        None => {
            eprintln!("error: unknown template '{template_name}'");
            let names: Vec<&str> = TEMPLATES.iter().map(|t| t.name).collect();
            eprintln!("available: {}", names.join(", "));
            std::process::exit(1);
        }
    };

    let mut builder = ConchBuilder::new(creator);

    for f in tmpl.fields {
        builder = builder.field(f.name, f.field_type.clone(), f.required, f.description);
        if f.required {
            builder = builder.data(f.name, default_value(&f.field_type));
        }
    }

    let obj = builder.build();

    match write_conch(&obj) {
        Ok(raw) => println!("{raw}"),
        Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn fmt_perm_list(list: &[String]) -> String {
    if list == ["*"] {
        "* (public)".to_string()
    } else {
        format!("{} key{}", list.len(), plural(list.len()))
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Validate { file }              => cmd_validate(&file),
        Command::Inspect  { file }              => cmd_inspect(&file),
        Command::New      { template, creator } => cmd_new(&template, &creator),
    }
}
