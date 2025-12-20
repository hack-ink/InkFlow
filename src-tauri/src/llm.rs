use std::time::Duration;

use rig::{
	client::CompletionClient,
	completion::{AssistantContent, CompletionModel},
	providers::openai,
};

use crate::{error::AppError, settings::LlmSettingsResolved};

#[derive(Clone, Debug)]
pub struct RewriteResult {
	pub text: String,
	pub model: String,
}

pub async fn rewrite(
	settings: &LlmSettingsResolved,
	input: &str,
) -> Result<RewriteResult, AppError> {
	let client = build_openai_client(settings)?;
	let model = client.completion_model(settings.model.as_str());
	let response = model
		.completion_request(input.to_string())
		.preamble(settings.system_prompt.clone())
		.temperature(settings.temperature as f64)
		.send()
		.await
		.map_err(|err| {
			AppError::new("llm_request_failed", format!("LLM request failed: {err}."))
		})?;

	let content = extract_response_text(response.choice);
	let content = content.trim().to_string();
	if content.is_empty() {
		return Err(AppError::new("llm_response_failed", "The LLM response was empty."));
	}

	Ok(RewriteResult { text: content, model: settings.model.clone() })
}

fn build_openai_client(settings: &LlmSettingsResolved) -> Result<openai::Client, AppError> {
	let base_url = settings.base_url.trim().trim_end_matches('/');
	let http_client =
		reqwest::Client::builder().timeout(Duration::from_secs(30)).build().map_err(|err| {
			AppError::new("llm_client_failed", format!("Failed to build HTTP client: {err}."))
		})?;

	openai::Client::<reqwest::Client>::builder()
		.api_key(&settings.api_key)
		.base_url(base_url)
		.http_client(http_client)
		.build()
		.map_err(|err| {
			AppError::new("llm_client_failed", format!("Failed to build OpenAI client: {err}."))
		})
}

fn extract_response_text(choice: rig::OneOrMany<AssistantContent>) -> String {
	let mut out = String::new();
	for part in choice.into_iter() {
		if let AssistantContent::Text(text) = part {
			out.push_str(text.text());
		}
	}
	out
}
