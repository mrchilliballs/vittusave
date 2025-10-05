use std::{fmt::Display, iter};

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    prelude::*,
    style::{Styled, palette::tailwind},
    widgets::{List, ListState, Paragraph, Tabs},
};
use strum::{Display, VariantNames};

use crate::{save_manager::GameId, save_manager::SaveManager};

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
pub struct SelectedTab {
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
struct ActionKeyBinding {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
#[non_exhaustive]
enum Action {
    AddGame,
    RemoveGame,
    // #[strum(serialize = "go back")]
    // Back,
    // #[strum(serialize = "quit")]
    // Quit,
}
impl Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Action::AddGame => "add game".add_modifier(Modifier::ITALIC).fg(Color::Blue),
                Action::RemoveGame => "delete game"
                    .add_modifier(Modifier::ITALIC)
                    .fg(Color::LightRed),
            }
        )
    }
}
impl Action {
    fn bindings(&self) -> &'static [ActionKeyBinding] {
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
    fn key_style(&self) -> Style {
        match self {
            Action::AddGame => Style::new().add_modifier(Modifier::ITALIC).fg(Color::Blue),
            Action::RemoveGame => Style::new()
                .add_modifier(Modifier::ITALIC)
                .fg(Color::LightRed),
            // Action::Back => {
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

#[derive(Debug, Display, Clone, Copy, VariantNames)]
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

    // Returns a list keybindings used in the current tab and its description
    fn keybindings(&self) -> &'static [Action] {
        match self.state {
            TabState::Tab1 { .. } => &[Action::AddGame, Action::RemoveGame],
            TabState::Tab2 => &[],
        }
    }

    // TODO: tests
    fn fmt_keybindings(&self) -> Vec<Span<'_>> {
        self.keybindings()
            .iter()
            .enumerate()
            .flat_map(|(i, action)| {
                let prev_comma = if i > 0 { "; " } else { "" };
                iter::once(Span::raw(prev_comma)).chain(
                    action
                        .bindings()
                        .iter()
                        .enumerate()
                        .flat_map(|(i, binding)| {
                            let prev_comma = if i > 0 { ", " } else { "" };
                            let span = if binding.modifiers == KeyModifiers::NONE {
                                Span::styled(
                                    format!("{}", &binding.code.to_string()),
                                    action.key_style(),
                                )
                            } else {
                                Span::styled(
                                    format!(
                                        "{prev_comma}{}+{}",
                                        &binding.modifiers.to_string(),
                                        &binding.code.to_string(),
                                    ),
                                    action.key_style(),
                                )
                            };
                            iter::once(Span::raw(prev_comma)).chain([span])
                        })
                        .chain([format!(": {}", &action.to_string()).into()]),
                )
            })
            .collect()
    }

    pub fn render_header(&self, frame: &mut Frame, area: Rect) {
        let instructions = Paragraph::new(Line::from(self.fmt_keybindings())).centered();
        let selected = TabState::VARIANTS
            .iter()
            .position(|state| state == &self.state.to_string());
        frame.render_widget(
            Tabs::new(TabState::VARIANTS.iter().copied())
                .divider("->")
                .select(selected),
            area,
        );
        frame.render_widget(instructions, area);
    }

    pub fn tab(&self) -> usize {
        TabState::VARIANTS
            .iter()
            .position(|state| state == &self.state.to_string())
            .unwrap()
    }

    pub fn render_tab0(&mut self, frame: &mut Frame, area: Rect, items: &[GameId]) {
        let add_game_action = self
            .keybindings()
            .iter()
            .find(|&&action| Action::AddGame == action)
            .expect("add game action should be defined in tab 1");
        let empty_message = Paragraph::new(Line::from(vec![
            Span::raw("No games found. Press "),
            Span::styled(
                add_game_action.to_string(),
                add_game_action.to_string().style(),
            ),
            Span::raw(" to add a new one."),
        ]))
        .centered();
        // TODO: Show message/help if no games are addded, suggest Steam?
        if items.is_empty() {
            frame.render_widget(empty_message, area);
        } else {
            frame.render_stateful_widget(
                List::new(
                    items
                        .iter()
                        // FIXME: handle errors and change this because it's   taking   too   long
                        .map(|item| item.get_name().unwrap().to_string()),
                )
                .highlight_symbol(">> "),
                area,
                &mut self.ctx.game_selection,
            );
        }
    }

    pub fn render_tab1(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Paragraph::new("TODO"), area);
    }

    /// Handles keyboard events for corresponding tabs. Returns `true` if user requested to quit,
    /// otherwise false.
    pub fn on_key_event(&mut self, key: KeyEvent, save_manager: &mut SaveManager) -> bool {
        let tab = &mut self.state;
        match tab {
            TabState::Tab1 { g_pressed } => {
                let g_pressed_copy = *g_pressed;
                if let KeyCode::Char('g') = key.code {
                    *g_pressed = !*g_pressed;
                } else {
                    *g_pressed = false;
                }
                match (key.modifiers, key.code) {
                    (_, KeyCode::Esc) => {
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
                    (_, KeyCode::End) | (_, KeyCode::Char('G')) => {
                        self.ctx.game_selection.select_last();
                    }
                    (_, KeyCode::Home) => {
                        self.ctx.game_selection.select_first();
                    }
                    (_, KeyCode::Char('g')) => {
                        if g_pressed_copy {
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

// TODO: test this module, e.g. `SelectedTab::fmt_keybindings`
