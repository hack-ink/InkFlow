pub fn safe_ratio(numerator: f64, denominator: f64) -> f64 {
	if denominator > 0.0 { numerator / denominator } else { 0.0 }
}

pub fn truncate_text(text: &str, max_len: usize) -> String {
	let text = text.trim();

	if text.is_empty() {
		return String::new();
	}
	if max_len == 0 {
		return String::new();
	}
	if max_len == usize::MAX {
		return text.to_string();
	}

	let mut iter = text.chars();
	let mut out = String::new();

	for _ in 0..max_len {
		let Some(ch) = iter.next() else {
			return text.to_string();
		};

		out.push(ch);
	}

	if iter.next().is_some() {
		out.push('…');
	}

	out
}

pub fn join_text_parts(parts: &[String]) -> String {
	parts.iter().map(|p| p.trim()).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ")
}
