use crate::core::ports::{OcrTextBlock, OcrTextLine};

fn get_char_size(line: &OcrTextLine) -> f32 {
    let chars_count = line.text.chars().filter(|c| !c.is_whitespace()).count().max(1) as f32;
    let area = line.w * line.h;
    (area / chars_count).sqrt()
}

fn is_close(a: &OcrTextLine, b: &OcrTextLine) -> bool {
    // Calculate the true average character size based on area and character count
    let char_size_a = get_char_size(a);
    let char_size_b = get_char_size(b);
    let char_size = char_size_a.max(char_size_b);
    
    // Expansion margin. Be conservative to prevent merging separate speech bubbles.
    let expand_y = char_size * 0.8; // Tight vertical gap — avoid merging different bubbles
    let expand_x = char_size * 0.6; // Strict horizontal distance
    
    // Sanity check: Don't merge if font sizes are wildly different (e.g. title vs subtitle)
    let size_ratio = char_size_a / char_size_b;
    if size_ratio > 2.0 || size_ratio < 0.5 {
        return false;
    }
    
    let a_left = a.x - expand_x;
    let a_right = a.x + a.w + expand_x;
    let a_top = a.y - expand_y;
    let a_bottom = a.y + a.h + expand_y;
    
    let b_left = b.x;
    let b_right = b.x + b.w;
    let b_top = b.y;
    let b_bottom = b.y + b.h;
    
    // Check intersection
    !(a_right < b_left || a_left > b_right || a_bottom < b_top || a_top > b_bottom)
}

fn merge_text(lines: &[OcrTextLine]) -> String {
    // If lines contain mostly Asian characters, we don't insert space.
    // Otherwise we insert space.
    // A simple heuristic: if the text contains a lot of ASCII, add space.
    // For now, to be safe and simple: just join them. We rely on the text cleaner 
    // to have stripped unnecessary whitespace, but we might need spaces for English.
    // Let's inspect the first line's characters.
    let is_asian = lines.iter().any(|l| {
        l.text.chars().any(|c| {
            let u = c as u32;
            (u >= 0x4E00 && u <= 0x9FFF) || // CJK Unified Ideographs
            (u >= 0x3040 && u <= 0x309F) || // Hiragana
            (u >= 0x30A0 && u <= 0x30FF)    // Katakana
        })
    });

    let separator = if is_asian { "" } else { " " };
    
    lines.iter()
        .map(|l| l.text.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(separator)
}

pub fn build_blocks(lines: Vec<OcrTextLine>, smart_merge: bool) -> Vec<OcrTextBlock> {
    if lines.is_empty() {
        return vec![];
    }

    if !smart_merge {
        return lines.into_iter().map(|l| OcrTextBlock {
            source_text: l.text.clone(),
            lines: vec![l],
        }).collect();
    }

    let mut blocks: Vec<OcrTextBlock> = Vec::new();

    for line in lines {
        // Find if this line intersects with any existing block
        let mut matched_idx = None;
        for (i, block) in blocks.iter().enumerate() {
            if block.lines.iter().any(|existing_line| is_close(&line, existing_line)) {
                matched_idx = Some(i);
                break;
            }
        }

        if let Some(idx) = matched_idx {
            blocks[idx].lines.push(line);
        } else {
            blocks.push(OcrTextBlock {
                lines: vec![line],
                source_text: String::new(),
            });
        }
    }

    // Build the merged text for each block
    for block in &mut blocks {
        // Sort lines top-to-bottom within each block to ensure correct positional overlay alignment.
        // This guarantees that trans_lines[i] maps to the correct bounding box.
        block.lines.sort_by(|a, b| {
            let y_cmp = a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal);
            if y_cmp != std::cmp::Ordering::Equal {
                y_cmp
            } else {
                a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
            }
        });
        
        block.source_text = merge_text(&block.lines);
    }

    blocks
}
