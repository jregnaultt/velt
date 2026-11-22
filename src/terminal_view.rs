use crate::config::TerminalTheme;
use crate::pty::session::normalize_selection;
use crate::pty::TerminalSession;
use crate::theme::{self, Palette};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::{Color as TermColor, CursorShape, NamedColor};
use iced::event::Status;
use iced::font::Weight;
use iced::mouse;
use iced::widget::canvas::{self, Cache, Frame, Geometry, Path, Stroke, Text};
use iced::widget::{button, column, container, row, stack, text, Space};
use iced::{Alignment, Color, Element, Font, Length, Padding, Point, Rectangle, Renderer, Size, Theme};
use std::time::Instant;

const PADDING_X: f32 = 12.0;
const PADDING_Y: f32 = 10.0;

#[derive(Debug, Clone)]
pub enum TerminalMessage {
    ResizePane(usize, u16, u16),
    ScrollPane(usize, i32),
    SelectionStart(usize, usize, usize),
    SelectionUpdate(usize, usize, usize),
    SelectionEnd(usize),
    DoubleClickWord(usize, usize, usize),
    RightClickPane(usize),
}

#[derive(Default)]
pub struct CanvasState {
    dragging: bool,
    last_click: Option<(Instant, usize, usize)>,
}

pub struct TerminalCanvas<'a> {
    pub session: &'a TerminalSession,
    pub pane_index: usize,
    pub focused: bool,
    pub font_size: f32,
    pub font: Font,
    pub palette: Palette,
    pub selection: Option<((usize, usize), (usize, usize))>,
    pub cache: &'a Cache,
}

impl<'a> TerminalCanvas<'a> {
    fn metrics(&self) -> (f32, f32, f32) {
        let fs = self.font_size.clamp(10.0, 24.0);
        let line_height = (fs * 1.35).round();
        let char_width = fs * 0.60;
        (fs, line_height, char_width)
    }

    fn point_to_cell(&self, bounds: Rectangle, pos: Point) -> (usize, usize) {
        let (_, line_height, char_width) = self.metrics();
        let (cols, rows) = self.session.current_size();
        let rel_x = (pos.x - bounds.x - PADDING_X).max(0.0);
        let rel_y = (pos.y - bounds.y - PADDING_Y).max(0.0);
        let col = (rel_x / char_width).floor() as usize;
        let row = (rel_y / line_height).floor() as usize;
        (
            col.min(cols.saturating_sub(1) as usize),
            row.min(rows.saturating_sub(1) as usize),
        )
    }
}

impl<'a> canvas::Program<TerminalMessage> for TerminalCanvas<'a> {
    type State = CanvasState;

    fn update(
        &self,
        state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (Status, Option<TerminalMessage>) {
        let (_, line_height, char_width) = self.metrics();
        let usable_w = (bounds.width - PADDING_X * 2.0).max(char_width * 10.0);
        let usable_h = (bounds.height - PADDING_Y * 2.0).max(line_height * 4.0);
        let cols = (usable_w / char_width).floor().max(10.0) as u16;
        let rows = (usable_h / line_height).floor().max(4.0) as u16;
        let (cur_cols, cur_rows) = self.session.current_size();

        if (cols, rows) != (cur_cols, cur_rows) {
            return (
                Status::Ignored,
                Some(TerminalMessage::ResizePane(self.pane_index, cols, rows)),
            );
        }

        if let canvas::Event::Mouse(mouse_event) = event {
            match mouse_event {
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    if let Some(pos) = cursor.position_in(bounds) {
                        let abs_pos = Point::new(bounds.x + pos.x, bounds.y + pos.y);
                        let (col, row) = self.point_to_cell(bounds, abs_pos);
                        let now = Instant::now();
                        if let Some((prev_t, pc, pr)) = state.last_click {
                            if now.duration_since(prev_t).as_millis() <= 350 && pc == col && pr == row
                            {
                                state.last_click = None;
                                state.dragging = false;
                                return (
                                    Status::Captured,
                                    Some(TerminalMessage::DoubleClickWord(
                                        self.pane_index,
                                        col,
                                        row,
                                    )),
                                );
                            }
                        }
                        state.last_click = Some((now, col, row));
                        state.dragging = true;
                        return (
                            Status::Captured,
                            Some(TerminalMessage::SelectionStart(self.pane_index, col, row)),
                        );
                    }
                }
                mouse::Event::CursorMoved { .. } => {
                    if state.dragging {
                        if let Some(pos) = cursor.position_in(bounds) {
                            let abs_pos = Point::new(bounds.x + pos.x, bounds.y + pos.y);
                            let (col, row) = self.point_to_cell(bounds, abs_pos);
                            return (
                                Status::Captured,
                                Some(TerminalMessage::SelectionUpdate(self.pane_index, col, row)),
                            );
                        }
                    }
                }
                mouse::Event::ButtonReleased(mouse::Button::Left) => {
                    if state.dragging {
                        state.dragging = false;
                        return (
                            Status::Captured,
                            Some(TerminalMessage::SelectionEnd(self.pane_index)),
                        );
                    }
                }
                mouse::Event::ButtonPressed(mouse::Button::Right) => {
                    if cursor.is_over(bounds) {
                        return (
                            Status::Captured,
                            Some(TerminalMessage::RightClickPane(self.pane_index)),
                        );
                    }
                }
                mouse::Event::WheelScrolled { delta } => {
                    if cursor.is_over(bounds) {
                        let lines = match delta {
                            mouse::ScrollDelta::Lines { y, .. } => (y * 3.0).round() as i32,
                            mouse::ScrollDelta::Pixels { y, .. } => {
                                (y / line_height).round() as i32
                            }
                        };
                        if lines != 0 {
                            return (
                                Status::Captured,
                                Some(TerminalMessage::ScrollPane(self.pane_index, lines)),
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        (Status::Ignored, None)
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let (font_size, line_height, char_width) = self.metrics();
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            draw_term(
                frame,
                bounds.size(),
                self.session,
                self.focused,
                font_size,
                line_height,
                char_width,
                self.font,
                self.palette,
                self.selection,
            );
        });
        vec![geometry]
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::default()
        }
    }
}

fn cell_in_selection(
    col: usize,
    row: usize,
    sel: Option<((usize, usize), (usize, usize))>,
) -> bool {
    let Some((a, b)) = sel else {
        return false;
    };
    if a == b {
        return false;
    }
    let ((sc, sr), (ec, er)) = normalize_selection(a, b);
    if row < sr || row > er {
        return false;
    }
    if sr == er {
        col >= sc && col <= ec
    } else if row == sr {
        col >= sc
    } else if row == er {
        col <= ec
    } else {
        true
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_term(
    frame: &mut Frame,
    size: Size,
    session: &TerminalSession,
    focused: bool,
    font_size: f32,
    line_height: f32,
    char_width: f32,
    base_font: Font,
    palette: Palette,
    selection: Option<((usize, usize), (usize, usize))>,
) {
    frame.fill_rectangle(Point::ORIGIN, size, palette.bg);

    let Ok(term) = session.term.lock() else {
        return;
    };

    let content = term.renderable_content();
    let display_offset = content.display_offset as i32;
    let screen_lines = term.screen_lines() as i32;
    let cols = term.columns();

    // Draw selection background across entire grid range so empty trailing cells highlight too
    if let Some((a, b)) = selection {
        if a != b {
            for r in 0..screen_lines.max(0) as usize {
                for c in 0..cols {
                    if cell_in_selection(c, r, Some((a, b))) {
                        let sx = PADDING_X + c as f32 * char_width;
                        let sy = PADDING_Y + r as f32 * line_height;
                        frame.fill_rectangle(
                            Point::new(sx, sy),
                            Size::new(char_width + 0.5, line_height),
                            palette.selection,
                        );
                    }
                }
            }
        }
    }

    for indexed in content.display_iter {
        let point = indexed.point;
        let cell = indexed.cell;
        let row_idx = point.line.0 + display_offset;
        if row_idx < 0 || row_idx >= screen_lines {
            continue;
        }
        let col_idx = point.column.0;
        if col_idx >= cols {
            continue;
        }

        let x = PADDING_X + col_idx as f32 * char_width;
        let y = PADDING_Y + row_idx as f32 * line_height;

        let inverse = cell.flags.contains(Flags::INVERSE);
        let is_bold = cell.flags.contains(Flags::BOLD);
        let mut fg = resolve_color(cell.fg, is_bold, palette);
        let mut bg = resolve_color(cell.bg, false, palette);

        if inverse {
            std::mem::swap(&mut fg, &mut bg);
        }

        if bg != palette.bg {
            frame.fill_rectangle(Point::new(x, y), Size::new(char_width, line_height), bg);
        }

        let ch = cell.c;
        if ch != ' ' && ch != '\0' && !cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
            let mut cell_font = base_font;
            if is_bold {
                cell_font.weight = Weight::Bold;
            }
            frame.fill_text(Text {
                content: ch.to_string(),
                position: Point::new(x, y + 1.0),
                color: fg,
                size: font_size.into(),
                font: cell_font,
                ..Text::default()
            });
        }
    }

    // Draw cursor when viewport is at live bottom
    if display_offset == 0 && content.cursor.shape != CursorShape::Hidden {
        let c_point = content.cursor.point;
        let c_row = c_point.line.0;
        if c_row >= 0 && c_row < screen_lines {
            let cx = PADDING_X + c_point.column.0 as f32 * char_width;
            let cy = PADDING_Y + c_row as f32 * line_height;

            if focused {
                match content.cursor.shape {
                    CursorShape::Beam => {
                        frame.fill_rectangle(
                            Point::new(cx, cy + 1.0),
                            Size::new(2.0, line_height - 2.0),
                            palette.cursor,
                        );
                    }
                    CursorShape::Underline => {
                        frame.fill_rectangle(
                            Point::new(cx, cy + line_height - 3.0),
                            Size::new(char_width, 2.0),
                            palette.cursor,
                        );
                    }
                    _ => {
                        frame.fill_rectangle(
                            Point::new(cx, cy + 1.0),
                            Size::new(char_width, line_height - 2.0),
                            Color {
                                a: 0.72,
                                ..palette.cursor
                            },
                        );
                    }
                }
            } else {
                let rect = Path::rectangle(
                    Point::new(cx + 0.5, cy + 1.5),
                    Size::new(char_width - 1.0, line_height - 3.0),
                );
                frame.stroke(
                    &rect,
                    Stroke::default()
                        .with_color(theme::BORDER_ACTIVE)
                        .with_width(1.0),
                );
            }
        }
    }
}

fn resolve_color(color: TermColor, bold: bool, palette: Palette) -> Color {
    match color {
        TermColor::Named(named) => named_color(named, bold, palette),
        TermColor::Spec(rgb) => Color::from_rgb8(rgb.r, rgb.g, rgb.b),
        TermColor::Indexed(idx) => indexed_color(idx, palette),
    }
}

fn named_color(named: NamedColor, bold: bool, palette: Palette) -> Color {
    match named {
        NamedColor::Foreground => palette.fg,
        NamedColor::Background => palette.bg,
        NamedColor::Cursor => palette.cursor,
        NamedColor::Black => {
            if bold {
                Color::from_rgb8(0x52, 0x52, 0x5B)
            } else {
                Color::from_rgb8(0x18, 0x18, 0x1B)
            }
        }
        NamedColor::Red => palette.red,
        NamedColor::Green => palette.green,
        NamedColor::Yellow => palette.amber,
        NamedColor::Blue => palette.blue,
        NamedColor::Magenta => palette.magenta,
        NamedColor::Cyan => palette.cyan,
        NamedColor::White => {
            if bold {
                Color::from_rgb8(0xFA, 0xFA, 0xFA)
            } else {
                palette.fg
            }
        }
        NamedColor::BrightBlack => Color::from_rgb8(0x63, 0x63, 0x6E),
        NamedColor::BrightRed => Color::from_rgb8(0xF8, 0x71, 0x71),
        NamedColor::BrightGreen => Color::from_rgb8(0x4A, 0xDE, 0x80),
        NamedColor::BrightYellow => Color::from_rgb8(0xFB, 0xBF, 0x24),
        NamedColor::BrightBlue => Color::from_rgb8(0x60, 0xA5, 0xFA),
        NamedColor::BrightMagenta => Color::from_rgb8(0xC0, 0x84, 0xFC),
        NamedColor::BrightCyan => Color::from_rgb8(0x22, 0xD3, 0xEE),
        NamedColor::BrightWhite => Color::from_rgb8(0xFA, 0xFA, 0xFA),
        NamedColor::DimBlack => Color::from_rgb8(0x12, 0x12, 0x15),
        NamedColor::DimRed => Color::from_rgb8(0x99, 0x1B, 0x1B),
        NamedColor::DimGreen => Color::from_rgb8(0x16, 0x65, 0x34),
        NamedColor::DimYellow => Color::from_rgb8(0x92, 0x40, 0x0E),
        NamedColor::DimBlue => Color::from_rgb8(0x1E, 0x40, 0xAF),
        NamedColor::DimMagenta => Color::from_rgb8(0x6B, 0x21, 0xA8),
        NamedColor::DimCyan => Color::from_rgb8(0x15, 0x5E, 0x75),
        NamedColor::DimWhite | NamedColor::DimForeground => theme::TEXT_MUTED,
        NamedColor::BrightForeground => Color::from_rgb8(0xFA, 0xFA, 0xFA),
    }
}

fn indexed_color(idx: u8, palette: Palette) -> Color {
    match idx {
        0 => Color::from_rgb8(0x18, 0x18, 0x1B),
        1 => palette.red,
        2 => palette.green,
        3 => palette.amber,
        4 => palette.blue,
        5 => palette.magenta,
        6 => palette.cyan,
        7 => palette.fg,
        8 => Color::from_rgb8(0x52, 0x52, 0x5B),
        9 => Color::from_rgb8(0xF8, 0x71, 0x71),
        10 => Color::from_rgb8(0x4A, 0xDE, 0x80),
        11 => Color::from_rgb8(0xFB, 0xBF, 0x24),
        12 => Color::from_rgb8(0x60, 0xA5, 0xFA),
        13 => Color::from_rgb8(0xC0, 0x84, 0xFC),
        14 => Color::from_rgb8(0x22, 0xD3, 0xEE),
        15 => Color::from_rgb8(0xFA, 0xFA, 0xFA),
        16..=231 => {
            let n = idx - 16;
            let r = n / 36;
            let g = (n % 36) / 6;
            let b = n % 6;
            let to_u8 = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            Color::from_rgb8(to_u8(r), to_u8(g), to_u8(b))
        }
        232..=255 => {
            let shade = 8 + (idx - 232) * 10;
            Color::from_rgb8(shade, shade, shade)
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn view_terminal_pane<'a, M: 'a + Clone>(
    session: &'a TerminalSession,
    pane_index: usize,
    focused: bool,
    font_size: f32,
    font: Font,
    term_theme: TerminalTheme,
    selection: Option<((usize, usize), (usize, usize))>,
    toast: Option<&'a str>,
    cache: &'a Cache,
    map_msg: fn(TerminalMessage) -> M,
    on_jump_bottom: M,
    on_open_palette: M,
) -> Element<'a, M> {
    let palette = theme::terminal_palette(term_theme);
    let canvas_widget: Element<'a, TerminalMessage> = canvas::Canvas::new(TerminalCanvas {
        session,
        pane_index,
        focused,
        font_size,
        font,
        palette,
        selection,
        cache,
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into();
    let canvas_el: Element<'a, M> = canvas_widget.map(map_msg);

    let scrolled_up = session.display_offset() > 0;

    let terminal_area: Element<'a, M> = if scrolled_up {
        let jump_pill = container(
            button(
                row![
                    text("Jump to bottom (click)")
                        .size(12)
                        .color(theme::TEXT_PRIMARY),
                    text("v").size(11).color(theme::TEXT_MUTED),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([5, 12])
            .style(theme::card_button_style(false))
            .on_press(on_jump_bottom),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::End)
        .padding(Padding {
            top: 0.0,
            right: 0.0,
            bottom: 12.0,
            left: 0.0,
        });

        stack![canvas_el, jump_pill].into()
    } else {
        canvas_el
    };

    // Bottom HUD bar
    let (cols, rows) = session.current_size();
    let mut hud_left = row![
        container(
            text(&session.shell_label)
                .size(10)
                .color(if focused { palette.blue } else { theme::TEXT_MUTED }),
        )
        .padding([2, 6])
        .style(theme::card_style),
        text(&session.project_name)
            .size(11)
            .color(theme::TEXT_PRIMARY),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    if let Some(branch) = &session.git_branch {
        hud_left = hud_left.push(
            text(format!("git:{branch}"))
                .size(11)
                .color(palette.cyan),
        );
    }

    for tag in &session.tech_stack {
        hud_left = hud_left.push(
            container(text(*tag).size(10).color(theme::TEXT_MUTED))
                .padding([1, 5])
                .style(theme::card_style),
        );
    }

    let mut hud_right = row![].spacing(10).align_y(Alignment::Center);
    if focused {
        if let Some(msg) = toast {
            hud_right = hud_right.push(text(msg).size(11).color(palette.green));
        }
    }
    hud_right = hud_right.push(
        text(format!("{cols}x{rows}"))
            .size(10)
            .color(theme::TEXT_MUTED),
    );
    hud_right = hud_right.push(
        button(text("Ctrl+K").size(10).color(theme::TEXT_MUTED))
            .padding([2, 6])
            .style(theme::ghost_button_style(false))
            .on_press(on_open_palette),
    );

    let hud_bar = container(
        row![hud_left, Space::with_width(Length::Fill), hud_right]
            .align_y(Alignment::Center)
            .padding([0, 10]),
    )
    .width(Length::Fill)
    .height(26)
    .align_y(Alignment::Center)
    .style(theme::top_bar_style);

    container(
        column![terminal_area, hud_bar]
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(theme::pane_container_style(focused))
    .into()
}
