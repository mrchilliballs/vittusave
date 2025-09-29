use std::{collections::HashMap, marker::PhantomData};

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
#[derive(Debug)]
pub struct App<T: CurrentTab> {
    /// Is the application running?
    running: bool,
    save_manager: SaveManager,
    tab_state: TabState<T>,
    steam_located: bool,
    _tab: Tab<T>,
}
impl Default for App<states::Tab1> {
    fn default() -> Self {
        let save_manager: SaveManager = Default::default();
        let tab_state = TabState::new(
            save_manager
                .game_data()
                .keys()
                .copied()
                .chain([GameId::Steam(516750)])
                .chain([GameId::Steam(367520)])
                .collect::<Vec<_>>(),
        );

        Self {
            running: Default::default(),
            save_manager,
            steam_located: Default::default(),
            tab_state,
            _tab: Tab::default(),
        }
    }
}

impl App<states::Tab1> {
    /// Construct a new instance of [`App`].
    pub fn new() -> Self {
        Self::default()
    }
}

impl<T: CurrentTab> App<T> {
    /// Run the application's main loop.
    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;
        // TODO: Steam installed popup
        if let Err(err) = self.save_manager.load_steam_library() {
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
    fn render(&mut self, frame: &mut Frame)
    where
        Tab<T>: Default + StatefulWidget,
    {
        let tab: Tab<T> = Tab::default();

        let layout =
            Layout::vertical(vec![Constraint::Length(1), Constraint::Min(20)]).split(frame.area());
        frame.render_widget(Tabs::new(self.tab_state.tab_names()), layout[0]);
        frame.render_stateful_widget(tab, layout[1], &mut self.tab_state);

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
        self.tab_state.on_key_event(key, &mut self.save_manager);
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

pub trait CurrentTab: private::Sealed + std::fmt::Display + Default {}
pub mod states {
    use crate::GameId;

    use super::CurrentTab;

    macro_rules! impl_display {
        ($struct:ty, $str:expr) => {
            impl std::fmt::Display for $struct {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, $str)
                }
            }
        };
    }

    #[derive(Debug, Default)]
    pub struct Tab1 {
        pub g_pressed: bool,
        game_ids: Vec<GameId>,
    }
    impl Tab1 {
        pub fn new(game_ids: Vec<GameId>) -> Self {
            Self {
                game_ids,
                ..Default::default()
            }
        }
    }
    #[derive(Debug, Default)]
    pub struct Tab2;

    impl_display!(Tab1, "Games");
    impl_display!(Tab2, "Saves");

    impl CurrentTab for Tab1 {}
    impl CurrentTab for Tab2 {}
}

mod private {
    use super::states::*;

    pub trait Sealed {}

    impl Sealed for Tab1 {}
    impl Sealed for Tab2 {}
}

/// Tabs state machine
#[derive(Debug)]
struct Tab<T: CurrentTab>(PhantomData<T>);
impl Default for Tab<states::Tab1> {
    fn default() -> Self {
        Self(PhantomData)
    }
}
impl Tab<states::Tab1> {
    fn on_key_event(self, key_event: KeyEvent, state: &mut <Self as StatefulWidget>::State) {}
}

impl StatefulWidget for Tab<states::Tab1> {
    type State = TabState<states::Tab1>;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {}
}

#[derive(Debug, Default)]
struct TabContext {
    selected_game: ListState,
}

#[derive(Debug)]
struct TabState<T: CurrentTab> {
    ctx: TabContext,
    state: T,
}

// #[derive(Debug, Default, Display, Hash, PartialEq, Eq, Clone, Copy, FromRepr, EnumIter)]
// enum Tab {
//     #[default]
//     #[strum(to_string = "Games")]
//     Tab1,
//     #[strum(to_string = "Saves")]
//     Tab2,
// }
//
// impl Tab {
//     /// Get the previous tab, if there is no previous tab return the current tab.
//     fn previous(self) -> Self {
//         let current_index: usize = self as usize;
//         let previous_index = current_index.saturating_sub(1);
//         Self::from_repr(previous_index).unwrap_or(self)
//     }
//
//     /// Get the next tab, if there is no next tab return the current tab.
//     fn next(self) -> Self {
//         let current_index = self as usize;
//         let next_index = current_index.saturating_add(1);
//         Self::from_repr(next_index).unwrap_or(self)
//     }
// }

impl TabState<states::Tab1> {
    fn new(games: Vec<GameId>) -> Self {
        Self {
            ctx: Default::default(),
            state: states::Tab1::new(games),
        }
    }
}

impl<T: CurrentTab> TabState<T> {
    /// Get the previous tab, if there is no previous tab return the current tab.
    // fn previous(mut self) -> Self {
    //     todo!()
    // }

    /// Get the next tab, if there is no next tab return the current tab.

    fn tab_names(&self) -> Vec<String> {
        vec![
            states::Tab1::default().to_string(),
            states::Tab2::default().to_string(),
        ]
    }
}
impl TabState<states::Tab1> {
    fn render(&mut self, frame: &mut Frame, area: Rect, items: &[GameId]) {
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
                &mut self.ctx.selected_game,
            );
        }
    }
    fn on_key_event(&mut self, key: KeyEvent, save_manager: &mut SaveManager) {
        match (key.modifiers, key.code) {
            (_, KeyCode::Down) | (_, KeyCode::Char('j')) => self.ctx.selected_game.select_next(),
            (_, KeyCode::Up) | (_, KeyCode::Char('k')) => self.ctx.selected_game.select_previous(),
            (_, KeyCode::End) | (_, KeyCode::Char('G')) => self.ctx.selected_game.select_last(),
            (_, KeyCode::Home) | (_, KeyCode::Char('g')) => {
                if self.state.g_pressed {
                    self.ctx.selected_game.select_last();
                } else {
                    self.state.g_pressed = true;
                }
            }
            // TODO: l or enter to select, moves to next tab
            (_, KeyCode::Left) | (_, KeyCode::Enter) | (_, KeyCode::Char('l')) => self.next,
            (_, KeyCode::Char('a')) => todo!(),
            (_, _) => {}
        }
        self.state.g_pressed = false;
    }
    fn next(self) -> Result<TabState<states::Tab2>, Self> {
        if let None = self.ctx.selected_game.selected() {
            return Err(self);
        }
        Ok(TabState {
            ctx: self.ctx,
            state: states::Tab2::default(),
        })
    }
}

impl TabState<states::Tab2> {
    fn render(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Paragraph::new("TODO"), area);
    }
}

// /// A block surrounding the tab's content
// fn block(self) -> Block<'static> {
//     Block::bordered()
//         .border_set(symbols::border::PROPORTIONAL_TALL)
//         .padding(Padding::horizontal(1))
//         .border_style(self.palette().c700)
// }

//     const fn palette(&self) -> tailwind::Palette {
//         match self.tab {
//             Tab::Tab1 => tailwind::BLUE,
//             Tab::Tab2 => tailwind::EMERALD,
//             // Self::Tab3 => tailwind::INDIGO,
//             // Self::Tab4 => tailwind::RED,
//         }
//     }
// }
