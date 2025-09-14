use std::collections::HashMap;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    prelude::*,
    style::{Stylize, palette::tailwind},
    text::Line,
    widgets::{List, ListState, Paragraph, Tabs},
};
use strum::{Display, EnumIter, FromRepr, IntoEnumIterator};

use crate::{GameId, SaveSwapper};

/// The main application which holds the state and logic of the application.
#[derive(Debug, Default)]
pub struct App {
    /// Is the application running?
    running: bool,
    save_swapper: SaveSwapper,
    selected_tab: SelectedTab,
}

impl App {
    /// Construct a new instance of [`App`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Run the application's main loop.
    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;
        // FIXME: move this to popup
        self.save_swapper.load_steam_library().unwrap();
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
                self.save_swapper.game_data().keys().copied().collect(),
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

    fn render_tab0(&mut self, frame: &mut Frame, area: Rect, items: Vec<GameId>) {
        let state = self.list_states.entry(self.tab).or_default();
        frame.render_stateful_widget(
            // FIXME: handle errors
            List::new(items.iter().map(|item| item.get_name().unwrap())),
            area,
            state,
        );
    }

    fn render_tab1(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Paragraph::new("TODO"), area);
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
