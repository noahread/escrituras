use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use crate::app::{App, FlashcardPhase, FocusPane, FocusSubMode, InputMode, MemorizeMode, NavLevel, Screen, SearchFocus};
use escrituras_core::{Provider, Scripture};

/// Ensure the selected item in a list is visible by adjusting the ListState offset.
/// This clamps the offset to a valid range where the selected item is always visible.
fn ensure_selected_visible(state: &mut ListState, visible_height: usize) {
    // Need at least 1 visible row to make sense
    let visible_height = visible_height.max(1);

    if let Some(selected) = state.selected() {
        // Calculate the valid offset range where selected would be visible:
        // - min_offset: selected at bottom of visible area
        // - max_offset: selected at top of visible area
        let min_offset = selected.saturating_sub(visible_height - 1);
        let max_offset = selected;

        // Always clamp offset to valid range - this ensures selected is visible
        // regardless of how we got to the current offset
        let new_offset = state.offset().clamp(min_offset, max_offset);
        if new_offset != state.offset() {
            *state.offset_mut() = new_offset;
        }
    }
}

/// Wrap text to fit within a given width, returning multiple lines
/// Uses word boundaries for wrapping (doesn't break mid-word)
fn wrap_text_to_width(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_len = 0;

    for word in text.split_whitespace() {
        let word_len = word.chars().count();

        if current_len == 0 {
            // First word on line
            current_line = word.to_string();
            current_len = word_len;
        } else if current_len + 1 + word_len <= width {
            // Word fits on current line
            current_line.push(' ');
            current_line.push_str(word);
            current_len += 1 + word_len;
        } else {
            // Word doesn't fit, start new line
            lines.push(current_line);
            current_line = word.to_string();
            current_len = word_len;
        }
    }

    // Don't forget the last line
    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

/// Pre-calculated layout information for a single verse
struct VerseLayout {
    verse_idx: usize,       // Index into cached_verses
    start_line: usize,      // Global line number where this verse starts
    line_count: usize,      // Number of lines this verse occupies (including trailing blank)
    wrapped_lines: Vec<String>,  // Pre-wrapped text lines
}

/// Layout information for the entire chapter
struct ChapterLayout {
    verses: Vec<VerseLayout>,
    total_lines: usize,     // Total lines in the chapter
}

/// Calculate the line-based layout for all verses in a chapter
fn calculate_chapter_layout(verses: &[Scripture], width: usize) -> ChapterLayout {
    let mut layouts = Vec::with_capacity(verses.len());
    let mut current_line = 0;

    for (idx, verse) in verses.iter().enumerate() {
        // Calculate the verse number prefix width (e.g., "12  " = 4 chars)
        let num_prefix = format!("{}  ", verse.verse_number);
        let prefix_len = num_prefix.chars().count();

        // First line has less available width due to verse number prefix
        let first_line_width = width.saturating_sub(prefix_len);

        // Wrap the scripture text (without verse number)
        let mut wrapped = Vec::new();
        let text = &verse.scripture_text;

        if first_line_width > 0 && !text.is_empty() {
            // Wrap first line with reduced width
            let first_line_wrapped = wrap_text_to_width(text, first_line_width);
            if !first_line_wrapped.is_empty() {
                wrapped.push(first_line_wrapped[0].clone());

                // If there's remaining text after the first line, wrap it at full width
                if first_line_wrapped.len() > 1 {
                    // Reconstruct remaining text and rewrap at full width
                    let first_line_chars: usize = first_line_wrapped[0].chars().count();
                    let remaining: String = text.chars().skip(first_line_chars).collect();
                    let remaining = remaining.trim_start();
                    if !remaining.is_empty() {
                        let rest_wrapped = wrap_text_to_width(remaining, width);
                        wrapped.extend(rest_wrapped);
                    }
                }
            }
        } else if !text.is_empty() {
            // Fallback: just wrap at full width
            wrapped = wrap_text_to_width(text, width.max(1));
        }

        if wrapped.is_empty() {
            wrapped.push(String::new());
        }

        let line_count = wrapped.len() + 1; // +1 for blank line after verse

        layouts.push(VerseLayout {
            verse_idx: idx,
            start_line: current_line,
            line_count,
            wrapped_lines: wrapped,
        });

        current_line += line_count;
    }

    ChapterLayout {
        verses: layouts,
        total_lines: current_line,
    }
}

/// Calculate the optimal line_scroll position for a selected verse.
/// Uses lazy scrolling - only adjusts scroll when verse would go out of view.
fn calculate_scroll_for_verse(
    layout: &ChapterLayout,
    verse_idx: usize,
    view_height: usize,
    current_scroll: usize,
    _direction: crate::app::ScrollDirection,  // Kept for potential future use
    verse_line_offset: usize,
) -> usize {
    if verse_idx >= layout.verses.len() || view_height == 0 {
        return 0;
    }

    let verse = &layout.verses[verse_idx];
    let verse_start = verse.start_line + verse_line_offset;
    let verse_content_height = verse.wrapped_lines.len(); // Exclude trailing blank for visibility check
    let verse_end = verse.start_line + verse_content_height;

    // For verses taller than view, just show from the offset position
    if verse_content_height > view_height {
        return verse_start;
    }

    // Lazy scrolling: only scroll if verse is not fully visible
    let view_end = current_scroll + view_height;

    // If verse top is above view, scroll up to show verse at top
    if verse_start < current_scroll {
        return verse_start;
    }

    // If verse bottom is below view, scroll down to show verse at bottom
    if verse_end > view_end {
        return verse_end.saturating_sub(view_height);
    }

    // Verse is fully visible - keep current scroll position
    current_scroll
}

/// Parse a line of text and convert **bold** and *italic* markdown to styled spans
fn parse_markdown_line(text: &str) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut chars = text.char_indices().peekable();
    let mut current_text = String::new();

    while let Some((_, c)) = chars.next() {
        if c == '*' {
            // Check for ** (bold)
            if chars.peek().map(|(_, c)| *c) == Some('*') {
                // Consume the second *
                chars.next();

                // Push any accumulated plain text
                if !current_text.is_empty() {
                    spans.push(Span::raw(std::mem::take(&mut current_text)));
                }

                // Find closing **
                let mut bold_text = String::new();
                let mut found_close = false;

                while let Some((_, c)) = chars.next() {
                    if c == '*' && chars.peek().map(|(_, c)| *c) == Some('*') {
                        chars.next(); // consume second *
                        found_close = true;
                        break;
                    }
                    bold_text.push(c);
                }

                if found_close && !bold_text.is_empty() {
                    spans.push(Span::styled(
                        bold_text,
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                } else {
                    // No closing **, treat as literal
                    current_text.push_str("**");
                    current_text.push_str(&bold_text);
                }
            } else {
                // Single * - could be italic, but for now treat as literal
                current_text.push(c);
            }
        } else {
            current_text.push(c);
        }
    }

    // Push any remaining text
    if !current_text.is_empty() {
        spans.push(Span::raw(current_text));
    }

    if spans.is_empty() {
        Line::default()
    } else {
        Line::from(spans)
    }
}

pub fn render(app: &mut App, frame: &mut Frame) {
    let area = frame.area();

    // Main layout: header, body, footer
    let [header_area, body_area, footer_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);

    render_header(app, frame, header_area);

    match app.screen {
        Screen::Browse => render_browse_screen(app, frame, body_area),
        Screen::Search => render_search_screen(app, frame, body_area),
        Screen::Query => render_query_screen(app, frame, body_area),
        Screen::Focus => render_focus_screen(app, frame, body_area),
    }

    render_footer(app, frame, footer_area);

    // Render popups (in order of priority)
    if app.show_api_key_input {
        render_api_key_input(app, frame, area);
    } else if app.show_provider_picker {
        render_provider_picker(app, frame, area);
    } else if app.show_model_picker {
        render_model_picker(app, frame, area);
    }
}

fn render_header(app: &App, frame: &mut Frame, area: Rect) {
    let context_count = app.session_context_count();
    let context_indicator = if context_count > 0 {
        format!(" [{} saved]", context_count)
    } else {
        String::new()
    };

    let title = Line::from(vec![
        Span::styled(" Stick of Joseph, Stick of Judah ", Style::default().fg(Color::Cyan).bold()),
        Span::styled(context_indicator, Style::default().fg(Color::DarkGray)),
        Span::raw(" "),
        Span::styled(
            format!("v{}", env!("CARGO_PKG_VERSION")),
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    let header = Paragraph::new(title).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(header, area);
}

fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
    let mode_style = match app.input_mode {
        InputMode::Normal => Style::default().bg(Color::Blue).fg(Color::White),
        InputMode::Editing => Style::default().bg(Color::Yellow).fg(Color::Black),
    };

    let mode_text = match app.screen {
        Screen::Browse => " BROWSE ",
        Screen::Search => " SEARCH ",
        Screen::Query => " AI ",
        Screen::Focus => " FOCUS ",
    };

    // Key style: dark background with bright text for visibility on both light/dark terminals
    let key_style = Style::default().bg(Color::DarkGray).fg(Color::White);
    let label_style = Style::default().bg(Color::Black).fg(Color::White);

    let hints = match (app.screen, app.input_mode) {
        (Screen::Browse, InputMode::Normal) => {
            let mut hints = if app.focus == FocusPane::Content {
                if app.show_context_panel {
                    // Saved scriptures panel is showing
                    vec![
                        Span::styled(" j/k ", key_style),
                        Span::styled(" nav ", label_style),
                        Span::styled(" d ", key_style),
                        Span::styled(" remove ", label_style),
                    ]
                } else {
                    // Normal scripture content
                    vec![
                        Span::styled(" j/k ", key_style),
                        Span::styled(" verse ", label_style),
                        Span::styled(" c ", key_style),
                        Span::styled(" copy ", label_style),
                        Span::styled(" x ", key_style),
                        Span::styled(" save ", label_style),
                        Span::styled(" f ", key_style),
                        Span::styled(" focus ", label_style),
                        Span::styled(" s ", key_style),
                        Span::styled(" search ", label_style),
                    ]
                }
            } else {
                vec![
                    Span::styled(" j/k ", key_style),
                    Span::styled(" nav ", label_style),
                    Span::styled(" Enter ", key_style),
                    Span::styled(" select ", label_style),
                    Span::styled(" h ", key_style),
                    Span::styled(" back ", label_style),
                ]
            };
            // Common hints for Browse mode
            hints.extend(vec![
                Span::styled(" Tab ", key_style),
                Span::styled(" focus ", label_style),
                Span::styled(" X ", key_style),
                Span::styled(if app.show_context_panel { " scripture " } else { " saved " }, label_style),
                Span::styled(" / ", key_style),
                Span::styled(" search ", label_style),
                Span::styled(" a ", key_style),
                Span::styled(" AI ", label_style),
                Span::styled(" q ", key_style),
                Span::styled(" quit ", label_style),
            ]);
            hints
        },
        (Screen::Search, InputMode::Normal) => {
            let mut hints = vec![
                Span::styled(" j/k ", key_style),
                Span::styled(" nav ", label_style),
            ];

            if app.search_focus == SearchFocus::Results {
                hints.extend(vec![
                    Span::styled(" Enter ", key_style),
                    Span::styled(" view ", label_style),
                ]);
            } else if app.show_context_panel {
                // Preview focused, showing saved scriptures
                hints.extend(vec![
                    Span::styled(" d ", key_style),
                    Span::styled(" remove ", label_style),
                ]);
            } else {
                // Preview focused, showing preview
                hints.extend(vec![
                    Span::styled(" x ", key_style),
                    Span::styled(" save ", label_style),
                    Span::styled(" c ", key_style),
                    Span::styled(" copy ", label_style),
                    Span::styled(" f ", key_style),
                    Span::styled(" focus ", label_style),
                ]);
            }

            hints.extend(vec![
                Span::styled(" Tab ", key_style),
                Span::styled(" focus ", label_style),
                Span::styled(" X ", key_style),
                Span::styled(if app.show_context_panel { " scripture " } else { " saved " }, label_style),
                Span::styled(" i ", key_style),
                Span::styled(" edit ", label_style),
                Span::styled(" Esc ", key_style),
                Span::styled(" browse ", label_style),
            ]);
            hints
        },
        (Screen::Search, InputMode::Editing) => vec![
            Span::styled(" Enter ", key_style),
            Span::styled(" search ", label_style),
            Span::styled(" Esc ", key_style),
            Span::styled(" cancel ", label_style),
        ],
        (Screen::Query, InputMode::Normal) => {
            let mut hints = vec![
                Span::styled(" Tab ", key_style),
                Span::styled(" focus ", label_style),
            ];

            // Focus-specific hints
            match app.focus {
                FocusPane::Navigation => {
                    hints.extend(vec![
                        Span::styled(" j/k ", key_style),
                        Span::styled(" scroll ", label_style),
                    ]);
                }
                FocusPane::Content => {
                    hints.extend(vec![
                        Span::styled(" j/k ", key_style),
                        Span::styled(" nav ", label_style),
                    ]);
                    if app.show_context_panel {
                        hints.extend(vec![
                            Span::styled(" d ", key_style),
                            Span::styled(" remove ", label_style),
                        ]);
                    } else {
                        hints.extend(vec![
                            Span::styled(" c ", key_style),
                            Span::styled(" copy ", label_style),
                            Span::styled(" x ", key_style),
                            Span::styled(" save ", label_style),
                            Span::styled(" f ", key_style),
                            Span::styled(" focus ", label_style),
                        ]);
                    }
                }
                FocusPane::References => {
                    hints.extend(vec![
                        Span::styled(" j/k ", key_style),
                        Span::styled(" nav ", label_style),
                        Span::styled(" Enter ", key_style),
                        Span::styled(" jump ", label_style),
                    ]);
                }
                FocusPane::Input => {} // Handled by Editing mode
            }

            // Saved scriptures toggle hint
            hints.extend(vec![
                Span::styled(" X ", key_style),
                Span::styled(if app.show_context_panel { " scripture " } else { " saved " }, label_style),
            ]);
            // Provider and model picker hints
            hints.extend(vec![
                Span::styled(" P ", key_style),
                Span::styled(" provider ", label_style),
                Span::styled(" M ", key_style),
                Span::styled(" model ", label_style),
            ]);
            if !app.navigation_stack.is_empty() {
                hints.extend(vec![
                    Span::styled(" b ", key_style),
                    Span::styled(" back ", label_style),
                ]);
            }
            hints.extend(vec![
                Span::styled(" Esc ", key_style),
                Span::styled(" browse ", label_style),
            ]);
            hints
        },
        (Screen::Query, InputMode::Editing) => vec![
            Span::styled(" Enter ", key_style),
            Span::styled(" send ", label_style),
            Span::styled(" Esc ", key_style),
            Span::styled(" stop typing ", label_style),
        ],
        (Screen::Focus, InputMode::Normal) => {
            let mut hints = vec![
                Span::styled(" j/k ", key_style),
                Span::styled(" verse ", label_style),
                Span::styled(" c ", key_style),
                Span::styled(" copy ", label_style),
                Span::styled(" x ", key_style),
                Span::styled(" save ", label_style),
                Span::styled(" m ", key_style),
                Span::styled(" memorize ", label_style),
            ];

            // Add memorize-specific hints when in memorize mode
            if let Some(state) = &app.focus_state {
                if state.sub_mode == FocusSubMode::Memorize {
                    match state.memorize_mode {
                        MemorizeMode::Progressive => {
                            hints.extend(vec![
                                Span::styled(" +/- ", key_style),
                                Span::styled(" difficulty ", label_style),
                                Span::styled(" M ", key_style),
                                Span::styled(" mode ", label_style),
                            ]);
                        }
                        MemorizeMode::Flashcard => {
                            match state.flashcard_phase {
                                FlashcardPhase::Hidden => {
                                    hints.extend(vec![
                                        Span::styled(" Space ", key_style),
                                        Span::styled(" reveal ", label_style),
                                        Span::styled(" t ", key_style),
                                        Span::styled(" type ", label_style),
                                        Span::styled(" M ", key_style),
                                        Span::styled(" mode ", label_style),
                                    ]);
                                }
                                FlashcardPhase::Typing => {
                                    hints.clear();
                                    hints.extend(vec![
                                        Span::styled(" Enter ", key_style),
                                        Span::styled(" submit ", label_style),
                                        Span::styled(" Esc ", key_style),
                                        Span::styled(" cancel ", label_style),
                                    ]);
                                }
                                FlashcardPhase::Revealed => {
                                    hints.extend(vec![
                                        Span::styled(" r ", key_style),
                                        Span::styled(" reset ", label_style),
                                        Span::styled(" M ", key_style),
                                        Span::styled(" mode ", label_style),
                                    ]);
                                }
                            }
                        }
                    }
                }
            }

            // Only show exit hint if not in typing mode
            let in_typing = app.focus_state
                .as_ref()
                .map(|s| s.flashcard_phase == FlashcardPhase::Typing)
                .unwrap_or(false);
            if !in_typing {
                hints.extend(vec![
                    Span::styled(" Esc ", key_style),
                    Span::styled(" exit ", label_style),
                ]);
            }
            hints
        },
        _ => vec![],
    };

    let footer_content = Line::from(
        vec![
            Span::styled(mode_text, mode_style),
            Span::styled(" ", label_style),
        ]
        .into_iter()
        .chain(hints)
        .collect::<Vec<_>>(),
    );

    let footer = Paragraph::new(footer_content).style(Style::default().bg(Color::Black));
    frame.render_widget(footer, area);
}

fn render_browse_screen(app: &mut App, frame: &mut Frame, area: Rect) {
    // Split into navigation (left) and content (right)
    let [nav_area, content_area] = Layout::horizontal([
        Constraint::Length(30),
        Constraint::Min(0),
    ])
    .areas(area);

    // Store areas for mouse hit-testing
    app.nav_area = Some(nav_area);
    app.content_area = Some(content_area);
    app.refs_area = None;

    render_navigation(app, frame, nav_area);

    // Show saved scriptures panel or scripture content
    if app.show_context_panel {
        render_context_panel(app, frame, content_area);
    } else {
        render_content(app, frame, content_area);
    }
}

fn render_navigation(app: &mut App, frame: &mut Frame, area: Rect) {
    let nav_focused = app.focus == FocusPane::Navigation;
    let border_color = if nav_focused { Color::Cyan } else { Color::DarkGray };

    // Calculate visible height (subtract borders)
    let visible_height = area.height.saturating_sub(2) as usize;
    app.nav_visible_height = visible_height;

    // Get items, scroll reference, and selection for current nav level
    let (items, total, scroll, selected): (Vec<String>, usize, &mut usize, usize) = match app.nav_level {
        NavLevel::Volume => (
            app.cached_volumes.clone(),
            app.cached_volumes.len(),
            &mut app.volume_scroll,
            app.volume_state.selected().unwrap_or(0),
        ),
        NavLevel::Book => (
            app.cached_books.clone(),
            app.cached_books.len(),
            &mut app.book_scroll,
            app.book_state.selected().unwrap_or(0),
        ),
        NavLevel::Chapter => (
            app.cached_chapters.iter().map(|c| app.get_chapter_label(*c)).collect(),
            app.cached_chapters.len(),
            &mut app.chapter_scroll,
            app.chapter_state.selected().unwrap_or(0),
        ),
    };

    // Only adjust scroll when selected would go OUT of view
    // This is "lazy scrolling" - scroll stays put until necessary
    if visible_height > 0 && total > 0 {
        // If selected is above visible area, scroll up to show it at top
        if selected < *scroll {
            *scroll = selected;
        }
        // If selected is below visible area, scroll down to show it at bottom
        else if selected >= *scroll + visible_height {
            *scroll = selected - visible_height + 1;
        }
        // Clamp scroll to valid range (in case list shrank)
        let max_scroll = total.saturating_sub(visible_height);
        if *scroll > max_scroll {
            *scroll = max_scroll;
        }
    } else {
        *scroll = 0;
    }

    let scroll_val = *scroll;

    let title = format!(" {} ", app.current_nav_title());

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(title);

    // Calculate inner width for full-width highlighting (subtract borders)
    let inner_width = area.width.saturating_sub(2) as usize;

    // Build lines as plain text - no List widget, just a Paragraph
    // This gives us complete control and avoids any widget-specific behavior
    let end = (scroll_val + visible_height).min(total);
    let lines: Vec<Line> = if total > 0 && scroll_val < total {
        items[scroll_val..end]
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let actual_index = scroll_val + i;
                let text = format!("> {} ", v);
                // Pad to full width so background color fills the line
                let padded = format!("{:<width$}", text, width = inner_width);
                if actual_index == selected {
                    Line::styled(
                        padded,
                        Style::default()
                            .bg(Color::Blue)
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    )
                } else {
                    Line::raw(format!("  {} ", v))
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn render_content(app: &mut App, frame: &mut Frame, area: Rect) {
    let content_focused = app.focus == FocusPane::Content;
    let border_color = if content_focused { Color::Cyan } else { Color::DarkGray };

    let title = app.content_title();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(format!(" {} ", title));

    let inner_area = block.inner(area);
    app.content_height = inner_area.height;
    app.content_width = inner_area.width as usize;
    let view_height = inner_area.height as usize;
    let inner_width = inner_area.width as usize;

    if app.cached_verses.is_empty() {
        let placeholder = Paragraph::new("Select a chapter to view verses")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(placeholder, area);
        return;
    }

    // Calculate layout for all verses (pre-wrap all text)
    let layout = calculate_chapter_layout(&app.cached_verses, inner_width);
    app.total_content_lines = layout.total_lines as u16;

    // Determine selected verse
    let selected_idx = app.selected_verse_idx.unwrap_or(0);

    // Calculate optimal scroll position using lazy scrolling
    // Only adjusts if verse would go out of view
    let optimal_scroll = calculate_scroll_for_verse(
        &layout,
        selected_idx,
        view_height,
        app.line_scroll,  // Pass current scroll for lazy behavior
        app.last_scroll_direction,
        app.verse_line_offset,
    );
    app.line_scroll = optimal_scroll;

    // Determine the line range to render
    let scroll_start = app.line_scroll;
    let scroll_end = scroll_start + view_height;

    // Build visible lines
    let mut lines: Vec<Line> = Vec::new();

    for verse_layout in &layout.verses {
        let verse = &app.cached_verses[verse_layout.verse_idx];
        let verse_end_line = verse_layout.start_line + verse_layout.line_count;

        // Skip verses entirely above the view
        if verse_end_line <= scroll_start {
            continue;
        }
        // Stop when we're past the view
        if verse_layout.start_line >= scroll_end {
            break;
        }

        let is_cursor = app.selected_verse_idx == Some(verse_layout.verse_idx)
            && app.focus == FocusPane::Content;
        let is_in_range = app.selected_range.as_ref().is_some_and(|range| {
            range.book_title == verse.book_title
                && range.chapter_number == verse.chapter_number
                && range.contains_verse(verse.verse_number)
        });

        // Determine styles
        let verse_num_style = if is_cursor {
            Style::default().fg(Color::White).bg(Color::Blue).bold()
        } else if is_in_range {
            Style::default().fg(Color::Black).bg(Color::Cyan).bold()
        } else {
            Style::default().fg(Color::Yellow).bold()
        };

        let verse_text_style = if is_cursor {
            Style::default().fg(Color::White).bg(Color::Blue)
        } else if is_in_range {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };

        // Render each wrapped line that's visible
        for (line_idx, wrapped_line) in verse_layout.wrapped_lines.iter().enumerate() {
            let global_line = verse_layout.start_line + line_idx;

            if global_line >= scroll_start && global_line < scroll_end {
                let num_prefix = format!("{}  ", verse.verse_number);

                if line_idx == 0 {
                    // First line: prepend verse number
                    if is_cursor {
                        // Full line with verse number, padded for highlight
                        let full_line = format!("{}{}", num_prefix, wrapped_line);
                        let padded = format!("{:<width$}", full_line, width = inner_width);
                        lines.push(Line::styled(padded, verse_text_style));
                    } else {
                        // Verse number in yellow, text in default
                        lines.push(Line::from(vec![
                            Span::styled(num_prefix, verse_num_style),
                            Span::styled(wrapped_line.clone(), verse_text_style),
                        ]));
                    }
                } else {
                    // Continuation lines
                    if is_cursor {
                        let padded = format!("{:<width$}", wrapped_line, width = inner_width);
                        lines.push(Line::styled(padded, verse_text_style));
                    } else {
                        lines.push(Line::styled(wrapped_line.clone(), verse_text_style));
                    }
                }
            }
        }

        // Add blank line after verse (if visible)
        let blank_line_pos = verse_layout.start_line + verse_layout.wrapped_lines.len();
        if blank_line_pos >= scroll_start && blank_line_pos < scroll_end {
            lines.push(Line::default());
        }
    }

    // Render without Paragraph's internal wrapping (we did it manually)
    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn render_search_screen(app: &mut App, frame: &mut Frame, area: Rect) {
    // Layout: search input at top, results below split into list and preview
    let [input_area, results_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
    ])
    .areas(area);

    // Search input
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(
            if app.input_mode == InputMode::Editing {
                Color::Yellow
            } else {
                Color::DarkGray
            },
        ))
        .title(" Search ");

    let input = Paragraph::new(app.search_input.as_str())
        .style(Style::default().fg(Color::Cyan))
        .block(input_block);

    frame.render_widget(input, input_area);

    // Show cursor when editing
    if app.input_mode == InputMode::Editing {
        frame.set_cursor_position((
            input_area.x + app.search_input.len() as u16 + 1,
            input_area.y + 1,
        ));
    }

    // Results: list on left, preview/saved on right
    let [list_area, preview_area] = Layout::horizontal([
        Constraint::Percentage(40),
        Constraint::Percentage(60),
    ])
    .areas(results_area);

    // Results list - highlight when focused
    let results_focused = app.search_focus == SearchFocus::Results;
    let results_border_color = if results_focused { Color::Cyan } else { Color::DarkGray };

    let results_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(results_border_color))
        .title(format!(" Results ({}) ", app.search_results.len()));

    let items: Vec<ListItem> = app
        .search_results
        .iter()
        .map(|s| ListItem::new(format!(" {} ", s.verse_title)))
        .collect();

    let list = List::new(items)
        .block(results_block)
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    // Calculate and store visible height for offset management
    let visible_height = list_area.height.saturating_sub(2) as usize;
    app.search_visible_height = visible_height;
    ensure_selected_visible(&mut app.search_state, visible_height);

    frame.render_stateful_widget(list, list_area, &mut app.search_state);

    // Right panel: Preview or Saved Scriptures
    if app.show_context_panel {
        render_context_panel(app, frame, preview_area);
    } else {
        // Preview panel - highlight when focused
        let preview_focused = app.search_focus == SearchFocus::Preview;
        let preview_border_color = if preview_focused { Color::Cyan } else { Color::DarkGray };

        let preview_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(preview_border_color))
            .title(" Preview ");

        let preview_text = if let Some(i) = app.search_state.selected() {
            if let Some(scripture) = app.search_results.get(i) {
                Text::from(vec![
                    Line::from(Span::styled(
                        &scripture.verse_title,
                        Style::default().fg(Color::Yellow).bold(),
                    )),
                    Line::default(),
                    Line::from(&scripture.scripture_text[..]),
                ])
            } else {
                Text::from("Select a result to preview")
            }
        } else {
            Text::from("Select a result to preview")
        };

        let preview = Paragraph::new(preview_text)
            .block(preview_block)
            .wrap(Wrap { trim: true });

        frame.render_widget(preview, preview_area);
    }
}

fn render_query_screen(app: &mut App, frame: &mut Frame, area: Rect) {
    use escrituras_core::ChatRole;

    // Split layout: AI panel on left, scripture content on right (like browse)
    let [ai_area, content_area] = Layout::horizontal([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .areas(area);

    // Calculate references panel height (if we have references)
    let refs_height = if app.extracted_references.is_empty() {
        0
    } else {
        (app.extracted_references.len().min(5) + 2) as u16 // +2 for borders
    };

    // AI panel: chat history on top, references (if any), input at bottom
    let ai_layout = if refs_height > 0 {
        Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(refs_height),
            Constraint::Length(3),
        ])
        .split(ai_area)
    } else {
        Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(0),
            Constraint::Length(3),
        ])
        .split(ai_area)
    };
    let chat_area = ai_layout[0];
    let refs_area = ai_layout[1];
    let input_area = ai_layout[2];

    // Store areas for mouse hit-testing
    app.nav_area = Some(chat_area);
    app.content_area = Some(content_area);
    app.refs_area = if refs_height > 0 { Some(refs_area) } else { None };

    // Store chat area dimensions for scroll calculations (inner size minus borders)
    app.query_chat_height = chat_area.height.saturating_sub(2);
    app.query_chat_width = chat_area.width.saturating_sub(2);

    // Determine focus colors
    let ai_focused = app.focus == FocusPane::Navigation;
    let ai_border_color = if ai_focused { Color::Cyan } else { Color::DarkGray };

    // Chat history area - show provider and model
    let provider_name = match app.current_provider {
        Provider::Ollama => "Ollama",
        Provider::Claude => "Claude",
        Provider::OpenAI => "OpenAI",
    };
    let chat_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ai_border_color))
        .title(format!(" {}: {} ", provider_name, app.selected_model));

    let chat_text = if app.chat_messages.is_empty() && !app.query_loading {
        Text::from(Span::styled(
            "Ask a question about the scriptures...",
            Style::default().fg(Color::DarkGray),
        ))
    } else {
        let mut lines: Vec<Line> = Vec::new();

        for msg in &app.chat_messages {
            match msg.role {
                ChatRole::User => {
                    lines.push(Line::from(Span::styled(
                        "You:",
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    )));
                    lines.push(Line::from(msg.content.as_str()));
                    lines.push(Line::default());
                }
                ChatRole::Assistant => {
                    lines.push(Line::from(Span::styled(
                        "AI:",
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    )));
                    // Split response into lines and parse markdown
                    for line in msg.content.lines() {
                        lines.push(parse_markdown_line(line));
                    }
                    lines.push(Line::default());
                }
            }
        }

        if app.query_loading {
            lines.push(Line::from(Span::styled(
                "AI:",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )));
            // Animated ellipsis: cycles through ".", "..", "..."
            let dots = ".".repeat((app.animation_frame as usize) + 1);
            lines.push(Line::from(Span::styled(
                format!("Thinking{}", dots),
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            )));
        }

        Text::from(lines)
    };

    let chat = Paragraph::new(chat_text)
        .block(chat_block)
        .wrap(Wrap { trim: true })
        .scroll((app.query_scroll, 0));

    frame.render_widget(chat, chat_area);

    // Render references panel if we have any
    if !app.extracted_references.is_empty() && refs_area.height > 0 {
        let refs_focused = app.focus == FocusPane::References;
        let refs_border_color = if refs_focused { Color::Cyan } else { Color::Magenta };

        let refs_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(refs_border_color))
            .title(" References (Tab to focus, Enter to jump) ");

        let refs_items: Vec<ListItem> = app
            .extracted_references
            .iter()
            .enumerate()
            .map(|(i, range)| {
                ListItem::new(format!(" {}. {} ", i + 1, range.display_title()))
            })
            .collect();

        let refs_list = List::new(refs_items)
            .block(refs_block)
            .highlight_style(
                Style::default()
                    .bg(Color::Magenta)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        // Calculate and store visible height for offset management
        let visible_height = refs_area.height.saturating_sub(2) as usize;
        app.refs_visible_height = visible_height;
        ensure_selected_visible(&mut app.references_state, visible_height);

        frame.render_stateful_widget(refs_list, refs_area, &mut app.references_state);
    }

    // Query input at the bottom - highlight when focused or editing
    let input_focused = app.focus == FocusPane::Input;
    let input_border_color = if input_focused || app.input_mode == InputMode::Editing {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(input_border_color))
        .title(" Ask (Tab to focus) ");

    // Calculate visible portion of input with horizontal scrolling
    // Inner width = total width - 2 (for borders)
    let inner_width = input_area.width.saturating_sub(2) as usize;
    let cursor_pos = app.query_cursor;

    // Calculate scroll offset to keep cursor visible
    let scroll_offset = if inner_width == 0 {
        0
    } else if cursor_pos >= inner_width {
        cursor_pos - inner_width + 1
    } else {
        0
    };

    // Get the visible slice of the input
    let visible_text: String = app.query_input
        .chars()
        .skip(scroll_offset)
        .take(inner_width)
        .collect();

    // Use cyan text to match the "You:" style - visible in both light and dark terminals
    let input = Paragraph::new(visible_text)
        .style(Style::default().fg(Color::Cyan))
        .block(input_block);

    frame.render_widget(input, input_area);

    // Show cursor when editing
    if app.input_mode == InputMode::Editing {
        let cursor_x = (cursor_pos - scroll_offset) as u16;
        frame.set_cursor_position((
            input_area.x + cursor_x + 1,
            input_area.y + 1,
        ));
    }

    // Right side: Show scripture content or context panel
    if app.show_context_panel {
        render_context_panel(app, frame, content_area);
    } else {
        render_content(app, frame, content_area);
    }
}

fn render_context_panel(app: &mut App, frame: &mut Frame, area: Rect) {
    let content_focused = app.focus == FocusPane::Content;
    let border_color = if content_focused { Color::Cyan } else { Color::DarkGray };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(format!(" Saved Scriptures ({}) ", app.session_context.len()));

    if app.session_context.is_empty() {
        let placeholder = Paragraph::new("No saved scriptures.\nPress 'x' on a verse to save it.")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(placeholder, area);
        return;
    }

    let items: Vec<ListItem> = app
        .session_context
        .iter()
        .map(|v| {
            let preview: String = v.scripture_text.chars().take(60).collect();
            ListItem::new(vec![
                Line::from(Span::styled(
                    v.verse_title.clone(),
                    Style::default().fg(Color::Yellow).bold(),
                )),
                Line::from(format!("{}...", preview)),
            ])
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    // Each item is 2 lines, so calculate visible items accordingly
    let visible_items = (area.height.saturating_sub(2) / 2) as usize;
    app.context_visible_height = visible_items;
    ensure_selected_visible(&mut app.context_state, visible_items);

    frame.render_stateful_widget(list, area, &mut app.context_state);
}

fn render_model_picker(app: &mut App, frame: &mut Frame, area: Rect) {
    use ratatui::widgets::Clear;

    // Calculate popup size and position (centered)
    let popup_width = 40.min(area.width.saturating_sub(4));
    let popup_height = (app.available_models.len() as u16 + 2).min(area.height.saturating_sub(4));

    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Select Model (Enter to select, Esc to cancel) ");

    let items: Vec<ListItem> = app
        .available_models
        .iter()
        .map(|model| {
            let style = if model == &app.selected_model {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!(" {} ", model)).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, popup_area, &mut app.model_picker_state);
}

fn render_provider_picker(app: &mut App, frame: &mut Frame, area: Rect) {
    use ratatui::widgets::Clear;

    let providers = Provider::all();

    // Calculate popup size and position (centered)
    let popup_width = 45.min(area.width.saturating_sub(4));
    let popup_height = (providers.len() as u16 + 2).min(area.height.saturating_sub(4));

    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Select Provider ");

    let items: Vec<ListItem> = providers
        .iter()
        .map(|provider| {
            let key_source = app.get_key_source(*provider);
            let is_current = *provider == app.current_provider;

            let status = match key_source {
                Some("env") => "(env var)",
                Some("config") => "(configured)",
                Some("local") => "(local)",
                _ => "(needs key)",
            };
            let prefix = if is_current { "* " } else { "  " };

            let style = if is_current {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            } else if key_source.is_some() {
                Style::default()
            } else {
                Style::default().fg(Color::DarkGray)
            };

            ListItem::new(format!("{}{} {}", prefix, provider.display_name(), status)).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, popup_area, &mut app.provider_picker_state);
}

fn render_api_key_input(app: &App, frame: &mut Frame, area: Rect) {
    use ratatui::widgets::Clear;

    let provider_name = app.api_key_target_provider
        .map(|p| p.display_name())
        .unwrap_or("Provider");

    // Calculate popup size and position (centered)
    let popup_width = 60.min(area.width.saturating_sub(4));
    let popup_height = 7;

    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(format!(" Enter API Key for {} ", provider_name));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    // Instructions
    let instructions = Paragraph::new("Paste your API key below. Press Enter to save, Esc to cancel.")
        .style(Style::default().fg(Color::DarkGray));

    let instructions_area = Rect::new(inner.x, inner.y, inner.width, 1);
    frame.render_widget(instructions, instructions_area);

    // Input field
    let input_area = Rect::new(inner.x, inner.y + 2, inner.width, 1);

    // Mask the key with asterisks for security (show last 4 chars)
    let display_text = if app.api_key_input.is_empty() {
        String::new()
    } else if app.api_key_input.len() <= 4 {
        "*".repeat(app.api_key_input.len())
    } else {
        let masked_len = app.api_key_input.len() - 4;
        let last_four: String = app.api_key_input.chars().skip(masked_len).collect();
        format!("{}...{}", "*".repeat(masked_len.min(20)), last_four)
    };

    let input = Paragraph::new(display_text)
        .style(Style::default().fg(Color::Cyan));

    frame.render_widget(input, input_area);

    // Show cursor
    let cursor_x = app.api_key_input_cursor.min(input_area.width as usize) as u16;
    frame.set_cursor_position((input_area.x + cursor_x, input_area.y));

    // Status line
    let char_count = format!("{} characters", app.api_key_input.len());
    let status = Paragraph::new(char_count)
        .style(Style::default().fg(Color::DarkGray));

    let status_area = Rect::new(inner.x, inner.y + 4, inner.width, 1);
    frame.render_widget(status, status_area);
}

fn render_focus_screen(app: &mut App, frame: &mut Frame, area: Rect) {
    let Some(state) = &app.focus_state else {
        return;
    };

    // Calculate centered content area with margins
    // Cap line width at 80 chars for readability
    let max_width = 80u16;
    let content_width = area.width.min(max_width + 4); // +4 for borders
    let h_margin = (area.width.saturating_sub(content_width)) / 2;

    let centered_area = Rect::new(
        area.x + h_margin,
        area.y,
        content_width,
        area.height,
    );

    // Layout: Title (reference) and Content
    let [title_area, content_area] = Layout::vertical([
        Constraint::Length(3),  // Reference title
        Constraint::Min(5),     // Scripture content
    ])
    .areas(centered_area);

    // Render title (verse reference)
    let title_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let title_text = Paragraph::new(Line::from(vec![
        Span::styled(
            state.current_verse.verse_title.clone(),
            Style::default().fg(Color::Yellow).bold(),
        ),
    ]))
    .block(title_block)
    .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(title_text, title_area);

    // Determine content title based on mode
    let content_title = match state.sub_mode {
        FocusSubMode::Reading => " Scripture ".to_string(),
        FocusSubMode::Memorize => match state.memorize_mode {
            MemorizeMode::Progressive => format!(" Memorize (Level {}/5) ", state.memorize_level),
            MemorizeMode::Flashcard => match state.flashcard_phase {
                FlashcardPhase::Hidden => " Flashcard (hidden) ".to_string(),
                FlashcardPhase::Typing => " Flashcard (type your attempt) ".to_string(),
                FlashcardPhase::Revealed => " Flashcard (revealed) ".to_string(),
            },
        },
    };

    let content_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(content_title);

    // Handle special rendering for flashcard typing and revealed phases
    if state.sub_mode == FocusSubMode::Memorize
        && state.memorize_mode == MemorizeMode::Flashcard
    {
        match state.flashcard_phase {
            FlashcardPhase::Typing => {
                // Show typing input area
                let inner = content_block.inner(content_area);
                frame.render_widget(content_block, content_area);

                // Layout for input
                let [prompt_area, input_area] = Layout::vertical([
                    Constraint::Length(2),
                    Constraint::Min(3),
                ])
                .areas(inner);

                let prompt = Paragraph::new("Type the scripture from memory:")
                    .style(Style::default().fg(Color::DarkGray));
                frame.render_widget(prompt, prompt_area);

                let input = Paragraph::new(state.flashcard_input.as_str())
                    .style(Style::default().fg(Color::Cyan))
                    .wrap(Wrap { trim: true });
                frame.render_widget(input, input_area);

                // Show cursor - calculate position accounting for text wrapping
                let input_width = input_area.width as usize;
                let cursor_char_pos = state.flashcard_input_cursor;

                // Calculate which line and column the cursor is on after wrapping
                let (cursor_line, cursor_col) = if input_width > 0 {
                    // Walk through the text to find cursor position accounting for word wrap
                    let mut line = 0usize;
                    let mut col = 0usize;
                    let mut char_count = 0usize;

                    for word in state.flashcard_input.split_whitespace() {
                        let word_len = word.chars().count();

                        // Check if we need to wrap before this word
                        if col > 0 && col + 1 + word_len > input_width {
                            line += 1;
                            col = 0;
                        }

                        // Add space before word if not at start of line
                        if col > 0 {
                            if char_count == cursor_char_pos {
                                break; // Cursor is at the space
                            }
                            char_count += 1;
                            col += 1;
                        }

                        // Process each character in the word
                        for _ in word.chars() {
                            if char_count == cursor_char_pos {
                                break;
                            }
                            char_count += 1;
                            col += 1;
                        }

                        if char_count >= cursor_char_pos {
                            break;
                        }
                    }

                    (line, col)
                } else {
                    (0, cursor_char_pos)
                };

                let cursor_x = (cursor_col as u16).min(input_area.width.saturating_sub(1));
                let cursor_y = (cursor_line as u16).min(input_area.height.saturating_sub(1));
                frame.set_cursor_position((input_area.x + cursor_x, input_area.y + cursor_y));

                return;
            }
            FlashcardPhase::Revealed if !state.flashcard_input.is_empty() => {
                // Show diff between user's attempt and actual text
                let inner = content_block.inner(content_area);
                frame.render_widget(content_block, content_area);

                // Layout for diff display
                let [user_label_area, user_area, _spacer, actual_label_area, actual_area] = Layout::vertical([
                    Constraint::Length(1),
                    Constraint::Min(2),
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Min(2),
                ])
                .areas(inner);

                // User's attempt label
                let user_label = Paragraph::new("Your attempt:")
                    .style(Style::default().fg(Color::DarkGray));
                frame.render_widget(user_label, user_label_area);

                // Compute diff and render user's attempt with highlighting
                let diff_result = compute_word_diff(
                    &state.current_verse.scripture_text,
                    &state.flashcard_input,
                );

                let user_spans = render_diff_user_attempt(&diff_result);
                let user_text = Paragraph::new(Line::from(user_spans))
                    .wrap(Wrap { trim: true });
                frame.render_widget(user_text, user_area);

                // Actual text label
                let actual_label = Paragraph::new("Actual scripture:")
                    .style(Style::default().fg(Color::DarkGray));
                frame.render_widget(actual_label, actual_label_area);

                // Render actual text with missing words highlighted
                let actual_spans = render_diff_actual_text(&diff_result);
                let actual_text = Paragraph::new(Line::from(actual_spans))
                    .wrap(Wrap { trim: true });
                frame.render_widget(actual_text, actual_area);

                return;
            }
            _ => {
                // Fall through to default rendering
            }
        }
    }

    // Default content rendering with vertical centering and horizontal padding
    let content_text = match state.sub_mode {
        FocusSubMode::Reading => {
            state.current_verse.scripture_text.clone()
        }
        FocusSubMode::Memorize => {
            render_memorize_text(state)
        }
    };

    // Render the block first
    let inner = content_block.inner(content_area);
    frame.render_widget(content_block, content_area);

    // Add horizontal padding (4 chars on each side)
    let h_padding = 4u16;
    let padded_width = inner.width.saturating_sub(h_padding * 2);
    let padded_area = Rect::new(
        inner.x + h_padding,
        inner.y,
        padded_width,
        inner.height,
    );

    // Estimate text height for vertical centering
    // Count wrapped lines based on padded width
    let text_lines: usize = content_text
        .split_whitespace()
        .fold((0usize, 0usize), |(lines, line_len), word| {
            let word_len = word.chars().count() + 1; // +1 for space
            if line_len + word_len > padded_width as usize {
                (lines + 1, word_len)
            } else {
                (lines, line_len + word_len)
            }
        }).0 + 1; // +1 for the final line

    let text_height = text_lines as u16;
    let available_height = padded_area.height;
    let v_offset = available_height.saturating_sub(text_height) / 2;

    // Create vertically centered area
    let centered_text_area = Rect::new(
        padded_area.x,
        padded_area.y + v_offset,
        padded_area.width,
        padded_area.height.saturating_sub(v_offset),
    );

    let content = Paragraph::new(content_text)
        .wrap(Wrap { trim: true })
        .alignment(ratatui::layout::Alignment::Left);

    frame.render_widget(content, centered_text_area);
}

/// Represents the diff result between original and user text
#[derive(Debug)]
struct DiffResult {
    /// Words from original with their status
    original_words: Vec<(String, WordStatus)>,
    /// Words from user with their status
    user_words: Vec<(String, WordStatus)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum WordStatus {
    Correct,  // Word matches
    Missing,  // Word in original but not in user
    Wrong,    // Word in user but not matching original
}

/// Normalize a word for comparison (lowercase, strip punctuation)
fn normalize_word(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphabetic())
        .collect::<String>()
        .to_lowercase()
}

/// Compute LCS (Longest Common Subsequence) for word alignment
fn compute_lcs(original: &[String], user: &[String]) -> Vec<(usize, usize)> {
    let m = original.len();
    let n = user.len();

    // Build LCS table
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            if original[i - 1] == user[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // Backtrack to find matching pairs
    let mut matches = Vec::new();
    let mut i = m;
    let mut j = n;
    while i > 0 && j > 0 {
        if original[i - 1] == user[j - 1] {
            matches.push((i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] > dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }

    matches.reverse();
    matches
}

/// Compute word-by-word diff using LCS
fn compute_word_diff(original: &str, user: &str) -> DiffResult {
    // Split into words, preserving original forms
    let orig_words: Vec<&str> = original.split_whitespace().collect();
    let user_words: Vec<&str> = user.split_whitespace().collect();

    // Normalize for comparison
    let orig_normalized: Vec<String> = orig_words.iter().map(|w| normalize_word(w)).collect();
    let user_normalized: Vec<String> = user_words.iter().map(|w| normalize_word(w)).collect();

    // Find LCS matches
    let matches = compute_lcs(&orig_normalized, &user_normalized);
    let match_set_orig: std::collections::HashSet<usize> = matches.iter().map(|(o, _)| *o).collect();
    let match_set_user: std::collections::HashSet<usize> = matches.iter().map(|(_, u)| *u).collect();

    // Build result for original words
    let original_result: Vec<(String, WordStatus)> = orig_words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            if match_set_orig.contains(&i) {
                (w.to_string(), WordStatus::Correct)
            } else {
                (w.to_string(), WordStatus::Missing)
            }
        })
        .collect();

    // Build result for user words
    let user_result: Vec<(String, WordStatus)> = user_words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            if match_set_user.contains(&i) {
                (w.to_string(), WordStatus::Correct)
            } else {
                (w.to_string(), WordStatus::Wrong)
            }
        })
        .collect();

    DiffResult {
        original_words: original_result,
        user_words: user_result,
    }
}

/// Render user's attempt with diff highlighting
fn render_diff_user_attempt(diff: &DiffResult) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (i, (word, status)) in diff.user_words.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        let style = match status {
            WordStatus::Correct => Style::default().fg(Color::Green),
            WordStatus::Wrong => Style::default().fg(Color::Red),
            WordStatus::Missing => Style::default().fg(Color::Yellow), // Shouldn't happen for user
        };
        spans.push(Span::styled(word.clone(), style));
    }
    spans
}

/// Render actual text with missing words highlighted
fn render_diff_actual_text(diff: &DiffResult) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (i, (word, status)) in diff.original_words.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        let style = match status {
            WordStatus::Correct => Style::default().fg(Color::Green),
            WordStatus::Missing => Style::default().fg(Color::Yellow),
            WordStatus::Wrong => Style::default().fg(Color::Red), // Shouldn't happen for original
        };
        spans.push(Span::styled(word.clone(), style));
    }
    spans
}

/// Render text for memorization mode (simple version for Progressive mode)
fn render_memorize_text(state: &crate::app::FocusState) -> String {
    match state.memorize_mode {
        MemorizeMode::Progressive => {
            apply_progressive_hiding(&state.current_verse.scripture_text, state.memorize_level)
        }
        MemorizeMode::Flashcard => {
            match state.flashcard_phase {
                FlashcardPhase::Hidden => {
                    "(Press Space to reveal, or t to type your attempt)".to_string()
                }
                FlashcardPhase::Typing => {
                    // This is handled specially in render_focus_screen
                    String::new()
                }
                FlashcardPhase::Revealed => {
                    // This is handled specially in render_focus_screen for diff display
                    state.current_verse.scripture_text.clone()
                }
            }
        }
    }
}

/// Apply progressive word hiding based on difficulty level
/// Uses deterministic hash so words hidden at level N stay hidden at level N+1
/// Level 0: Full text
/// Level 1: Hide ~20% of words
/// Level 2: Hide ~40% of words
/// Level 3: Hide ~60% of words
/// Level 4: Hide ~80% of words
/// Level 5: All words hidden, first letter shown as hint
fn apply_progressive_hiding(text: &str, level: u8) -> String {
    if level == 0 {
        return text.to_string();
    }

    let threshold = level as f32 / 5.0; // 0.2, 0.4, 0.6, 0.8, 1.0
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut result = Vec::new();

    for (i, word) in words.iter().enumerate() {
        // Deterministic hash based on word position
        // This ensures words hidden at level N stay hidden at level N+1
        let hide_priority = ((i * 7919 + 104729) % 100) as f32 / 100.0;
        let should_hide = hide_priority < threshold;

        if should_hide {
            // Always show first letter, hide rest (replace with underscores)
            // Keep punctuation visible
            let hidden: String = word.chars().enumerate().map(|(j, c)| {
                if j == 0 || !c.is_alphabetic() {
                    c  // Keep first letter and punctuation
                } else {
                    '_'
                }
            }).collect();
            result.push(hidden);
        } else {
            result.push(word.to_string());
        }
    }

    result.join(" ")
}
