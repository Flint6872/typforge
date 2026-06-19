use docx_rs::*;
use std::io::Cursor;
use typst::layout::{Frame, FrameItem};
use typst::text::TextItemView;
use typst_layout::PagedDocument;

pub struct DocxOptions {
    pub title: Option<String>,
}

impl Default for DocxOptions {
    fn default() -> Self {
        Self { title: None }
    }
}

#[derive(Clone)]
struct PositionedText {
    x: f64,
    y: f64,
    text: String,
    size_pt: f64,
    color_hex: String,
}

pub fn docx(document: &PagedDocument, _options: &DocxOptions) -> Vec<u8> {
    let mut docx = Docx::new();
    eprintln!("--- typsdocx: Starting DOCX export ---");

    if document.pages().is_empty() {
        eprintln!("typsdocx: Document has no pages. Returning empty DOCX.");
        return vec![];
    }

    let mut total_paragraphs_added = 0;

    for (page_idx, page) in document.pages().iter().enumerate() {
        let page_w_pt = page.frame.width().to_pt();
        let page_h_pt = page.frame.height().to_pt();
        let w = (page_w_pt * 20.0) as u32;
        let h = (page_h_pt * 20.0) as u32;

        eprintln!(
            "typsdocx: Processing Page {} ({:.1}x{:.1} pts)...",
            page_idx, page_w_pt, page_h_pt
        );
        docx = docx.page_size(w, h);

        if page_idx > 0 {
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(Run::new().add_text(format!("--- Page {} Break ---", page_idx))),
            );
        } else {
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(Run::new().add_text(format!("--- Page {} ---", page_idx + 1))),
            );
        }

        let mut text_items = Vec::new();
        eprintln!("typsdocx: Collecting text items for Page {}...", page_idx);
        collect_text_items(&page.frame, 0.0, 0.0, &mut text_items);
        eprintln!(
            "typsdocx: Collected {} raw text items for Page {}.",
            text_items.len(),
            page_idx
        );

        if text_items.is_empty() {
            eprintln!("typsdocx: No text items collected for Page {}.", page_idx);
            continue; // Move to the next page if no text found
        }

        let mut lines: Vec<(f64, Vec<PositionedText>)> = Vec::new();
        eprintln!(
            "typsdocx: Grouping {} text items into lines for Page {}...",
            text_items.len(),
            page_idx
        );
        for item in text_items {
            let mut found = false;
            for (y_coord, line_items) in lines.iter_mut() {
                if (item.y - *y_coord).abs() < 2.0 {
                    line_items.push(item.clone());
                    found = true;
                    break;
                }
            }
            if !found {
                lines.push((item.y, vec![item]));
            }
        }
        eprintln!(
            "typsdocx: Grouped into {} lines for Page {}.",
            lines.len(),
            page_idx
        );

        lines.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        for (_, line_items) in lines.iter_mut() {
            line_items.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
        }

        eprintln!(
            "typsdocx: Rendering {} lines into paragraphs for Page {}...",
            lines.len(),
            page_idx
        );
        for (_, line_items) in lines {
            let mut p = Paragraph::new();
            for item in line_items {
                let run = Run::new()
                    .add_text(item.text)
                    .size((item.size_pt * 2.0) as usize)
                    .color(item.color_hex);
                p = p.add_run(run);
            }
            docx = docx.add_paragraph(p);
            total_paragraphs_added += 1;
        }
    }

    eprintln!(
        "typsdocx: Finished rendering pages. Total paragraphs added: {}",
        total_paragraphs_added
    );

    let mut buf = Vec::new();
    let mut cursor = Cursor::new(&mut buf);
    eprintln!("typsdocx: Packing DOCX document...");
    match docx.build().pack(&mut cursor) {
        Ok(_) => {
            eprintln!("typsdocx: Pack succeeded. Buffer size: {}", buf.len());
            if buf.is_empty() {
                eprintln!("typsdocx: Warning: DOCX buffer is empty after packing!");
            }
            buf
        }
        Err(e) => {
            eprintln!("typsdocx: DOCX packing failed: {:?}", e);
            vec![]
        }
    }
}

fn collect_text_items(
    frame: &Frame,
    base_x: f64,
    base_y: f64,
    collector: &mut Vec<PositionedText>,
) {
    // eprintln!("    Entering collect_text_items for frame with {} items.", frame.items().len());
    for (pos, item) in frame.items() {
        let abs_x = base_x + pos.x.to_pt();
        let abs_y = base_y + pos.y.to_pt();

        match item {
            FrameItem::Text(text) => {
                let view = TextItemView::full(text);
                let content: String = text
                    .glyphs
                    .iter()
                    .map(|g| view.glyph_text(g).to_string())
                    .collect();

                if content.trim().is_empty() {
                    // eprintln!("    Skipping empty text run.");
                    continue;
                }

                let color_hex = if let typst::visualize::Paint::Solid(color) = &text.fill {
                    let rgb = color.to_rgb();
                    format!(
                        "{:02x}{:02x}{:02x}",
                        (rgb.red * 255.0) as u8,
                        (rgb.green * 255.0) as u8,
                        (rgb.blue * 255.0) as u8
                    )
                } else {
                    "000000".to_string() // Default to black if not solid
                };

                collector.push(PositionedText {
                    x: abs_x,
                    y: abs_y,
                    text: content,
                    size_pt: text.size.to_pt(),
                    color_hex,
                });
            }
            FrameItem::Group(group) => {
                collect_text_items(&group.frame, abs_x, abs_y, collector);
            }
            // Explicitly ignore other FrameItem types for now
            _ => {}
        }
    }
}
