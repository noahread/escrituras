use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Wrap,
    },
};
use crate::app::{App, FocusPane, InputMode, NavLevel, Screen, SearchFocus};
use crate::provider::Provider;

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

    let title = app.current_nav_title();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(format!(" {} ", title));

    let items: Vec<ListItem> = match app.nav_level {
        NavLevel::Volume => app
            .cached_volumes
            .iter()
            .map(|v| ListItem::new(format!(" {} ", v)))
            .collect(),
        NavLevel::Book => app
            .cached_books
            .iter()
            .map(|b| ListItem::new(format!(" {} ", b)))
            .collect(),
        NavLevel::Chapter => app
            .cached_chapters
            .iter()
            .map(|c| ListItem::new(format!(" Chapter {} ", c)))
            .collect(),
    };

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    let state = match app.nav_level {
        NavLevel::Volume => &mut app.volume_state,
        NavLevel::Book => &mut app.book_state,
        NavLevel::Chapter => &mut app.chapter_state,
    };

    frame.render_stateful_widget(list, area, state);
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

    if app.cached_verses.is_empty() {
        let placeholder = Paragraph::new("Select a chapter to view verses")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(placeholder, area);
        return;
    }

    // Build verse text with formatting
    let mut lines: Vec<Line> = Vec::new();
    for (idx, verse) in app.cached_verses.iter().enumerate() {
        // Check if this verse is selected (either by manual selection or within a selected range)
        let is_cursor = app.selected_verse_idx == Some(idx) && app.focus == FocusPane::Content;
        let is_in_range = app.selected_range.as_ref().is_some_and(|range| {
            range.book_title == verse.book_title
                && range.chapter_number == verse.chapter_number
                && range.contains_verse(verse.verse_number)
        });
        let is_highlighted = is_cursor || is_in_range;

        let verse_num_style = if is_cursor {
            // Cursor position: bright highlight
            Style::default().fg(Color::Black).bg(Color::Yellow).bold()
        } else if is_in_range {
            // In range but not cursor: softer highlight
            Style::default().fg(Color::Black).bg(Color::Cyan).bold()
        } else {
            Style::default().fg(Color::Yellow).bold()
        };

        let verse_text_style = if is_highlighted {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };

        let verse_num = Span::styled(
            format!("{}  ", verse.verse_number),
            verse_num_style,
        );
        let verse_text = Span::styled(&verse.scripture_text[..], verse_text_style);
        lines.push(Line::from(vec![verse_num, verse_text]));
        lines.push(Line::default()); // Empty line between verses
    }

    app.total_content_lines = lines.len() as u16;

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: true })
        .scroll((app.content_scroll, 0));

    frame.render_widget(paragraph, area);

    // Render scrollbar
    if app.total_content_lines > app.content_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("^"))
            .end_symbol(Some("v"));

        let mut scrollbar_state = ScrollbarState::new(app.total_content_lines as usize)
            .position(app.content_scroll as usize);

        frame.render_stateful_widget(
            scrollbar,
            area.inner(ratatui::layout::Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
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
    use crate::app::ChatRole;

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
