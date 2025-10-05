use anyhow::{Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    prelude::*,
    style::Color,
    style::{Stylize, palette::tailwind},
    text::Line,
    widgets::{List, ListState, Paragraph, Tabs},
};
use steamlocate::error::LocateError;
use strum::{Display, EnumIter, FromRepr, IntoEnumIterator, IntoStaticStr, VariantNames};

use crate::{GameId, SaveManager, utils};

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
        // TODO: Uncomment when it's time
        /*
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
            */
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
        let instructions = Paragraph::new(Line::from(utils::format_keybindings(
            self.selected_tab.keybindings(),
        )))
        .centered();
        let selected = TabState::VARIANTS
            .iter()
            .position(|state| state == &self.selected_tab.tab().to_string());
        frame.render_widget(
            Tabs::new(TabState::VARIANTS.iter().copied())
                .divider("->")
                .select(selected),
            // .select(self.selected_tab.tab()),
            layout[0],
        );
        frame.render_widget(instructions, layout[0]);
        match self.selected_tab.tab() {
            TabState::Tab1 { .. } => self.selected_tab.render_tab1(
                frame,
                layout[1],
                &self
                    .save_swapper
                    .game_data()
                    .keys()
                    .copied()
                    // TODO: remove after Steam fetching is fixed
                    .chain([
                        GameId::Steam(516750),
                        GameId::Steam(367520),
                        GameId::Steam(730),
                    ])
                    .collect::<Vec<_>>(),
            ),
            TabState::Tab2 => self.selected_tab.render_tab2(frame, layout[1]),
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
        match (key.modifiers, key.code) {
            (_, KeyCode::Char('q'))
            | (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) => self.quit(),
            _ => {}
        }

        if self.selected_tab.on_key_event(key, &mut self.save_swapper) {
            self.quit();
        }
    }

    /// Set running to false to quit the application.
    fn quit(&mut self) {
        self.running = false;
    }
}

#[derive(Debug)]
struct SelectedTabContext {
    game_selection: ListState,
}
impl Default for SelectedTabContext {
    fn default() -> Self {
        Self {
            game_selection: ListState::default().with_selected(Some(0)),
        }
    }
}

#[derive(Debug)]
struct SelectedTab {
    ctx: SelectedTabContext,
    state: TabState,
}
impl Default for SelectedTab {
    fn default() -> Self {
        Self {
            ctx: Default::default(),
            state: Default::default(),
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
// TODO: change visbilities when save manager and utils have better paths
pub struct ActionKeyBinding {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

/// Information intended for user display on a key binding or bindings that all perform *one* action.
#[derive(Debug)]
// TODO: change visbilities when save manager and utils have better paths
pub struct ActionStyle {
    /// Intended display style of the description
    pub description_style: Style,
    /// Intended display style of the key character(s)
    pub key_style: Style,
}

#[derive(Debug, IntoStaticStr, Display, Hash, PartialEq, Eq)]
#[non_exhaustive]
// TODO: change visbilities when save manager and utils have better paths
pub enum Action {
    #[strum(serialize = "add game")]
    AddGame,
    #[strum(serialize = "delete game")]
    RemoveGame,
    // #[strum(serialize = "go back")]
    // Back,
    // #[strum(serialize = "quit")]
    // Quit,
}
impl Action {
    pub fn bindings(&self) -> &'static [ActionKeyBinding] {
        match self {
            Action::AddGame => &[ActionKeyBinding {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::NONE,
            }],
            Action::RemoveGame => &[
                ActionKeyBinding {
                    code: KeyCode::Delete,
                    modifiers: KeyModifiers::NONE,
                },
                ActionKeyBinding {
                    code: KeyCode::Char('d'),
                    modifiers: KeyModifiers::NONE,
                },
            ],
        }
    }
    pub fn display_style(&self) -> &'static ActionStyle {
        match self {
            Action::AddGame => {
                const {
                    &ActionStyle {
                        description_style: Style::new(),
                        key_style: Style::new().add_modifier(Modifier::ITALIC).fg(Color::Blue),
                    }
                }
            }
            Action::RemoveGame => {
                const {
                    &ActionStyle {
                        description_style: Style::new(),
                        key_style: Style::new()
                            .add_modifier(Modifier::ITALIC)
                            .fg(Color::LightRed),
                    }
                }
            } // Action::Back => {
              //     const {
              //         &ActionStyle {
              //             description_style: Style::new(),
              //             key_style: Style::new().add_modifier(Modifier::ITALIC).fg(Color::Cyan),
              //         }
              //     }
              // }
        }
    }
}

#[derive(Debug, Display, Clone, Copy, FromRepr, VariantNames)]
enum TabState {
    #[strum(to_string = "Games")]
    Tab1 { g_pressed: bool },
    #[strum(to_string = "Saves")]
    Tab2,
}

impl Default for TabState {
    fn default() -> Self {
        TabState::Tab1 {
            g_pressed: Default::default(),
        }
    }
}

impl TabState {
    /// Get the previous tab, if there is no previous tab return the current tab.
    fn previous(self) -> Self {
        match self {
            tab @ TabState::Tab1 { .. } => tab,
            TabState::Tab2 => TabState::Tab1 {
                g_pressed: Default::default(),
            },
        }
    }

    /// Get the next tab, if there is no next tab return the current tab.
    fn next(self) -> Self {
        match self {
            TabState::Tab1 { .. } => TabState::Tab2,
            tab @ TabState::Tab2 => tab,
        }
    }
}

impl SelectedTab {
    /// Get the previous tab, if there is no previous tab return the current tab.
    fn previous(self) -> Self {
        SelectedTab {
            ctx: self.ctx,
            state: self.state.previous(),
        }
    }

    /// Get the next tab, if there is no next tab return the current tab.
    fn next(self) -> Self {
        SelectedTab {
            ctx: self.ctx,
            state: self.state.next(),
        }
    }
    /// Return tab's name as a styled `Line`
    fn title(&self) -> Line<'static> {
        format!("  {}  ", self.state)
            .fg(tailwind::SLATE.c200)
            .bg(self.palette().c900)
            .into()
    }

    fn tab(&self) -> TabState {
        self.state
    }

    // Returns a list keybindings used in the current tab and its description
    fn keybindings(&self) -> &'static [Action] {
        match self.state {
            TabState::Tab1 { .. } => &[Action::AddGame, Action::RemoveGame],
            TabState::Tab2 => &[],
        }
    }

    fn render_tab1(&mut self, frame: &mut Frame, area: Rect, items: &[GameId]) {
        // let layout =
        //     Layout::vertical([Constraint::Percentage(5), Constraint::Percentage(95)]).split(area);
        // let instructions = Paragraph::new(Line::from(vec![
        //     Span::styled("a", Style::new().italic().light_blue()),
        //     Span::raw(": add game; "),
        //     Span::styled("d", Style::new().italic().light_red()),
        //     Span::raw(": delete game"),
        // ]))
        // .centered();
        let empty_message = Paragraph::new(Line::from(vec![
            Span::raw("No games found. Press "),
            // TODO: Lookup actions in keybindings cuz they might change
            // TODO: Use "->" symbol to separate tabs instead of "|" in the app code
            Span::styled("a", Style::new().italic().light_blue()),
            Span::raw(" to add a new one."),
        ]))
        .centered();
        // TODO: Title
        // frame.render_widget(instructions, layout[0]);
        // TODO: Show message/help if no games are addded, suggest Steam?
        if items.is_empty() {
            frame.render_widget(empty_message, area);
        } else {
            frame.render_stateful_widget(
                // FIXME: handle errors and change this because it's   taking   too   long
                List::new(
                    items
                        .iter()
                        // TODO: something other than to_string?
                        .map(|item| item.get_name().unwrap().to_string()),
                )
                .highlight_symbol(">>"),
                area,
                &mut self.ctx.game_selection,
            );
        }
    }

    fn render_tab2(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Paragraph::new("TODO"), area);
    }

    /// Handles keyboard events for corresponding tabs. Returns `true` if user requested to quit,
    /// otherwise false.
    fn on_key_event(&mut self, key: KeyEvent, save_manager: &mut SaveManager) -> bool {
        // Note: make sure to provide documentation for a binding in the `BindingHelp` struct
        // returned by `self.keybindings()`.
        let tab = &mut self.state;
        // TODO: Lookup actions in keybindings and check if they match
        match tab {
            TabState::Tab1 { g_pressed } => {
                let g_pressed_copy = *g_pressed;
                if let KeyCode::Char('g') = key.code {
                    *g_pressed = !*g_pressed;
                } else {
                    *g_pressed = false;
                }
                match (key.modifiers, key.code) {
                    (_, KeyCode::Left) | (_, KeyCode::Char('h')) | (_, KeyCode::Esc) => {
                        return true;
                    }
                    (_, KeyCode::Down) | (_, KeyCode::Char('j')) => {
                        self.ctx.game_selection.select_next();
                    }
                    (_, KeyCode::Up) | (_, KeyCode::Char('k')) => {
                        self.ctx.game_selection.select_previous();
                    }
                    (_, KeyCode::Right) | (_, KeyCode::Char('l')) | (_, KeyCode::Enter) => {
                        *tab = tab.next()
                    }
                    (_, KeyCode::Char('a')) => todo!(),
                    (_, KeyCode::Char('d')) | (_, KeyCode::Delete) => todo!(),
                    (_, KeyCode::Home) | (_, KeyCode::Char('G')) => {
                        self.ctx.game_selection.select_last();
                    }
                    (_, KeyCode::End) | (_, KeyCode::Char('g')) => {
                        println!("Pressed once");
                        if g_pressed_copy {
                            println!("it's true");
                            self.ctx.game_selection.select_first();
                        }
                    }
                    _ => {}
                }
            }

            TabState::Tab2 => match (key.modifiers, key.code) {
                (_, KeyCode::Left) | (_, KeyCode::Char('h')) | (_, KeyCode::Esc) => {
                    self.state = self.state.previous()
                }
                _ => {}
            },
        }
        false
    }

    const fn palette(&self) -> tailwind::Palette {
        match self.state {
            TabState::Tab1 { .. } => tailwind::BLUE,
            TabState::Tab2 => tailwind::EMERALD,
            // Self::Tab3 => tailwind::INDIGO,
            // Self::Tab4 => tailwind::RED,
        }
    }
}
