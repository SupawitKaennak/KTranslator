use crate::core::ports::{OcrTextBlock, OcrTextLine};

fn get_char_size(line: &OcrTextLine) -> f32 {
    // The height of a text line bounding box is generally the most reliable
    // indicator of its font size, especially for vertical manga text and CJK/Thai.
    // Avoid area calculation because it gets heavily skewed by line-length differences.
    line.h.min(line.w).max(12.0)
}

fn is_close(a: &OcrTextLine, b: &OcrTextLine, jp_merge_vertical: bool) -> bool {
    let char_size_a = get_char_size(a);
    let char_size_b = get_char_size(b);
    let char_size = char_size_a.max(char_size_b);

    // Check if the lines are likely part of vertical text (typical in Japanese Manga)
    let is_a_vertical = a.h > a.w * 1.2;
    let is_b_vertical = b.h > b.w * 1.2;
    let is_vertical_context = jp_merge_vertical && (is_a_vertical || is_b_vertical);

    if is_vertical_context {
        // --- CJK Vertical Text Context ---
        // 1. Horizontal Stack Case (separate vertical columns in the same bubble)
        let y_overlap_len = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
        let shorter_h = a.h.min(b.h);
        let has_y_overlap = y_overlap_len > 0.0 && (y_overlap_len / shorter_h) > 0.6; // Stricter Y overlap

        let x_gap = if a.x + a.w < b.x {
            b.x - (a.x + a.w)
        } else if b.x + b.w < a.x {
            a.x - (b.x + b.w)
        } else {
            0.0
        };
        let close_horizontally = x_gap < char_size * 0.8; // Stricter X gap

        if has_y_overlap && close_horizontally {
            // Extra column separation guard: if separate vertical columns are too far, don't merge
            let x_dist = (a.x - b.x).abs();
            if x_dist > char_size * 1.2 {
                // Stricter distance
                return false;
            }
            return true;
        }

        // 2. Same Column Segment Case (vertical splits inside a column)
        let x_overlap_len = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
        let narrower_w = a.w.min(b.w);
        let has_x_overlap = x_overlap_len > 0.0 && (x_overlap_len / narrower_w) > 0.6;

        let y_gap = if a.y + a.h < b.y {
            b.y - (a.y + a.h)
        } else if b.y + b.h < a.y {
            a.y - (b.y + b.h)
        } else {
            0.0
        };
        let close_vertically = y_gap < char_size * 1.0; // Stricter Y gap

        if has_x_overlap && close_vertically {
            return true;
        }
    } else {
        // --- Horizontal Text Context (English, Thai, Russian, etc.) ---
        // 1. Vertical Stack Case (standard lines stacked one below the other)
        let x_overlap_len = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
        let narrower_w = a.w.min(b.w);
        let has_x_overlap = x_overlap_len > 0.0 && (x_overlap_len / narrower_w) > 0.4; // Stricter horizontal alignment

        let y_gap = if a.y + a.h < b.y {
            b.y - (a.y + a.h)
        } else if b.y + b.h < a.y {
            a.y - (b.y + b.h)
        } else {
            0.0
        };
        let close_vertically = y_gap < char_size * 0.6; // Stricter vertical space in standard text

        if has_x_overlap && close_vertically {
            return true;
        }

        // 2. Same Line Segment Case (split words or horizontally adjacent inline text on the SAME line)
        let y_overlap_len = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
        let shorter_h = a.h.min(b.h);
        let has_y_overlap = y_overlap_len > 0.0 && (y_overlap_len / shorter_h) > 0.7; // Higher vertical alignment (same line)

        // Calculate horizontal overlap
        let x_overlap_len = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
        let has_x_overlap = x_overlap_len > 0.0;

        let x_gap = if a.x + a.w < b.x {
            b.x - (a.x + a.w)
        } else if b.x + b.w < a.x {
            a.x - (b.x + b.w)
        } else {
            0.0
        };

        // Strict inline checking:
        // 1. Must NOT have any horizontal overlap (if they overlap in X, they are separate overlapping columns/bubbles).
        // 2. The horizontal gap must be very small (less than 35% of the average character size).
        let close_horizontally = !has_x_overlap && x_gap > 0.0 && x_gap < char_size * 0.35; // Stricter gap

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
            (0x4E00..=0x9FFF).contains(&u) || // CJK Unified Ideographs
            (0x3040..=0x309F).contains(&u) || // Hiragana
            (0x30A0..=0x30FF).contains(&u) // Katakana
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
            let prev_trimmed = lines[..i]
                .iter()
                .rev()
                .map(|l| l.text.trim())
                .find(|s| !s.is_empty());

            let mut join_without_space = false;
            let mut remove_hyphen = false;
            if let Some(prev) = prev_trimmed {
                if prev.ends_with('-') {
                    join_without_space = true;
                    if !is_asian {
                        remove_hyphen = true;
                    }
                }
            }

            if join_without_space {
                if remove_hyphen && result.ends_with('-') {
                    result.pop(); // Remove the hyphen that was appended from the previous line
                }
                result.push_str(trimmed);
            } else {
                result.push_str(default_separator);
                result.push_str(trimmed);
            }
        }
    }

    result
}

pub fn build_blocks(
    lines: Vec<OcrTextLine>,
    smart_merge: bool,
    jp_merge_vertical: bool,
) -> Vec<OcrTextBlock> {
    if lines.is_empty() {
        return vec![];
    }

    if !smart_merge {
        return lines
            .into_iter()
            .map(|l| OcrTextBlock {
                source_text: l.text.clone(),
                lines: vec![l],
            })
            .collect();
    }

    let mut blocks: Vec<OcrTextBlock> = Vec::new();

    for line in lines {
        let mut matched_idx = None;
        for (i, block) in blocks.iter().enumerate() {
            if block
                .lines
                .iter()
                .any(|existing_line| is_close(&line, existing_line, jp_merge_vertical))
            {
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
        let avg_char_size = if block.lines.is_empty() {
            12.0
        } else {
            let sum: f32 = block.lines.iter().map(get_char_size).sum();
            sum / block.lines.len() as f32
        };
        let band_size = avg_char_size * 0.5;

        block.lines.sort_by(|a, b| {
            let is_a_vertical = a.h > a.w * 1.2;
            let is_b_vertical = b.h > b.w * 1.2;
            let is_vertical = is_a_vertical || is_b_vertical;

            if is_vertical {
                // Vertical CJK reading order: Right-to-Left (x descending) primarily, then top-to-bottom
                // Group X coordinates into vertical bands
                let a_band = (a.x / band_size).round() as i32;
                let b_band = (b.x / band_size).round() as i32;
                if a_band != b_band {
                    b_band.cmp(&a_band) // Descending
                } else {
                    a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
                }
            } else {
                // Horizontal reading order: Top-to-Bottom (y ascending) primarily, then left-to-right
                // Group Y coordinates into horizontal bands
                let a_band = (a.y / band_size).round() as i32;
                let b_band = (b.y / band_size).round() as i32;
                if a_band != b_band {
                    a_band.cmp(&b_band)
                } else {
                    a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
                }
            }
        });

        block.source_text = merge_text(&block.lines);
    }

    // Sort blocks by reading order to preserve dialogue flow
    blocks.sort_by(|a, b| {
        let (a_x1, a_y1, _a_x2, a_y2) = a.lines.iter().fold(
            (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
            |(min_x, min_y, max_x, max_y), l| {
                (min_x.min(l.x), min_y.min(l.y), max_x.max(l.x + l.w), max_y.max(l.y + l.h))
            },
        );
        let (b_x1, b_y1, _b_x2, b_y2) = b.lines.iter().fold(
            (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
            |(min_x, min_y, max_x, max_y), l| {
                (min_x.min(l.x), min_y.min(l.y), max_x.max(l.x + l.w), max_y.max(l.y + l.h))
            },
        );

        let a_h = a_y2 - a_y1;
        let b_h = b_y2 - b_y1;
        let band_size = a_h.min(b_h) * 0.4;
        let band_size = if band_size < 5.0 { 5.0 } else { band_size };

        let a_band = (a_y1 / band_size).round() as i32;
        let b_band = (b_y1 / band_size).round() as i32;

        if a_band != b_band {
            a_band.cmp(&b_band)
        } else {
            if jp_merge_vertical {
                b_x1.partial_cmp(&a_x1).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                a_x1.partial_cmp(&b_x1).unwrap_or(std::cmp::Ordering::Equal)
            }
        }
    });

    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_line(text: &str, x: f32, y: f32, w: f32, h: f32) -> OcrTextLine {
        OcrTextLine {
            text: text.to_string(),
            x,
            y,
            w,
            h,
            bubble_idx: None,
        }
    }

    #[test]
    fn test_get_char_size() {
        let line = create_line("Test", 10.0, 10.0, 100.0, 20.0);
        assert_eq!(get_char_size(&line), 20.0); // min of w and h, maxed with 12.0
    }

    #[test]
    fn test_is_close_horizontal_stack() {
        let a = create_line("Hello", 10.0, 10.0, 80.0, 15.0);
        let b = create_line("World", 12.0, 28.0, 80.0, 15.0); // Stacked vertically below 'a'
        
        // Should be close because they overlap horizontally and have small vertical gap
        assert!(is_close(&a, &b, false));
    }

    #[test]
    fn test_is_close_vertical_cjk() {
        let a = create_line("こ", 100.0, 10.0, 20.0, 80.0); // Vertical column 1
        let b = create_line("れ", 75.0, 12.0, 20.0, 80.0);  // Vertical column 2 (to the left)

        // Should be close in vertical context
        assert!(is_close(&a, &b, true));
    }

    #[test]
    fn test_merge_text_english() {
        let lines = vec![
            create_line("Hello", 0.0, 0.0, 50.0, 15.0),
            create_line("World", 0.0, 20.0, 50.0, 15.0),
        ];
        assert_eq!(merge_text(&lines), "Hello World");
    }

    #[test]
    fn test_merge_text_japanese() {
        let lines = vec![
            create_line("これは", 0.0, 0.0, 15.0, 50.0),
            create_line("テスト", 0.0, 60.0, 15.0, 50.0),
        ];
        assert_eq!(merge_text(&lines), "これはテスト"); // No spaces for Asian text
    }

    #[test]
    fn test_merge_text_hyphenation() {
        let lines = vec![
            create_line("DISAP-", 0.0, 0.0, 50.0, 15.0),
            create_line("PEAR!", 0.0, 20.0, 50.0, 15.0),
        ];
        assert_eq!(merge_text(&lines), "DISAPPEAR!"); // Joined and hyphen removed
    }

    #[test]
    fn test_build_blocks_no_merge() {
        let lines = vec![
            create_line("Line 1", 10.0, 10.0, 100.0, 15.0),
            create_line("Line 2", 10.0, 50.0, 100.0, 15.0),
        ];
        let blocks = build_blocks(lines, false, false);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].source_text, "Line 1");
        assert_eq!(blocks[1].source_text, "Line 2");
    }

    #[test]
    fn test_build_blocks_smart_merge() {
        let lines = vec![
            create_line("Line 1", 10.0, 10.0, 100.0, 15.0),
            create_line("Line 2", 12.0, 22.0, 100.0, 15.0), // Close to Line 1
        ];
        let blocks = build_blocks(lines, true, false);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].source_text, "Line 1 Line 2");
    }
}
