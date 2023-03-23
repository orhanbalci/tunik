use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use serde_json::Value;
use std::{
    error::Error,
    fs::{self, File},
    io,
    path::Path,
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;
use tunik::fuzzy_list::{FuzzyList, FuzzyListItem, FuzzyListState};

enum InputMode {
    Normal,
    Editing,
}

/// App holds the state of the application
struct App<'a> {
    /// Current value of the input box
    input: Input,
    /// Current input mode
    input_mode: InputMode,
    /// State of fuzzy list
    list_state: FuzzyListState<'a>,
}

impl<'a> Default for App<'a> {
    fn default() -> App<'a> {
        let country_data = App::get_country_data();
        let countries = country_data
            .into_iter()
            .flat_map(|(country, cities)| {
                let mut items = vec![];
                let style = Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::ITALIC);
                for city in cities.as_array().unwrap().iter() {
                    let content = vec![
                        Span::styled(city.as_str().unwrap().to_string(), style),
                        Span::raw(" - "),
                        Span::from(country.clone()),
                    ];
                    items.push(
                        FuzzyListItem::new(Spans::from(content))
                            .filter_style(Style::default().fg(Color::Blue)),
                    );
                }
                items
            })
            .collect();
        App {
            input: Input::default(),
            input_mode: InputMode::Normal,
            list_state: FuzzyListState::with_items(countries),
        }
    }
}

impl<'a> App<'a> {
    pub fn get_country_data() -> serde_json::Map<String, Value> {
        let country_data = fs::read_to_string("./assets/countries.json").unwrap();
        let json_country_data: Value = serde_json::from_str(&country_data).unwrap();
        json_country_data.as_object().unwrap().clone()
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let app = App::default();
    let res = run_app(&mut terminal, app);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
            match app.input_mode {
                InputMode::Normal => match key.code {
                    KeyCode::F(4) => {
                        app.input_mode = InputMode::Editing;
                    }
                    KeyCode::Char('q') => {
                        return Ok(());
                    }
                    KeyCode::Up => {
                        app.list_state.decrement_selected();
                    }
                    KeyCode::Down => {
                        app.list_state.increment_selected();
                    }
                    _ => {}
                },
                InputMode::Editing => match key.code {
                    KeyCode::Enter => {
                        // set filter here
                        app.list_state.set_filter(Some(app.input.value()));
                        app.input.reset();
                    }
                    KeyCode::Esc => {
                        app.input_mode = InputMode::Normal;
                    }
                    KeyCode::Up => {
                        app.list_state.decrement_selected();
                    }
                    KeyCode::Down => {
                        app.list_state.increment_selected();
                    }
                    _ => {
                        app.input.handle_event(&Event::Key(key));
                    }
                },
            }
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Min(1),
            ]
            .as_ref(),
        )
        .split(f.size());

    let (msg, style) = match app.input_mode {
        InputMode::Normal => (
            vec![
                Span::raw("Press "),
                Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to exit, "),
                Span::styled("F4", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to start filtering."),
            ],
            Style::default().add_modifier(Modifier::RAPID_BLINK),
        ),
        InputMode::Editing => (
            vec![
                Span::raw("Press "),
                Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to stop filtering, "),
                Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to filter list"),
            ],
            Style::default(),
        ),
    };
    let mut text = Text::from(Spans::from(msg));
    text.patch_style(style);
    let help_message = Paragraph::new(text);
    f.render_widget(help_message, chunks[0]);

    let width = chunks[0].width.max(3) - 3; // keep 2 for borders and 1 for cursor

    let scroll = app.input.visual_scroll(width as usize);
    let input = Paragraph::new(app.input.value())
        .style(match app.input_mode {
            InputMode::Normal => Style::default(),
            InputMode::Editing => Style::default().fg(Color::Yellow),
        })
        .scroll((0, scroll as u16))
        .block(Block::default().borders(Borders::ALL).title("Input"));
    f.render_widget(input, chunks[1]);
    match app.input_mode {
        InputMode::Normal =>
            // Hide the cursor. `Frame` does this by default, so we don't need to do anything here
            {}

        InputMode::Editing => {
            // Make the cursor visible and ask tui-rs to put it at the specified coordinates after rendering
            f.set_cursor(
                // Put cursor past the end of the input text
                chunks[1].x + ((app.input.visual_cursor()).max(scroll) - scroll) as u16 + 1,
                // Move one line down, from the border to the input line
                chunks[1].y + 1,
            )
        }
    }

    let cities_widget = FuzzyList::new(app.list_state.get_items())
        .block(Block::default().borders(Borders::ALL).title("Cities"))
        .highlight_style(Style::default().bg(Color::Red));
    f.render_stateful_widget(cities_widget, chunks[2], &mut app.list_state);
}
