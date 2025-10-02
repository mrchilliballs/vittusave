use std::marker::PhantomData;

use anyhow::{Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use either::Either;
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    prelude::*,
    style::Stylize,
    text::Line,
    widgets::{List, ListState, Paragraph, TableState, Tabs},
};
use steamlocate::error::LocateError;

use crate::{GameId, SaveManager};

trait KeyHandler {
    type Dest;

    type DestState;
    fn on_key_event(self, key: KeyEvent) -> Either<Self, Self::Dest<Self::DestState>>
    where
        Self: Sized;
}

/// The main application which holds the state and logic of the application.
#[derive(Debug)]
pub struct App<T>
where
    T: CurrentTab,
    Tab<T>: StatefulWidget<State = TabState<T>>,
{
    /// Is the application running?
    running: bool,
    save_manager: SaveManager,
    tab_state: TabState<T>,
    steam_located: bool,
}

impl<T> App<T>
where
    T: CurrentTab,
    Tab<T>: StatefulWidget<State = TabState<T>>,
{
    /// Renders the user interface.
    ///
    /// This is where you add new widgets. See the following resources for more information:
    ///
    /// - <https://docs.rs/ratatui/latest/ratatui/widgets/index.html>
    /// - <https://github.com/ratatui/ratatui/tree/main/ratatui-widgets/examples>
    fn render(&mut self, frame: &mut Frame) {
        let layout =
            Layout::vertical(vec![Constraint::Length(1), Constraint::Min(20)]).split(frame.area());
        frame.render_widget(Tabs::new(Tab::<T>::default().tab_names()), layout[0]);

        let tab: Tab<T> = Tab::default();
        frame.render_stateful_widget(tab, layout[1], &mut self.tab_state);
    }
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
        }
    }
}

impl App<states::Tab1> {
    /// Construct a new instance of [`App`].
    pub fn new() -> Self {
        Self::default()
    }
}

impl<T> App<T>
where
    T: CurrentTab,
    Tab<T>: StatefulWidget<State = TabState<T>>,
    TabState<T>: KeyHandler,
    TabState<<TabState<T> as KeyHandler>::DestState>:
        KeyHandler<DestState: CurrentTab, Dest = TabState<<TabState<T> as KeyHandler>::DestState>>,
    Tab<T>: StatefulWidget<State = TabState<T>>,
    <TabState<T> as KeyHandler>::DestState: CurrentTab,
{
    /// Handles the key events and updates the state of [`App`].
    fn on_key_event(
        mut self,
        key: KeyEvent,
    ) -> Either<
        App<T>,
        App<<TabState<<TabState<T> as KeyHandler>::DestState> as KeyHandler>::DestState>,
    > {
        match (key.modifiers, key.code) {
            // TODO: arrows + hjkl to move menu up and down,
            (_, KeyCode::Esc | KeyCode::Char('q'))
            | (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) => self.quit(),
            // Add other key handlers here.
            _ => {}
        }

        match self.tab_state.on_key_event(key) {
            Either::Left(state) => Either::Left(Self {
                tab_state: state,
                running: self.running,
                save_manager: self.save_manager,
                steam_located: self.steam_located,
            }),
            Either::Right(state) => Either::Right(App::<
                <TabState<<TabState<T> as KeyHandler>::DestState> as KeyHandler>::DestState,
            > {
                tab_state: state,
                running: self.running,
                save_manager: self.save_manager,
                steam_located: self.steam_located,
            }),
        }
    }

    /// Reads the crossterm events and updates the state of [`App`].
    ///
    /// If your application needs to perform work in between handling events, you can use the
    /// [`event::poll`] function to check if there are any events available with a timeout.
    fn handle_crossterm_events(
        self,
    ) -> Result<Either<App<T>, App<<TabState<T> as KeyHandler>::DestState>>> {
        let event = event::read()?;
        // it's important to check KeyEventKind::Press to avoid handling key release events
        if let Event::Key(key) = event
            && key.kind == KeyEventKind::Press
        {
            Ok(self.on_key_event(key))
        } else {
            match event {
                Event::Mouse(_) => {}
                Event::Resize(_, _) => {}
                _ => {}
            }
            Ok(Either::Left(self))
        }
    }

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
        let mut app: Either<App<T>, App<<TabState<T> as KeyHandler>::DestState>> =
            Either::Left(self);
        loop {
            match &mut app {
                Either::Left(app) => {
                    if !app.running {
                        break;
                    }
                    terminal.draw(|frame| app.render(frame))?;
                }
                Either::Right(app) => {
                    if !app.running {
                        break;
                    }
                    terminal.draw(|frame| app.render(frame))?;
                }
            }
            match app {
                Either::Left(mut same_app) => {
                    app = same_app.handle_crossterm_events()?;
                }
                Either::Right(mut new_state_app) => {
                    app = new_state_app.handle_crossterm_events()?;
                }
            }
        }
        Ok(())
    }

    /// Set running to false to quit the application.
    fn quit(&mut self) {
        self.running = false;
    }
}

pub trait CurrentTab: private::Sealed + std::fmt::Display + std::fmt::Debug + Default {}
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

    #[derive(Debug, Default, Clone)]
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
    #[derive(Debug, Default, Clone)]
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

impl<T: CurrentTab> Tab<T> {
    fn tab_names(&self) -> Vec<String> {
        vec![
            states::Tab1::default().to_string(),
            states::Tab2::default().to_string(),
        ]
    }
}
impl<T: CurrentTab> Default for Tab<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}
default impl<T> KeyHandler for TabState<T>
where
    T: CurrentTab,
{
    type Dest = TabState<T>;

    type DestState = T;
    fn on_key_event(self, key: KeyEvent) -> Either<Self, Self::Dest>
    where
        Self: Sized,
    {
        let _ = key;
        Either::Left(self)
    }
}

impl KeyHandler for TabState<states::Tab1> {
    type Dest = TabState<Self::DestState>;

    type DestState = states::Tab2;
    fn on_key_event(mut self, key: KeyEvent) -> Either<Self, Self::Dest>
    where
        Self: Sized,
    {
        if let (_, KeyCode::Left) | (_, KeyCode::Enter) | (_, KeyCode::Char('l')) =
            (key.modifiers, key.code)
        {
            return match self.next() {
                Ok(state) => Either::Right(state),
                Err(state) => Either::Left(state),
            };
        } else {
            match (key.modifiers, key.code) {
                (_, KeyCode::Down) | (_, KeyCode::Char('j')) => {
                    self.ctx.selected_game.select_next()
                }
                (_, KeyCode::Up) | (_, KeyCode::Char('k')) => {
                    self.ctx.selected_game.select_previous()
                }
                (_, KeyCode::End) | (_, KeyCode::Char('G')) => self.ctx.selected_game.select_last(),
                (_, KeyCode::Home) | (_, KeyCode::Char('g')) => {
                    if self.state.g_pressed {
                        self.ctx.selected_game.select_last();
                    } else {
                        self.state.g_pressed = true;
                    }
                }
                (_, KeyCode::Char('a')) => todo!(),
                (_, _) => {}
            }
            self.state.g_pressed = false;
            Either::Left(self)
        }
    }
}
impl<T: CurrentTab> StatefulWidget for Tab<T> {
    type State = TabState<T>;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let (_, _, _) = (area, buf, state);
    }
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
