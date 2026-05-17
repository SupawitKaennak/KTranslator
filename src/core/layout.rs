use crate::core::ports::{OcrTextBlock, OcrTextLine};

fn get_char_size(line: &OcrTextLine) -> f32 {
    // The height of a text line bounding box is generally the most reliable 
    // indicator of its font size, especially for vertical manga text and CJK/Thai.
    // Avoid area calculation because it gets heavily skewed by line-length differences.
    line.h.min(line.w).max(12.0)
}

fn is_close(a: &OcrTextLine, b: &OcrTextLine) -> bool {
    let char_size_a = get_char_size(a);
    let char_size_b = get_char_size(b);
    let char_size = char_size_a.max(char_size_b);
    
    // Check if the lines are likely part of vertical text (typical in Japanese Manga)
    let is_a_vertical = a.h > a.w * 1.2;
    let is_b_vertical = b.h > b.w * 1.2;
    let is_vertical_context = is_a_vertical || is_b_vertical;

    if is_vertical_context {
        // --- CJK Vertical Text Context ---
        // 1. Horizontal Stack Case (separate vertical columns in the same bubble)
        let y_overlap_len = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
        let shorter_h = a.h.min(b.h);
        let has_y_overlap = y_overlap_len > 0.0 && (y_overlap_len / shorter_h) > 0.4;
        
        let x_gap = if a.x + a.w < b.x { b.x - (a.x + a.w) } else if b.x + b.w < a.x { a.x - (b.x + b.w) } else { 0.0 };
        let close_horizontally = x_gap < char_size * 1.2;

        if has_y_overlap && close_horizontally {
            // Extra column separation guard: if separate vertical columns are too far, don't merge
            let x_dist = (a.x - b.x).abs();
            if x_dist > char_size * 1.6 {
                return false;
            }
            return true;
        }

        // 2. Same Column Segment Case (vertical splits inside a column)
        let x_overlap_len = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
        let narrower_w = a.w.min(b.w);
        let has_x_overlap = x_overlap_len > 0.0 && (x_overlap_len / narrower_w) > 0.6;

        let y_gap = if a.y + a.h < b.y { b.y - (a.y + a.h) } else if b.y + b.h < a.y { a.y - (b.y + b.h) } else { 0.0 };
        let close_vertically = y_gap < char_size * 1.5;

        if has_x_overlap && close_vertically {
            return true;
        }
    } else {
        // --- Horizontal Text Context (English, Thai, Russian, etc.) ---
        // 1. Vertical Stack Case (standard lines stacked one below the other)
        let x_overlap_len = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
        let narrower_w = a.w.min(b.w);
        let has_x_overlap = x_overlap_len > 0.0 && (x_overlap_len / narrower_w) > 0.3; // Align horizontally

        let y_gap = if a.y + a.h < b.y { b.y - (a.y + a.h) } else if b.y + b.h < a.y { a.y - (b.y + b.h) } else { 0.0 };
        let close_vertically = y_gap < char_size * 0.8; // Tight vertical space in standard text

        if has_x_overlap && close_vertically {
            return true;
        }

        // 2. Same Line Segment Case (split words or horizontally adjacent inline text on the SAME line)
        let y_overlap_len = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
        let shorter_h = a.h.min(b.h);
        let has_y_overlap = y_overlap_len > 0.0 && (y_overlap_len / shorter_h) > 0.6; // High vertical alignment (same line)

        // Calculate horizontal overlap
        let x_overlap_len = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
        let has_x_overlap = x_overlap_len > 0.0;

        let x_gap = if a.x + a.w < b.x { b.x - (a.x + a.w) } else if b.x + b.w < a.x { a.x - (b.x + b.w) } else { 0.0 };
        
        // Strict inline checking: 
        // 1. Must NOT have any horizontal overlap (if they overlap in X, they are separate overlapping columns/bubbles).
        // 2. The horizontal gap must be very small (less than 45% of the average character size).
        let close_horizontally = !has_x_overlap && x_gap > 0.0 && x_gap < char_size * 0.45;

        if has_y_overlap && close_horizontally {
            return true;
        }
    }
    
    false
}

fn merge_text(lines: &[OcrTextLine]) -> String {
    let is_asian = lines.iter().any(|l| {
        l.text.chars().any(|c| {
            let u = c as u32;
            (u >= 0x4E00 && u <= 0x9FFF) || // CJK Unified Ideographs
            (u >= 0x3040 && u <= 0x309F) || // Hiragana
            (u >= 0x30A0 && u <= 0x30FF)    // Katakana
        })
    });

    let default_separator = if is_asian { "" } else { " " };
    let mut result = String::new();
    
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.text.trim();
        if trimmed.is_empty() {
            continue;
        }
        
        if result.is_empty() {
            result.push_str(trimmed);
        } else {
            // Check if the previous added line ended with a hyphen (Hyphenation Join Logic)
            let prev_trimmed = lines[..i].iter().rev()
                .map(|l| l.text.trim())
                .find(|s| !s.is_empty());
                
            let mut join_without_space = false;
            if let Some(prev) = prev_trimmed {
                if prev.ends_with('-') {
                    join_without_space = true;
                }
            }
            
            if join_without_space {
                // Keep the hyphen (e.g. ETOU-SAN, BLOOD-LUST) but join WITHOUT inserting a separator space.
                // This preserves semantic suffixes and hyphenated words perfectly for the translation engine.
                result.push_str(trimmed);
            } else {
                result.push_str(default_separator);
                result.push_str(trimmed);
            }
        }
    }
    
    result
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

    // Build the merged text for each block with smart natural reading order sorting
    for block in &mut blocks {
        block.lines.sort_by(|a, b| {
            let is_a_vertical = a.h > a.w * 1.2;
            let is_b_vertical = b.h > b.w * 1.2;
            let is_vertical = is_a_vertical || is_b_vertical;
            
            if is_vertical {
                // Vertical CJK reading order: Right-to-Left (x descending) primarily, then top-to-bottom
                let char_size = get_char_size(a).max(get_char_size(b));
                let x_diff = (a.x - b.x).abs();
                if x_diff > char_size * 0.5 {
                    b.x.partial_cmp(&a.x).unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
                }
            } else {
                // Horizontal reading order: Top-to-Bottom (y ascending) primarily, then left-to-right
                let y_diff = (a.y - b.y).abs();
                let char_size = get_char_size(a).max(get_char_size(b));
                if y_diff > char_size * 0.5 {
                    a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
                }
            }
        });
        
        block.source_text = merge_text(&block.lines);
    }

    blocks
}
