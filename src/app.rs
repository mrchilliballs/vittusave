use std::collections::HashMap;

use anyhow::{Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Flex, Layout},
    prelude::*,
    style::{Stylize, palette::tailwind},
    text::Line,
    widgets::{List, ListState, Paragraph, Tabs},
};
use steamlocate::error::{LocateError, ValidationError};
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
            Tab::Tab1 => self.selected_tab.render_tab0(
                frame,
                layout[1],
                &self
                    .save_swapper
                    .game_data()
                    .keys()
                    .copied()
                    .chain([GameId::Steam(516750)])
                    .chain([GameId::Steam(367520)])
                    .collect::<Vec<_>>(),
            ),
            Tab::Tab2 => self.selected_tab.render_tab1(frame, frame.area()),
        };

        // impl Widget for &App {
        //     fn render(self, area: Rect, buf: &mut Buffer) {
        //         use Constraint::{Length, Min};
        //         let vertical = Layout::vertical([Length(1), Min(0), Length(1)]);
        //         let [header_area, inner_area, footer_area] = vertical.areas(area);
        //
        //         let horizontal = Layout::horizontal([Min(0), Length(20)]);
        //         let [tabs_area, title_area] = horizontal.areas(header_area);
        //
        //         render_title(title_area, buf);
        //         self.render_tabs(tabs_area, buf);
        //         self.selected_tab.render(inner_area, buf);
        //         render_footer(footer_area, buf);
        //     }
        // }

        // let title = Line::from("Ratatui Simple Template")
        //     .bold()
        //     .blue()
        //     .centered();
        // let text = "Hello, Ratatui!\n\n\
        //     Created using https://github.com/ratatui/templates\n\
        //     Press `Esc`, `Ctrl-C` or `q` to stop running.";
        // let list = List::new(self.)
        // frame.render_widget(
        //     Paragraph::new(text)
        //         .block(Block::bordered().title(title))
        //         .centered(),
        //     layout[0],
        // )
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

#[derive(Debug, Default)]
struct SelectedTab {
    list_states: HashMap<Tab, ListState>,
    tab: Tab,
}

#[derive(Debug, Default, Display, Hash, PartialEq, Eq, Clone, Copy, FromRepr, EnumIter)]
enum Tab {
    #[default]
    #[strum(to_string = "Games")]
    Tab1,
    #[strum(to_string = "Saves")]
    Tab2,
}

impl Tab {
    /// Get the previous tab, if there is no previous tab return the current tab.
    fn previous(self) -> Self {
        let current_index: usize = self as usize;
        let previous_index = current_index.saturating_sub(1);
        Self::from_repr(previous_index).unwrap_or(self)
    }

    /// Get the next tab, if there is no next tab return the current tab.
    fn next(self) -> Self {
        let current_index = self as usize;
        let next_index = current_index.saturating_add(1);
        Self::from_repr(next_index).unwrap_or(self)
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
        // if let Some(list_state) = &self.list_states.get(&self.tab)
        //     && let None = list_state.selected()
        // {
        //     return self;
        // }
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

    fn render_tab0(&mut self, frame: &mut Frame, area: Rect, items: &[GameId]) {
        let state = self
            .list_states
            .entry(self.tab)
            .or_insert_with(|| ListState::default().with_selected((items.len() > 0).then_some(0)));
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
                .highlight_style(Style::new().italic()),
                layout[1],
                state,
            );
        }
    }

    fn render_tab1(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Paragraph::new("TODO"), area);
    }

    fn on_key_event(&mut self, key: KeyEvent, save_manager: &mut SaveManager) {
        match (key.modifiers, key.code) {
            (_, KeyCode::Down) | (_, KeyCode::Char('j')) => match &self.tab {
                tab @ Tab::Tab1 => {
                    if let Some(list_state) = self.list_states.get_mut(&tab) {
                        println!("here");
                        list_state.select_next();
                    }
                }
                _ => {}
            },
            (_, KeyCode::Up) | (_, KeyCode::Char('h')) => match &self.tab {
                tab @ Tab::Tab1 => {
                    if let Some(list_state) = self.list_states.get_mut(&tab) {
                        list_state.select_previous();
                    }
                }
                _ => {}
            },
            // TODO: l or enter to select, moves to next tab
            (_, KeyCode::Char('a')) => match &self.tab {
                tab @ Tab::Tab1 => todo!(),
                _ => {}
            },
            _ => {}
        }
    }

    // /// A block surrounding the tab's content
    // fn block(self) -> Block<'static> {
    //     Block::bordered()
    //         .border_set(symbols::border::PROPORTIONAL_TALL)
    //         .padding(Padding::horizontal(1))
    //         .border_style(self.palette().c700)
    // }

    const fn palette(&self) -> tailwind::Palette {
        match self.tab {
            Tab::Tab1 => tailwind::BLUE,
            Tab::Tab2 => tailwind::EMERALD,
            // Self::Tab3 => tailwind::INDIGO,
            // Self::Tab4 => tailwind::RED,
        }
    }
}
