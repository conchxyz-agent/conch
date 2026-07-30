/// CONCH Agent — Milestone 4
///
/// A minimal local agent that:
///   1. Reads a .conch file (its memory)
///   2. Sends the content to a local Ollama model for reasoning
///   3. Writes the response as a new .conch file (its new memory)
///
/// No backend. No cloud. Pure local file operations.
///
/// Usage:
///   conch-agent --input memory.conch --output response.conch
///   conch-agent --input memory.conch --output response.conch --model llama3.2
///   conch-agent --input memory.conch --output response.conch --ollama http://host.docker.internal:11434

use std::path::PathBuf;
use clap::Parser;
use serde::{Deserialize, Serialize};
use conch_api::conch::{parse_conch, validate_conch, write_conch, ConchBuilder, FieldType};

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "conch-agent",
    about = "Local CONCH agent — reads a conch, reasons with Ollama, writes a new conch",
    version = "0.1"
)]
struct Cli {
    /// Input .conch file (the agent's memory to read)
    #[arg(long)]
    input: PathBuf,

    /// Output .conch file (where the agent writes its response)
    #[arg(long)]
    output: PathBuf,

    /// Ollama model to use
    #[arg(long, default_value = "llama3.2")]
    model: String,

    /// Ollama base URL
    #[arg(long, default_value = "http://host.docker.internal:11434")]
    ollama: String,

    /// Agent identity (acts as creator key)
    #[arg(long, default_value = "conch-agent-v1")]
    agent_id: String,
}

// ── Ollama types ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

// ── Core logic ────────────────────────────────────────────────────────────────

async fn call_ollama(base_url: &str, model: &str, prompt: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::new();
    let url = format!("{base_url}/api/generate");

    let body = OllamaRequest {
        model: model.to_string(),
        prompt: prompt.to_string(),
        stream: false,
    };

    let res = client
        .post(&url)
        .json(&body)
        .send()
        .await?
        .json::<OllamaResponse>()
        .await?;

    Ok(res.response.trim().to_string())
}

fn build_prompt(conch: &conch_api::conch::ConchObject) -> String {
    // Extract the most meaningful content from the conch to send to the LLM.
    // We pass the schema field descriptions so the model understands context,
    // and the actual data values so it has something to reason about.

    let mut parts = Vec::new();

    parts.push("You are a local AI agent. You have just read the following memory conch.".to_string());
    parts.push(format!("Conch type (schema fields): {}",
        conch.schema.fields.keys().cloned().collect::<Vec<_>>().join(", ")
    ));
    parts.push("\nMemory contents:".to_string());

    for (key, value) in &conch.data {
        let display = match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(a) => a.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            other => other.to_string(),
        };
        if !display.is_empty() {
            if let Some(field) = conch.schema.fields.get(key) {
                parts.push(format!("  {key} ({}): {display}", field.description));
            } else {
                parts.push(format!("  {key}: {display}"));
            }
        }
    }

    parts.push("\nBased on this memory, write a concise knowledge synthesis. What is the core insight here? What should be remembered about this? Keep it under 3 sentences.".to_string());

    parts.join("\n")
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // ── Step 1: Read and validate the input conch ─────────────────────────────
    println!("▶ Reading memory: {}", cli.input.display());

    let json = match std::fs::read_to_string(&cli.input) {
        Ok(s) => s,
        Err(e) => { eprintln!("error: could not read '{}': {e}", cli.input.display()); std::process::exit(1); }
    };

    let conch = match parse_conch(&json) {
        Ok(c) => c,
        Err(e) => { eprintln!("error: parse failed: {e}"); std::process::exit(1); }
    };

    if let Err(errors) = validate_conch(&conch) {
        eprintln!("error: input conch is invalid:");
        for e in &errors { eprintln!("  · {e}"); }
        std::process::exit(1);
    }

    println!("  ✓ valid conch — {} fields, {} history entr{}",
        conch.schema.fields.len(),
        conch.history.len(),
        if conch.history.len() == 1 { "y" } else { "ies" }
    );

    // ── Step 2: Build prompt and call Ollama ──────────────────────────────────
    println!("▶ Reasoning with {} via {}…", cli.model, cli.ollama);

    let prompt = build_prompt(&conch);
    let response = match call_ollama(&cli.ollama, &cli.model, &prompt).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: Ollama call failed: {e}");
            eprintln!("  Is Ollama running? Is '{}' reachable from inside Docker?", cli.ollama);
            eprintln!("  On Windows/Mac, host.docker.internal resolves to your machine's IP.");
            std::process::exit(1);
        }
    };

    println!("  ✓ response received ({} chars)", response.len());
    println!("\n── Agent's synthesis ─────────────────────────────────────────");
    println!("{response}");
    println!("──────────────────────────────────────────────────────────────\n");

    // ── Step 3: Write the response as a new knowledge conch ───────────────────
    println!("▶ Writing new memory: {}", cli.output.display());

    // Source title from the input conch if available
    let source_title = conch.data.get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let output_conch = ConchBuilder::new(&cli.agent_id)
        .field("title",       FieldType::String, true,  "Knowledge title")
        .field("synthesis",   FieldType::String, true,  "Agent's synthesized insight")
        .field("source_conch",FieldType::String, false, "ID of the conch this was derived from")
        .field("model",       FieldType::String, false, "Local model used for reasoning")
        .field("tags",        FieldType::Array,  false, "Topic tags")
        .data("title",        serde_json::Value::String(format!("Synthesis: {source_title}")))
        .data("synthesis",    serde_json::Value::String(response))
        .data("source_conch", serde_json::Value::String(conch.meta.id.clone()))
        .data("model",        serde_json::Value::String(cli.model.clone()))
        .data("tags",         serde_json::Value::Array(vec![
            serde_json::Value::String("agent".into()),
            serde_json::Value::String("synthesis".into()),
            serde_json::Value::String("local".into()),
        ]))
        .build();

    let canonical = match write_conch(&output_conch) {
        Ok(s) => s,
        Err(e) => { eprintln!("error: write failed: {e}"); std::process::exit(1); }
    };

    match std::fs::write(&cli.output, &canonical) {
        Ok(_) => println!("  ✓ written to {}", cli.output.display()),
        Err(e) => { eprintln!("error: could not write file: {e}"); std::process::exit(1); }
    }

    println!("\n✓ Done. The agent's memory is now in: {}", cli.output.display());
}
