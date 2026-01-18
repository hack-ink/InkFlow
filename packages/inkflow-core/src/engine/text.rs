pub(crate) fn append_normalized(out: &mut String, next: &str) {
	let trimmed = next.trim();
	if trimmed.is_empty() {
		return;
	}

	if out.is_empty() {
		out.push_str(trimmed);
		return;
	}

	let last = out.chars().last().unwrap_or(' ');
	let next_first = trimmed.chars().next().unwrap_or(' ');
	if needs_space_between(last, next_first) {
		out.push(' ');
	}

	out.push_str(trimmed);
}

fn needs_space_between(left: char, right: char) -> bool {
	if is_cjk(left) || is_cjk(right) {
		return false;
	}

	left.is_ascii_alphanumeric() && right.is_ascii_alphanumeric()
}

fn is_cjk(c: char) -> bool {
	let u = c as u32;
	matches!(
		u,
		0x4E00..=0x9FFF
			| 0x3400..=0x4DBF
			| 0x3040..=0x309F
			| 0x30A0..=0x30FF
			| 0xAC00..=0xD7AF
	)
}

#[cfg(test)]
mod tests {
	use super::append_normalized;

	#[test]
	fn normalization_keeps_space_between_latin_words() {
		let mut out = String::from("hello");
		append_normalized(&mut out, "world");
		assert_eq!(out, "hello world");
	}

	#[test]
	fn normalization_avoids_space_for_cjk() {
		let mut out = String::from("你好");
		append_normalized(&mut out, "世界");
		assert_eq!(out, "你好世界");
	}
}
