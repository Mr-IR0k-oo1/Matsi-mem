pub mod amp;
pub mod claude;
pub mod generic;
pub mod gemini;
pub mod mistral;
pub mod ollama;
pub mod vibe;

use anyhow::Result;

pub trait Executor {
    fn name(&self) -> &str;
    fn run(&self, prompt: &str) -> Result<String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Model {
    Ollama      { model: String },
    Mistral     { model: String },
    Gemini,
    GeminiCli,
    ClaudePrint,
    ClaudeCode,
    Amp,
    Vibe,
    Custom { label: String, command: String, args: Vec<String> },
}

impl Model {
    pub fn display_name(&self) -> String {
        match self {
            Model::Ollama   { model } => format!("ollama/{}", model),
            Model::Mistral  { model } => format!("mistral/{}", model),
            Model::Gemini             => "gemini".into(),
            Model::GeminiCli          => "gemini-cli".into(),
            Model::ClaudePrint        => "claude --print".into(),
            Model::ClaudeCode         => "claude code".into(),
            Model::Amp                => "amp".into(),
            Model::Vibe               => "vibe".into(),
            Model::Custom { label, .. }=> label.clone(),
        }
    }

    pub fn category(&self) -> &'static str {
        match self {
            Model::Ollama { .. } | Model::Mistral { .. } => "Local",
            Model::Gemini | Model::GeminiCli             => "Cloud",
            Model::ClaudePrint | Model::ClaudeCode        => "Claude",
            Model::Amp | Model::Vibe                      => "Agents",
            Model::Custom { .. }                          => "Custom",
        }
    }

    pub fn executor(&self) -> Box<dyn Executor> {
        match self {
            Model::Ollama  { model } => Box::new(ollama::OllamaExecutor::new(model.clone())),
            Model::Mistral { model } => Box::new(mistral::MistralExecutor::new(model.clone())),
            Model::Gemini            => Box::new(gemini::GeminiExecutor::new(false)),
            Model::GeminiCli         => Box::new(gemini::GeminiExecutor::new(true)),
            Model::ClaudePrint       => Box::new(claude::ClaudeExecutor::new(true)),
            Model::ClaudeCode        => Box::new(claude::ClaudeExecutor::new(false)),
            Model::Amp               => Box::new(amp::AmpExecutor::new()),
            Model::Vibe              => Box::new(vibe::VibeExecutor::new()),
            Model::Custom { label, command, args } => Box::new(
                generic::GenericExecutor::new(
                    label.clone(), command.clone(),
                    args.iter().map(|s| s.as_str()).collect(),
                )
            ),
        }
    }

    pub fn presets() -> Vec<Model> {
        vec![
            Model::Ollama   { model: "llama3".into() },
            Model::Ollama   { model: "mistral".into() },
            Model::Ollama   { model: "codellama".into() },
            Model::Ollama   { model: "deepseek-coder".into() },
            Model::Mistral  { model: "mistral-small".into() },
            Model::GeminiCli,
            Model::Gemini,
            Model::ClaudePrint,
            Model::ClaudeCode,
            Model::Amp,
            Model::Vibe,
        ]
    }
}

pub fn run(model: &Model, prompt: &str) -> Result<String> {
    model.executor().run(prompt)
}
