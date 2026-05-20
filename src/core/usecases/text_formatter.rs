/// Formats text specifically for overlay display, enforcing Thai character integrity
/// and auto-calculating optimal distribution chunks.
pub struct TextFormatter;

impl TextFormatter {
    /// A helper function to handle Thai word-wrapping and character integrity.
    pub fn wrap_thai_text(text: &str) -> String {
        if !text.chars().any(|c| ('\u{0E00}'..='\u{0E7F}').contains(&c)) {
            return text.to_string();
        }

        let mut out = String::with_capacity(text.len() * 2);
        let chars: Vec<char> = text.chars().collect();
        let has_spaces = text.contains(' ');

        for i in 0..chars.len() {
            let c = chars[i];
            out.push(c);

            if i + 1 < chars.len() {
                let next = chars[i + 1];

                if next == ' ' || next == '\n' {
                    continue;
                }

                let next_is_mark = "\u{0E30}\u{0E31}\u{0E32}\u{0E33}\u{0E34}\u{0E35}\u{0E36}\u{0E37}\u{0E38}\u{0E39}\u{0E45}\u{0E47}\u{0E48}\u{0E49}\u{0E4A}\u{0E4B}\u{0E4C}\u{0E4D}\u{0E4E}".contains(next);
                let next_is_leading = "\u{0E40}\u{0E41}\u{0E42}\u{0E43}\u{0E44}".contains(next);
                let curr_is_ending =
                    "\u{0E30}\u{0E32}\u{0E33}\u{0E45}\u{0E4C}\u{0E46}\u{0E2F}".contains(c);
                let is_cluster = (c == 'ห'
                    && "\u{0E19}\u{0E0D}\u{0E21}\u{0E22}\u{0E23}\u{0E25}\u{0E27}".contains(next))
                    || ("\u{0E01}\u{0E02}\u{0E04}\u{0E15}\u{0E1B}\u{0E1C}\u{0E1E}\u{0E1F}"
                        .contains(c)
                        && "\u{0E23}\u{0E25}\u{0E27}".contains(next));

                if next_is_mark || is_cluster {
                    out.push('\u{2060}');
                } else if next_is_leading
                    || curr_is_ending
                    || (!has_spaces && c != ' ' && c != '\n')
                {
                    out.push('\u{200B}');
                } else if next != ' ' && next != '\n' && c != ' ' {
                    out.push('\u{2060}');
                }
            }
        }
        out.replace(' ', "\u{200B}")
    }

    /// Splits translated text into suitable visual chunks matching target OCR line segments.
    pub fn create_chunks(trans: &str, lines_count: usize) -> Vec<String> {
        let words: Vec<&str> = trans.split_whitespace().collect();
        let has_spaces = words.len() > 1;
        let lines_count = lines_count.max(1);

        let is_thai = trans
            .chars()
            .any(|c| (0x0E01..=0x0E7F).contains(&(c as u32)));
        if has_spaces {
            let words_per_line = (words.len() as f32 / lines_count as f32).ceil() as usize;
            let mut c = Vec::new();
            for chunk in words.chunks(words_per_line.max(1)) {
                let separator = if is_thai { "\u{200B}" } else { " " };
                c.push(chunk.join(separator));
            }
            c
        } else {
            let chars: Vec<char> = trans.chars().collect();
            let chars_per_line = (chars.len() as f32 / lines_count as f32).ceil() as usize;
            let mut c = Vec::new();
            for chunk in chars.chunks(chars_per_line.max(1)) {
                c.push(chunk.iter().collect::<String>());
            }
            c
        }
    }
}
