use std::collections::HashMap;

use anyhow::{Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    prelude::*,
    style::{Stylize, palette::tailwind},
    text::Line,
    widgets::{List, ListState, Paragraph, Tabs},
};
use steamlocate::error::LocateError;
use strum::{Display, EnumIter, FromRepr, IntoEnumIterator};

use crate::{GameId, SaveManager};

/// The main application which holds the state and logic of the application.
#[derive(Debug, Default)]
pub struct App {
    /// Is the application running?
    running: bool,
    save_swapper: SaveManager,
    selected_tab: SelectedTab,
    steam_located: bool,
}

impl App {
    /// Construct a new instance of [`App`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Run the application's main loop.
    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;
        // TODO: Steam installed popup
        if let Err(err) = self.save_swapper.load_steam_library() {
            match err.downcast::<steamlocate::Error>() {
                Ok(steamlocate::Error::FailedLocate(LocateError::Backend(_)))
                | Ok(steamlocate::Error::InvalidSteamDir(_)) => {
                    self.steam_located = false;
                }
                Ok(err) => bail!(err),
                Err(err) => bail!(err),
            }
        }
        while self.running {
            terminal.draw(|frame| self.render(frame))?;
            self.handle_crossterm_events()?;
        }
        Ok(())
    }

    /// Renders the user interface.
    ///
    /// This is where you add new widgets. See the following resources for more information:
    ///
    /// - <https://docs.rs/ratatui/latest/ratatui/widgets/index.html>
    /// - <https://github.com/ratatui/ratatui/tree/main/ratatui-widgets/examples>
    fn render(&mut self, frame: &mut Frame) {
        let layout =
            Layout::vertical(vec![Constraint::Length(1), Constraint::Min(20)]).split(frame.area());
        frame.render_widget(Tabs::new(Tab::iter().map(|tab| tab.to_string())), layout[0]);
        match self.selected_tab.tab() {
            Tab::Tab1 { .. } => self.selected_tab.render_tab1(
                frame,
                layout[1],
                &self
                    .save_swapper
                    .game_data()
                    .keys()
                    .copied()
                    .chain([
                        GameId::Steam(516750),
                        GameId::Steam(367520),
                        GameId::Steam(730),
                    ])
                    .collect::<Vec<_>>(),
            ),
            Tab::Tab2 => self.selected_tab.render_tab2(frame, layout[1]),
        };
    }

    /// Reads the crossterm events and updates the state of [`App`].
    ///
    /// If your application needs to perform work in between handling events, you can use the
    /// [`event::poll`] function to check if there are any events available with a timeout.
    fn handle_crossterm_events(&mut self) -> Result<()> {
        match event::read()? {
            // it's important to check KeyEventKind::Press to avoid handling key release events
            Event::Key(key) if key.kind == KeyEventKind::Press => self.on_key_event(key),
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
            _ => {}
        }
        Ok(())
    }

    /// Handles the key events and updates the state of [`App`].
    fn on_key_event(&mut self, key: KeyEvent) {
        self.selected_tab.on_key_event(key, &mut self.save_swapper);

        match (key.modifiers, key.code) {
            // TODO: arrows + hjkl to move menu up and down,
            (_, KeyCode::Esc | KeyCode::Char('q'))
            | (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) => self.quit(),
            // Add other key handlers here.
            _ => {}
        }
    }

    /// Set running to false to quit the application.
    fn quit(&mut self) {
        self.running = false;
    }
}

#[derive(Debug)]
struct SelectedTab {
    list_states: HashMap<Tab, ListState>,
    tab: Tab,
}
impl Default for SelectedTab {
    fn default() -> Self {
        Self {
            list_states: HashMap::from([(
                Tab::Tab1 {
                    g_pressed: Default::default(),
                },
                ListState::default().with_selected((items.len() > 0).then_some(0)),
            )]),
            tab: Default,
        }
    }
}

#[derive(Debug, Display, Hash, PartialEq, Eq, Clone, Copy, FromRepr, EnumIter)]
enum Tab {
    #[strum(to_string = "Games")]
    Tab1 { g_pressed: bool },
    #[strum(to_string = "Saves")]
    Tab2,
}
impl Default for Tab {
    fn default() -> Self {
        Tab::Tab1 {
            g_pressed: Default::default(),
        }
    }
}

impl Tab {
    /// Get the previous tab, if there is no previous tab return the current tab.
    fn previous(self) -> Self {
        match self {
            tab @ Tab::Tab1 { .. } => tab,
            Tab::Tab2 => Tab::Tab1 {
                g_pressed: Default::default(),
            },
        }
    }

    /// Get the next tab, if there is no next tab return the current tab.
    fn next(self) -> Self {
        match self {
            Tab::Tab1 { .. } => Tab::Tab2,
            tab @ Tab::Tab2 => tab,
        }
    }
}

impl SelectedTab {
    /// Get the previous tab, if there is no previous tab return the current tab.
    fn previous(mut self) -> Self {
        self.list_states.remove(&self.tab);
        SelectedTab {
            list_states: self.list_states,
            tab: self.tab.previous(),
        }
    }

    /// Get the next tab, if there is no next tab return the current tab.
    fn next(self) -> Self {
        SelectedTab {
            list_states: self.list_states,
            tab: self.tab.next(),
        }
    }
    /// Return tab's name as a styled `Line`
    fn title(&self) -> Line<'static> {
        format!("  {}  ", self.tab)
            .fg(tailwind::SLATE.c200)
            .bg(self.palette().c900)
            .into()
    }

    fn tab(&self) -> Tab {
        self.tab
    }

    fn render_tab1(&mut self, frame: &mut Frame, area: Rect, items: &[GameId]) {
        let state = self.list_states.get(&self.tab).unwrap();
        let layout =
            Layout::vertical([Constraint::Percentage(5), Constraint::Percentage(95)]).split(area);
        let instructions = Paragraph::new(Line::from(vec![
            Span::styled("a", Style::new().italic().light_blue()),
            Span::raw(": add game; "),
            Span::styled("d", Style::new().italic().light_red()),
            Span::raw(": delete game"),
        ]))
        .centered();
        let empty_message = Paragraph::new(Line::from(vec![
            Span::raw("No games found. Press "),
            Span::styled("a", Style::new().italic().light_blue()),
            Span::raw(" to add a new one."),
        ]))
        .centered();
        // TODO: Title
        frame.render_widget(instructions, layout[0]);
        // TODO: Show message/help if no games are addded.
        if items.is_empty() {
            frame.render_widget(empty_message, layout[1]);
        } else {
            frame.render_stateful_widget(
                // FIXME: handle errors and change this because it takes long
                List::new(
                    items
                        .iter()
                        .map(|item| item.get_name().unwrap().to_string()),
                )
                .highlight_symbol(">>"),
                layout[1],
                state,
            );
        }
    }

    fn render_tab2(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Paragraph::new("TODO"), area);
    }

    fn on_key_event(&mut self, key: KeyEvent, save_manager: &mut SaveManager) {
        // FIXME:
        let list_state = self.list_states.get_mut(&self.tab).unwrap();
        let tab = &mut self.tab;
        match tab {
            Tab::Tab1 { g_pressed } => {
                let g_pressed_copy = *g_pressed;
                if let KeyCode::Char('g') = key.code {
                    *g_pressed = !*g_pressed;
                } else {
                    *g_pressed = false;
                }
                match (key.modifiers, key.code) {
                    (_, KeyCode::Down) | (_, KeyCode::Char('j')) => {
                        list_state.select_next();
                    }
                    (_, KeyCode::Up) | (_, KeyCode::Char('k')) => {
                        list_state.select_previous();
                    }
                    (_, KeyCode::Right) | (_, KeyCode::Char('l')) | (_, KeyCode::Enter) => {
                        *tab = tab.next()
                    }
                    (_, KeyCode::Char('a')) => todo!(),
                    (_, KeyCode::Home) | (_, KeyCode::Char('G')) => {
                        list_state.select_last();
                    }
                    (_, KeyCode::End) | (_, KeyCode::Char('g')) => {
                        println!("Pressed once");
                        if g_pressed_copy {
                            println!("it's true");
                            list_state.select_first();
                        }
                    }
                    _ => {}
                }
            }

            Tab::Tab2 => match (key.modifiers, key.code) {
                (_, KeyCode::Left) | (_, KeyCode::Char('h')) => self.tab = self.tab.previous(),
                _ => {}
            },
        }
    }

    const fn palette(&self) -> tailwind::Palette {
        match self.tab {
            Tab::Tab1 { .. } => tailwind::BLUE,
            Tab::Tab2 => tailwind::EMERALD,
            // Self::Tab3 => tailwind::INDIGO,
            // Self::Tab4 => tailwind::RED,
        }
    }
}
