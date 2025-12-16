// std
use std::mem;

struct ErrorRate {
	distance: usize,
	reference_len: usize,
	rate: f64,
}

pub fn print_accuracy(engine: &str, reference: &str, hypothesis: &str) {
	if let Some(wer) = compute_wer(reference, hypothesis) {
		println!(
			"[accuracy] engine={engine} metric=wer value={:.3} edits={} ref_words={}",
			wer.rate, wer.distance, wer.reference_len
		);
	}
	if let Some(cer) = compute_cer(reference, hypothesis) {
		println!(
			"[accuracy] engine={engine} metric=cer value={:.3} edits={} ref_chars={}",
			cer.rate, cer.distance, cer.reference_len
		);
	}
}

fn compute_wer(reference: &str, hypothesis: &str) -> Option<ErrorRate> {
	let reference_tokens = tokenize_words(reference);

	if reference_tokens.is_empty() {
		return None;
	}

	let hypothesis_tokens = tokenize_words(hypothesis);
	let distance = edit_distance(&reference_tokens, &hypothesis_tokens);
	let reference_len = reference_tokens.len();

	Some(ErrorRate { distance, reference_len, rate: distance as f64 / reference_len as f64 })
}

fn compute_cer(reference: &str, hypothesis: &str) -> Option<ErrorRate> {
	let reference_chars = normalize_chars(reference);

	if reference_chars.is_empty() {
		return None;
	}

	let hypothesis_chars = normalize_chars(hypothesis);
	let distance = edit_distance(&reference_chars, &hypothesis_chars);
	let reference_len = reference_chars.len();

	Some(ErrorRate { distance, reference_len, rate: distance as f64 / reference_len as f64 })
}

fn tokenize_words(text: &str) -> Vec<String> {
	let mut normalized = String::with_capacity(text.len());

	for ch in text.chars() {
		for lower in ch.to_lowercase() {
			if lower.is_alphanumeric() || lower == '\'' {
				normalized.push(lower);
			} else {
				normalized.push(' ');
			}
		}
	}

	normalized.split_whitespace().filter(|t| !t.is_empty()).map(|t| t.to_string()).collect()
}

fn normalize_chars(text: &str) -> Vec<char> {
	let mut out: Vec<char> = Vec::with_capacity(text.len());

	for ch in text.chars() {
		for lower in ch.to_lowercase() {
			if lower.is_alphanumeric() || lower == '\'' {
				out.push(lower);
			}
		}
	}
	out
}

fn edit_distance<T: Eq>(a: &[T], b: &[T]) -> usize {
	if a.is_empty() {
		return b.len();
	}
	if b.is_empty() {
		return a.len();
	}

	let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
	let mut prev = (0..=short.len()).collect::<Vec<_>>();
	let mut curr = vec![0; short.len() + 1];

	for (i, long_item) in long.iter().enumerate() {
		curr[0] = i + 1;

		for (j, short_item) in short.iter().enumerate() {
			let cost = if long_item == short_item { 0 } else { 1 };
			let deletion = prev[j + 1].saturating_add(1);
			let insertion = curr[j].saturating_add(1);
			let substitution = prev[j].saturating_add(cost);

			curr[j + 1] = deletion.min(insertion).min(substitution);
		}

		mem::swap(&mut prev, &mut curr);
	}

	prev[short.len()]
}
