use crate::ai::usage::{format_compact, format_duration, UsageRange, UsageReport};
use crate::theme;
use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, Text};
use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Alignment, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme};

pub fn view_usage_dashboard<'a, M: 'a + Clone>(
    report: &'a UsageReport,
    range: UsageRange,
    selected_agent: Option<&'a str>,
    loading: bool,
    on_select_range: fn(UsageRange) -> M,
    on_toggle_agent: fn(String) -> M,
    on_refresh: M,
    on_close: M,
) -> Element<'a, M> {
    let subtitle = if loading {
        "Scanning local AI logs...".to_string()
    } else if let Some(agent) = selected_agent {
        format!("{} · filtered by {agent} (click again to clear)", report.range_label)
    } else {
        report.range_label.clone()
    };

    let header = row![
        column![
            text("Agent usage").size(18).color(theme::TEXT_PRIMARY),
            text(subtitle).size(12).color(theme::TEXT_MUTED),
        ]
        .spacing(3),
        Space::with_width(Length::Fill),
        button(text(if loading { "..." } else { "Refresh" }).size(12))
            .padding([5, 10])
            .style(theme::card_button_style(false))
            .on_press(on_refresh),
        Space::with_width(8),
        row![
            range_pill(
                "Today",
                range == UsageRange::Today,
                on_select_range(UsageRange::Today)
            ),
            range_pill(
                "7 days",
                range == UsageRange::Days7,
                on_select_range(UsageRange::Days7)
            ),
            range_pill(
                "30 days",
                range == UsageRange::Days30,
                on_select_range(UsageRange::Days30)
            ),
            range_pill(
                "90 days",
                range == UsageRange::Days90,
                on_select_range(UsageRange::Days90)
            ),
            range_pill(
                "All",
                range == UsageRange::All,
                on_select_range(UsageRange::All)
            ),
        ]
        .spacing(4),
        Space::with_width(12),
        button(text("esc").size(12).color(theme::TEXT_MUTED))
            .padding([5, 10])
            .style(theme::card_button_style(false))
            .on_press(on_close),
    ]
    .align_y(Alignment::Center);

    // KPI Sparkline values
    let inp_series: Vec<u64> = report.daily_series.iter().map(|d| d.input_tokens).collect();
    let out_series: Vec<u64> = report.daily_series.iter().map(|d| d.output_tokens).collect();
    let task_series: Vec<u64> = report.daily_series.iter().map(|d| d.tasks as u64).collect();
    let time_series: Vec<u64> = report.daily_series.iter().map(|d| d.duration_secs).collect();

    let cache_pct = if report.total_input_tokens > 0 {
        (report.total_cached_tokens as f64 / report.total_input_tokens as f64) * 100.0
    } else {
        0.0
    };
    let avg_out_per_turn = if report.total_turns > 0 {
        report.total_output_tokens / report.total_turns as u64
    } else {
        0
    };

    let kpi_row = row![
        kpi_card(
            "INPUT TOKENS",
            format_compact(report.total_input_tokens),
            format!("{:.0}% from cache", cache_pct),
            inp_series,
            Color::from_rgb8(0x71, 0x71, 0x7A),
        ),
        kpi_card(
            "OUTPUT TOKENS",
            format_compact(report.total_output_tokens),
            format!("{} per turn", format_compact(avg_out_per_turn)),
            out_series,
            theme::BLUE,
        ),
        kpi_card(
            "TASKS",
            report.total_tasks.to_string(),
            format!("{} turns", report.total_turns),
            task_series,
            theme::GREEN,
        ),
        kpi_card(
            "AGENT TIME",
            format_duration(report.total_agent_secs),
            format!("{} active days", report.active_days),
            time_series,
            theme::AMBER,
        ),
    ]
    .spacing(12);

    // Mid row: TOKENS BY DAY + WHEN THEY WORK
    let max_in = report
        .daily_series
        .iter()
        .map(|d| d.input_tokens)
        .max()
        .unwrap_or(0);
    let max_out = report
        .daily_series
        .iter()
        .map(|d| d.output_tokens)
        .max()
        .unwrap_or(0);

    let daily_chart: Element<'a, M> = canvas::Canvas::new(TokensByDayCanvas {
        buckets: report
            .daily_series
            .iter()
            .map(|b| (b.input_tokens, b.output_tokens))
            .collect(),
    })
    .width(Length::Fill)
    .height(140)
    .into();

    let first_label = if range == UsageRange::Today {
        "00h".to_string()
    } else {
        report
            .daily_series
            .first()
            .map(|d| d.label.clone())
            .unwrap_or_default()
    };
    let mid_label = if range == UsageRange::Today {
        "12h".to_string()
    } else {
        report
            .daily_series
            .get(report.daily_series.len() / 2)
            .map(|d| d.label.clone())
            .unwrap_or_default()
    };
    let end_label = if range == UsageRange::Today {
        "23h"
    } else {
        "today"
    };
    let chart_title = if range == UsageRange::Today {
        "TOKENS BY HOUR"
    } else {
        "TOKENS BY DAY"
    };
    let chart_right = if range == UsageRange::Today {
        format!(
            "{} tasks · {} turns today",
            report.total_tasks, report.total_turns
        )
    } else {
        format!(
            "{} of {} days with agents · {}/day",
            report.active_days,
            report.total_window_days,
            format_compact(report.avg_tokens_per_day)
        )
    };

    let heatmap_canvas: Element<'a, M> = canvas::Canvas::new(HeatmapCanvas {
        grid: report.heatmap,
    })
    .width(Length::Fill)
    .height(140)
    .into();

    let charts_row = row![
        container(
            column![
                row![
                    text(chart_title).size(11).color(theme::TEXT_MUTED),
                    Space::with_width(Length::Fill),
                    text(chart_right).size(11).color(theme::TEXT_MUTED),
                ],
                row![
                    column![
                        text(format!("{} in", format_compact(max_in)))
                            .size(10)
                            .color(theme::TEXT_MUTED),
                        Space::with_height(Length::Fill),
                        text(format!("{} out", format_compact(max_out)))
                            .size(10)
                            .color(theme::BLUE),
                    ]
                    .height(140)
                    .width(52),
                    daily_chart,
                ]
                .spacing(8),
                row![
                    Space::with_width(60),
                    text(first_label).size(10).color(theme::TEXT_MUTED),
                    Space::with_width(Length::Fill),
                    text(mid_label).size(10).color(theme::TEXT_MUTED),
                    Space::with_width(Length::Fill),
                    text(end_label).size(10).color(theme::TEXT_MUTED),
                ],
            ]
            .spacing(10)
            .padding(16),
        )
        .width(Length::FillPortion(6))
        .style(theme::card_style),
        container(
            column![
                row![
                    text("WHEN THEY WORK").size(11).color(theme::TEXT_MUTED),
                    Space::with_width(Length::Fill),
                    text(&report.most_active_label)
                        .size(11)
                        .color(theme::TEXT_MUTED),
                ],
                heatmap_canvas,
                row![
                    Space::with_width(18),
                    text("0").size(10).color(theme::TEXT_MUTED),
                    Space::with_width(Length::Fill),
                    text("6").size(10).color(theme::TEXT_MUTED),
                    Space::with_width(Length::Fill),
                    text("12").size(10).color(theme::TEXT_MUTED),
                    Space::with_width(Length::Fill),
                    text("18").size(10).color(theme::TEXT_MUTED),
                    Space::with_width(Length::Fill),
                    text("less -> more").size(10).color(theme::TEXT_MUTED),
                ],
            ]
            .spacing(10)
            .padding(16),
        )
        .width(Length::FillPortion(4))
        .style(theme::card_style),
    ]
    .spacing(12);

    // Bottom row: MODELS + AGENTS
    let provider_bar: Element<'a, M> = canvas::Canvas::new(StackedBarCanvas {
        segments: report
            .providers
            .iter()
            .map(|(name, pct)| (provider_color(name), *pct))
            .collect(),
    })
    .width(Length::Fill)
    .height(8)
    .into();

    let mut provider_legend = row![].spacing(12);
    for (name, pct) in &report.providers {
        provider_legend = provider_legend.push(
            row![
                text("o").size(8).color(provider_color(name)),
                text(format!("{name} {:.0}%", pct))
                    .size(11)
                    .color(theme::TEXT_MUTED),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        );
    }

    let max_model_tokens = report
        .models
        .iter()
        .map(|m| m.input_tokens + m.output_tokens)
        .max()
        .unwrap_or(1)
        .max(1);

    let mut models_col = column![
        row![
            text("MODELS").size(11).color(theme::TEXT_MUTED),
            Space::with_width(Length::Fill),
            text(format!("{} models", report.models.len()))
                .size(11)
                .color(theme::TEXT_MUTED),
        ],
        provider_bar,
        provider_legend,
        row![
            Space::with_width(Length::Fill),
            text("input").size(10).color(theme::TEXT_MUTED).width(54),
            text("output").size(10).color(theme::TEXT_MUTED).width(54),
            text("tasks").size(10).color(theme::TEXT_MUTED).width(42),
        ]
        .spacing(8),
    ]
    .spacing(8);

    if report.models.is_empty() {
        models_col = models_col.push(
            text("No model usage logs found in this time range.")
                .size(12)
                .color(theme::TEXT_MUTED),
        );
    } else {
        for m in report.models.iter().take(7) {
            let ratio = ((m.input_tokens + m.output_tokens) as f32 / max_model_tokens as f32)
                .clamp(0.02, 1.0);
            let bar: Element<'a, M> = canvas::Canvas::new(SingleBarCanvas {
                ratio,
                color: provider_color(&m.provider),
            })
            .width(Length::Fill)
            .height(6)
            .into();

            let label = if m.model.chars().count() > 24 {
                let t: String = m.model.chars().take(23).collect();
                format!("{t}...")
            } else {
                m.model.clone()
            };

            models_col = models_col.push(
                row![
                    text("o").size(8).color(provider_color(&m.provider)),
                    text(label)
                        .size(12)
                        .color(theme::TEXT_PRIMARY)
                        .width(155),
                    container(bar).width(Length::Fill).padding([4, 6]),
                    text(format_compact(m.input_tokens))
                        .size(12)
                        .color(theme::TEXT_PRIMARY)
                        .width(54),
                    text(format_compact(m.output_tokens))
                        .size(12)
                        .color(theme::TEXT_MUTED)
                        .width(54),
                    text(m.tasks.to_string())
                        .size(12)
                        .color(theme::TEXT_MUTED)
                        .width(42),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            );
        }
    }

    let max_agent_tokens = report
        .agents
        .iter()
        .map(|a| a.input_tokens + a.output_tokens)
        .max()
        .unwrap_or(1)
        .max(1);

    let mut agents_col = column![
        row![
            text("AGENTS").size(11).color(theme::TEXT_MUTED),
            Space::with_width(Length::Fill),
            text("click one to see only its usage")
                .size(11)
                .color(theme::TEXT_MUTED),
        ],
        row![
            Space::with_width(Length::Fill),
            text("input").size(10).color(theme::TEXT_MUTED).width(54),
            text("output").size(10).color(theme::TEXT_MUTED).width(54),
            text("time").size(10).color(theme::TEXT_MUTED).width(52),
        ]
        .spacing(8),
    ]
    .spacing(8);

    if report.agents.is_empty() {
        agents_col = agents_col.push(
            text("No local AI agents detected yet.")
                .size(12)
                .color(theme::TEXT_MUTED),
        );
    } else {
        for ag in &report.agents {
            let is_active_filter = selected_agent == Some(ag.agent.as_str());
            let total = ag.input_tokens + ag.output_tokens;
            let ratio = if total > 0 {
                (total as f32 / max_agent_tokens as f32).clamp(0.03, 1.0)
            } else {
                0.0
            };

            let bar: Element<'a, M> = canvas::Canvas::new(SingleBarCanvas {
                ratio,
                color: agent_color(&ag.agent),
            })
            .width(Length::Fill)
            .height(6)
            .into();

            let row_btn = button(
                row![
                    text("o").size(8).color(agent_color(&ag.agent)),
                    text(&ag.agent)
                        .size(12)
                        .color(theme::TEXT_PRIMARY)
                        .width(100),
                    container(bar).width(Length::Fill).padding([4, 6]),
                    text(if ag.input_tokens > 0 {
                        format_compact(ag.input_tokens)
                    } else {
                        "-".to_string()
                    })
                    .size(12)
                    .color(theme::TEXT_PRIMARY)
                    .width(54),
                    text(if ag.output_tokens > 0 {
                        format_compact(ag.output_tokens)
                    } else {
                        "-".to_string()
                    })
                    .size(12)
                    .color(theme::TEXT_MUTED)
                    .width(54),
                    text(if ag.duration_secs > 0 {
                        format_duration(ag.duration_secs)
                    } else {
                        "-".to_string()
                    })
                    .size(12)
                    .color(theme::TEXT_MUTED)
                    .width(52),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([5, 8])
            .style(theme::ghost_button_style(is_active_filter))
            .on_press(on_toggle_agent(ag.agent.clone()));

            agents_col = agents_col.push(row_btn);
        }
    }

    let bottom_row = row![
        container(models_col.padding(16))
            .width(Length::FillPortion(5))
            .style(theme::card_style),
        container(agents_col.padding(16))
            .width(Length::FillPortion(5))
            .style(theme::card_style),
    ]
    .spacing(12);

    let body = scrollable(
        column![header, kpi_row, charts_row, bottom_row]
            .spacing(16)
            .padding(24),
    )
    .height(Length::Fill)
    .style(theme::dark_scrollable_style);

    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::app_bg)
        .into()
}

fn provider_color(provider: &str) -> Color {
    match provider {
        "Anthropic" => theme::ORANGE,
        "Google" => theme::BLUE,
        "OpenAI" => theme::GREEN,
        "xAI" => theme::AMBER,
        _ => theme::TEXT_MUTED,
    }
}

fn agent_color(agent: &str) -> Color {
    match agent {
        "Antigravity" => theme::BLUE,
        "Claude Code" => theme::ORANGE,
        "Codex" => theme::GREEN,
        "Grok CLI" => theme::AMBER,
        _ => theme::TEXT_MUTED,
    }
}

fn range_pill<'a, M: 'a + Clone>(label: &'a str, active: bool, on_press: M) -> Element<'a, M> {
    button(text(label).size(12))
        .padding([5, 10])
        .style(theme::card_button_style(active))
        .on_press(on_press)
        .into()
}

fn kpi_card<'a, M: 'a>(
    title: &'a str,
    value: String,
    subtitle: String,
    series: Vec<u64>,
    bar_color: Color,
) -> Element<'a, M> {
    let spark: Element<'a, M> = canvas::Canvas::new(SparklineCanvas { series, bar_color })
        .width(Length::Fill)
        .height(26)
        .into();

    container(
        column![
            text(title).size(11).color(theme::TEXT_MUTED),
            text(value).size(26).color(theme::TEXT_PRIMARY),
            text(subtitle).size(11).color(theme::TEXT_MUTED),
            Space::with_height(4),
            spark,
        ]
        .spacing(4)
        .padding(14),
    )
    .width(Length::Fill)
    .style(theme::card_style)
    .into()
}

struct SparklineCanvas {
    series: Vec<u64>,
    bar_color: Color,
}

impl<M> canvas::Program<M> for SparklineCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let n = self.series.len();
        if n == 0 {
            return vec![frame.into_geometry()];
        }
        let max_v = self.series.iter().copied().max().unwrap_or(1).max(1) as f32;
        let slot_w = bounds.width / n as f32;
        let bar_w = (slot_w * 0.65).clamp(2.0, 8.0);

        for (i, &val) in self.series.iter().enumerate() {
            let h = if val == 0 {
                2.0
            } else {
                ((val as f32 / max_v) * (bounds.height - 2.0)).max(3.0)
            };
            let x = i as f32 * slot_w + (slot_w - bar_w) * 0.5;
            let y = bounds.height - h;
            let color = if val == 0 {
                theme::BORDER
            } else {
                self.bar_color
            };
            frame.fill_rectangle(Point::new(x, y), Size::new(bar_w, h), color);
        }

        vec![frame.into_geometry()]
    }
}

struct TokensByDayCanvas {
    buckets: Vec<(u64, u64)>,
}

impl<M> canvas::Program<M> for TokensByDayCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let mid_y = bounds.height * 0.58;
        frame.fill_rectangle(
            Point::new(0.0, mid_y),
            Size::new(bounds.width, 1.0),
            theme::BORDER,
        );

        let n = self.buckets.len();
        if n == 0 {
            return vec![frame.into_geometry()];
        }

        let max_in = self.buckets.iter().map(|b| b.0).max().unwrap_or(1).max(1) as f32;
        let max_out = self.buckets.iter().map(|b| b.1).max().unwrap_or(1).max(1) as f32;
        let top_h = (mid_y - 6.0).max(10.0);
        let bot_h = (bounds.height - mid_y - 6.0).max(10.0);

        let slot_w = bounds.width / n as f32;
        let bar_w = (slot_w * 0.62).clamp(2.0, 14.0);

        for (i, &(inp, out)) in self.buckets.iter().enumerate() {
            let x = i as f32 * slot_w + (slot_w - bar_w) * 0.5;
            if inp > 0 {
                let h = ((inp as f32 / max_in) * top_h).max(3.0);
                frame.fill_rectangle(
                    Point::new(x, mid_y - 2.0 - h),
                    Size::new(bar_w, h),
                    Color::from_rgb8(0x52, 0x52, 0x5B),
                );
            } else {
                frame.fill_rectangle(
                    Point::new(x, mid_y - 3.0),
                    Size::new(bar_w, 2.0),
                    theme::BORDER,
                );
            }

            if out > 0 {
                let h = ((out as f32 / max_out) * bot_h).max(3.0);
                frame.fill_rectangle(
                    Point::new(x, mid_y + 2.0),
                    Size::new(bar_w, h),
                    theme::BLUE,
                );
            }
        }

        vec![frame.into_geometry()]
    }
}

struct HeatmapCanvas {
    grid: [[u64; 24]; 7],
}

impl<M> canvas::Program<M> for HeatmapCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let label_w = 18.0;
        let grid_w = (bounds.width - label_w).max(24.0);
        let cell_w = (grid_w / 24.0).max(4.0);
        let cell_h = (bounds.height / 7.0).max(4.0);
        let gap = 2.0;

        let max_v = self
            .grid
            .iter()
            .flat_map(|r| r.iter())
            .copied()
            .max()
            .unwrap_or(0);

        let days = ["M", "T", "W", "T", "F", "S", "S"];
        for (d, row_vals) in self.grid.iter().enumerate() {
            let y = d as f32 * cell_h;
            frame.fill_text(Text {
                content: days[d].to_string(),
                position: Point::new(0.0, y + 1.0),
                color: theme::TEXT_MUTED,
                size: 10.0.into(),
                ..Text::default()
            });

            for (h, &v) in row_vals.iter().enumerate() {
                let x = label_w + h as f32 * cell_w;
                let intensity = if max_v == 0 || v == 0 {
                    0.0
                } else {
                    (v as f32 / max_v as f32).sqrt()
                };
                let color = if intensity == 0.0 {
                    Color::from_rgb8(0x19, 0x19, 0x1D)
                } else if intensity < 0.25 {
                    Color::from_rgb8(0x3F, 0x3F, 0x46)
                } else if intensity < 0.55 {
                    Color::from_rgb8(0x71, 0x71, 0x7A)
                } else if intensity < 0.80 {
                    Color::from_rgb8(0xA1, 0xA1, 0xAA)
                } else {
                    theme::TEXT_PRIMARY
                };

                frame.fill_rectangle(
                    Point::new(x, y + 1.0),
                    Size::new((cell_w - gap).max(2.0), (cell_h - gap).max(2.0)),
                    color,
                );
            }
        }

        vec![frame.into_geometry()]
    }
}

struct StackedBarCanvas {
    segments: Vec<(Color, f32)>,
}

impl<M> canvas::Program<M> for StackedBarCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), theme::BORDER);
        let mut x = 0.0;
        for &(color, pct) in &self.segments {
            let w = (bounds.width * (pct / 100.0)).max(2.0);
            frame.fill_rectangle(Point::new(x, 0.0), Size::new(w, bounds.height), color);
            x += w + 2.0;
        }
        vec![frame.into_geometry()]
    }
}

struct SingleBarCanvas {
    ratio: f32,
    color: Color,
}

impl<M> canvas::Program<M> for SingleBarCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        if self.ratio > 0.0 {
            let w = (bounds.width * self.ratio).max(3.0);
            frame.fill_rectangle(Point::ORIGIN, Size::new(w, bounds.height), self.color);
        }
        vec![frame.into_geometry()]
    }
}
