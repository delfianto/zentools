//! Ratatui-based live dashboard for `zen smu monitor`
//!
//! Replaces the old print-and-clear-screen loop with a proper terminal UI:
//! flicker-free redraws, color-coded thresholds, live sparkline history for
//! package power and temperature, and keyboard interaction (pause/quit).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, Cell, Paragraph, Row, Sparkline, Table};
use ratatui::Frame;

use zentools::smu::driver::{self, CpuTopology};
use zentools::smu::{self, msr, smn, CoreMetrics, CpuMetrics, SmuInfo};

const HISTORY_LEN: usize = 120;

// Color thresholds — tuned for desktop Ryzen idle/boost ranges, not user-configurable (yet).
const TEMP_WARN_C: f64 = 60.0;
const TEMP_HOT_C: f64 = 85.0;
const CORE_POWER_WARN_W: f64 = 5.0;
const CORE_POWER_HOT_W: f64 = 12.0;

pub fn run(interval: Duration) -> Result<()> {
    let mut app = App::new(interval);
    let mut terminal = ratatui::init();
    let result = run_loop(&mut terminal, &mut app);
    ratatui::restore();
    result
}

struct App {
    cpu_model: String,
    topo: Option<CpuTopology>,
    smu_info: Option<SmuInfo>,
    is_zen5: bool,
    smn_reader: smn::SmnReader,
    smn_available: bool,
    rapl: Option<msr::RaplReader>,
    metrics: CpuMetrics,
    ccd_temps: Vec<Option<f64>>,
    power_history: VecDeque<Option<u64>>,
    temp_history: VecDeque<Option<u64>>,
    paused: bool,
    interval: Duration,
}

impl App {
    fn new(interval: Duration) -> Self {
        let cpu_model = driver::read_cpu_model().unwrap_or_else(|| "Unknown".to_string());
        let topo = driver::read_cpu_topology();
        let smu_info = driver::read_info().ok();
        let is_zen5 = smu_info
            .as_ref()
            .map(|i| i.codename.is_zen5())
            .unwrap_or(false);

        let smn_reader = smn::SmnReader::new(is_zen5);
        let smn_available = smn_reader.is_available();

        let mut rapl = msr::RaplReader::new().ok();
        if let Some(r) = rapl.as_mut() {
            let _ = r.read_package_power();
            let _ = r.read_core_power();
        }

        Self {
            cpu_model,
            topo,
            smu_info,
            is_zen5,
            smn_reader,
            smn_available,
            rapl,
            metrics: CpuMetrics::default(),
            ccd_temps: Vec::new(),
            power_history: VecDeque::with_capacity(HISTORY_LEN),
            temp_history: VecDeque::with_capacity(HISTORY_LEN),
            paused: false,
            interval,
        }
    }

    fn refresh(&mut self) {
        if self.paused {
            return;
        }

        let metrics = smu::read_metrics(
            if self.smn_available {
                Some(&self.smn_reader)
            } else {
                None
            },
            self.rapl.as_mut(),
        );

        let mut ccd_temps = metrics.ccd_temps_c.clone();
        if ccd_temps.is_empty() && self.smn_available {
            let max_ccds: u32 = if self.is_zen5 { 2 } else { 8 };
            if let Ok(temps) = self.smn_reader.read_all_ccd_temps(max_ccds) {
                ccd_temps = temps;
            }
        }

        let max_core_temp = metrics
            .per_core
            .iter()
            .filter_map(|c| c.temp_c)
            .fold(None, |acc: Option<f64>, t| Some(acc.map_or(t, |a| a.max(t))));

        push_history(
            &mut self.power_history,
            metrics.package_power_w.or(metrics.core_power_w),
        );
        push_history(&mut self.temp_history, metrics.tctl_temp_c.or(max_core_temp));

        self.metrics = metrics;
        self.ccd_temps = ccd_temps;
    }
}

fn push_history(hist: &mut VecDeque<Option<u64>>, value: Option<f64>) {
    if hist.len() >= HISTORY_LEN {
        hist.pop_front();
    }
    hist.push_back(value.map(|v| v.max(0.0).round() as u64));
}

fn run_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    // Refresh immediately on first iteration rather than waiting a full interval.
    let mut last_tick = Instant::now() - app.interval;

    loop {
        if last_tick.elapsed() >= app.interval {
            app.refresh();
            last_tick = Instant::now();
        }

        terminal.draw(|frame| draw(frame, app))?;

        let wait = app
            .interval
            .saturating_sub(last_tick.elapsed())
            .min(Duration::from_millis(200));

        if event::poll(wait)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(());
                }
                KeyCode::Char('p') => app.paused = !app.paused,
                _ => {}
            }
        }
    }
}

// =============================================================================
// Rendering
// =============================================================================

fn draw(frame: &mut Frame, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(10),
            Constraint::Length(1),
        ])
        .split(frame.area());

    draw_header(frame, outer[0], app);

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(outer[1]);

    draw_core_table(frame, main[0], app);
    draw_side_panel(frame, main[1], app);

    draw_footer(frame, outer[2], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let mut line1 = app.cpu_model.clone();
    if let Some(info) = &app.smu_info {
        line1.push_str(&format!(
            "   {}   SMU {}",
            info.codename.as_str(),
            info.version
        ));
    }

    let mut line2 = format!("Mode: {}", app.metrics.source);
    if let Some(t) = &app.topo {
        line2.push_str(&format!(
            "   {} cores / {} threads ({})",
            t.physical_cores,
            t.logical_cpus,
            if t.smt { "SMT" } else { "no SMT" }
        ));
    }
    if app.paused {
        line2.push_str("   [PAUSED]");
    }

    let para = Paragraph::new(vec![Line::from(line1), Line::from(line2)])
        .block(Block::bordered().title(" Zen Monitor "));
    frame.render_widget(para, area);
}

fn draw_core_table(frame: &mut Frame, area: Rect, app: &App) {
    if app.metrics.per_core.is_empty() {
        let para = Paragraph::new(
            "No per-core data available.\nRequires the ryzen_smu driver with a PM table \
             version that has a known per-core field mapping.",
        )
        .block(Block::bordered().title(" Per-Core "))
        .dim();
        frame.render_widget(para, area);
        return;
    }

    let header = Row::new(vec![
        "Core", "State", "Power", "Volt", "Temp", "C0%", "C1%", "C6%",
    ])
    .bold();

    let rows: Vec<Row> = app
        .metrics
        .per_core
        .iter()
        .map(|core| {
            let state = freq_or_state_label(core);
            let state_style = state_style(&state);
            Row::new(vec![
                Cell::new(format!("{:>2}", core.core_id)),
                Cell::new(state).style(state_style),
                Cell::new(fmt_opt_f(core.power_w, 2)).style(power_style(core.power_w)),
                Cell::new(fmt_opt_f(core.voltage_v.filter(|&v| v > 0.1), 3)),
                Cell::new(fmt_opt_f(core.temp_c.filter(|&t| t > 0.1), 1))
                    .style(temp_style(core.temp_c)),
                Cell::new(fmt_opt(core.c0_pct)),
                Cell::new(fmt_opt(core.cc1_pct)),
                Cell::new(fmt_opt(core.cc6_pct)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Length(11),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(5),
        Constraint::Length(5),
        Constraint::Length(5),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .column_spacing(1)
        .block(Block::bordered().title(" Per-Core "));

    frame.render_widget(table, area);
}

fn draw_side_panel(frame: &mut Frame, area: Rect, app: &App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(7),
            Constraint::Length(7),
        ])
        .split(area);

    let info = Paragraph::new(system_lines(app)).block(Block::bordered().title(" System "));
    frame.render_widget(info, rows[0]);

    draw_sparkline(frame, rows[1], "Power", &app.power_history, "W");
    draw_sparkline(frame, rows[2], "Temp", &app.temp_history, "C");
}

fn draw_sparkline(frame: &mut Frame, area: Rect, title: &str, history: &VecDeque<Option<u64>>, unit: &str) {
    let data: Vec<Option<u64>> = history.iter().copied().collect();
    let latest = data.iter().rev().find_map(|v| *v);
    let title_text = match latest {
        Some(v) => format!(" {} ({} {}) ", title, v, unit),
        None => format!(" {} (no data) ", title),
    };

    let sparkline = Sparkline::default()
        .block(Block::bordered().title(title_text))
        .data(data.as_slice())
        .style(Style::new().fg(Color::Cyan));

    frame.render_widget(sparkline, area);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let text = format!(
        "q / Esc / Ctrl+C: quit    p: pause/resume    interval: {}s",
        app.interval.as_secs()
    );
    frame.render_widget(Paragraph::new(text).dim(), area);
}

fn system_lines(app: &App) -> Vec<Line<'static>> {
    let m = &app.metrics;
    let mut lines: Vec<Line<'static>> = Vec::new();

    if let Some(t) = m.tctl_temp_c {
        let text = m
            .tjmax_c
            .map(|tj| format!("Tctl/TjMax: {:.1} / {:.0} C", t, tj))
            .unwrap_or_else(|| format!("Tctl: {:.1} C", t));
        lines.push(Line::from(text).style(temp_style(Some(t))));
    }
    for (i, temp) in app.ccd_temps.iter().enumerate() {
        if let Some(t) = temp {
            lines.push(Line::from(format!("CCD{} Temp: {:.1} C", i, t)).style(temp_style(Some(*t))));
        }
    }
    if let Some(f) = m.peak_core_freq_mhz {
        lines.push(Line::from(format!("Peak Freq: {:.0} MHz", f)));
    }
    if let Some(p) = m.package_power_w.or(m.core_power_w) {
        lines.push(Line::from(format!("Pkg Power: {:.2} W", p)));
    }
    if let Some(p) = m.soc_power_w {
        lines.push(Line::from(format!("SoC Power: {:.2} W", p)));
    }
    if let Some(v) = m.peak_voltage_v.or(m.core_voltage_v) {
        lines.push(Line::from(format!("Core Volt: {:.4} V", v)));
    }
    if let Some(v) = m.soc_voltage_v {
        lines.push(Line::from(format!("SoC Volt: {:.4} V", v)));
    }
    if let Some(f) = m.fclk_mhz {
        let coupled = match (m.uclk_mhz, m.mclk_mhz) {
            (Some(u), Some(mc)) if (u - mc).abs() < 1.0 => "Coupled",
            (Some(_), Some(_)) => "Decoupled",
            _ => "-",
        };
        lines.push(Line::from(format!("FCLK: {:.0} MHz ({})", f, coupled)));
    }
    if let (Some(u), Some(mc)) = (m.uclk_mhz, m.mclk_mhz) {
        lines.push(Line::from(format!("UCLK/MCLK: {:.0} / {:.0} MHz", u, mc)));
    }
    if m.vddp_v.is_some() || m.vddg_v.is_some() {
        lines.push(Line::from(format!(
            "VDDP/VDDG: {} / {} V",
            m.vddp_v.map(|v| format!("{:.4}", v)).unwrap_or_else(|| "-".into()),
            m.vddg_v.map(|v| format!("{:.4}", v)).unwrap_or_else(|| "-".into()),
        )));
    }
    push_pbo_line(&mut lines, "PPT", m.ppt_current_w, m.ppt_limit_w, "W");
    push_pbo_line(&mut lines, "TDC", m.tdc_current_a, m.tdc_limit_a, "A");
    push_pbo_line(&mut lines, "EDC", m.edc_current_a, m.edc_limit_a, "A");

    if lines.is_empty() {
        lines.push(Line::from("No system metrics available.").dim());
        lines.push(Line::from("Run as root for Tctl/RAPL/SVI.").dim());
    }

    lines
}

fn push_pbo_line(lines: &mut Vec<Line<'static>>, name: &str, current: Option<f64>, limit: Option<f64>, unit: &str) {
    if let (Some(cur), Some(lim)) = (current, limit) {
        let pct = if lim > 0.0 { cur / lim * 100.0 } else { 0.0 };
        lines.push(Line::from(format!(
            "{}: {:.1}/{:.1} {} ({:.0}%)",
            name, cur, lim, unit, pct
        )));
    }
}

// =============================================================================
// Formatting and color helpers
// =============================================================================

/// Freq column content: real MHz when known (Zen 2/3), otherwise a state derived
/// from C-state residency (Zen 5), otherwise "-" when neither is available (Zen 4).
/// "Sleep" is only used when frequency itself is known to read ~0 — it should never
/// be shown just because frequency is unmapped for this generation.
fn freq_or_state_label(core: &CoreMetrics) -> String {
    if let Some(f) = core.frequency_mhz {
        if f > 0.1 {
            return format!("{:.0}", f);
        }
        return "Sleep".to_string();
    }

    match (core.c0_pct, core.cc1_pct, core.cc6_pct) {
        (Some(c0), Some(cc1), Some(cc6)) => {
            if c0 >= cc1 && c0 >= cc6 {
                "Active".to_string()
            } else if cc6 >= cc1 {
                "Deep Sleep".to_string()
            } else {
                "Light Sleep".to_string()
            }
        }
        _ => "-".to_string(),
    }
}

fn state_style(label: &str) -> Style {
    match label {
        "Active" => Style::new().fg(Color::Green).bold(),
        "Light Sleep" => Style::new().fg(Color::Cyan),
        "Deep Sleep" => Style::new().fg(Color::DarkGray),
        "Sleep" | "-" => Style::new().fg(Color::DarkGray),
        s if s.chars().all(|c| c.is_ascii_digit()) => Style::new().fg(Color::Green),
        _ => Style::new(),
    }
}

fn temp_style(temp: Option<f64>) -> Style {
    match temp {
        Some(t) if t >= TEMP_HOT_C => Style::new().fg(Color::Red).bold(),
        Some(t) if t >= TEMP_WARN_C => Style::new().fg(Color::Yellow),
        Some(_) => Style::new().fg(Color::Green),
        None => Style::new().fg(Color::DarkGray),
    }
}

fn power_style(power: Option<f64>) -> Style {
    match power {
        Some(p) if p >= CORE_POWER_HOT_W => Style::new().fg(Color::Red).bold(),
        Some(p) if p >= CORE_POWER_WARN_W => Style::new().fg(Color::Yellow),
        Some(_) => Style::new().fg(Color::Green),
        None => Style::new().fg(Color::DarkGray),
    }
}

fn fmt_opt(v: Option<f64>) -> String {
    v.map(|v| format!("{:.1}", v)).unwrap_or_else(|| "-".to_string())
}

fn fmt_opt_f(v: Option<f64>, prec: usize) -> String {
    v.map(|v| format!("{:.prec$}", v, prec = prec))
        .unwrap_or_else(|| "-".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core_with(power: Option<f64>, c0: Option<f64>, cc1: Option<f64>, cc6: Option<f64>) -> CoreMetrics {
        CoreMetrics {
            power_w: power,
            c0_pct: c0,
            cc1_pct: cc1,
            cc6_pct: cc6,
            ..Default::default()
        }
    }

    #[test]
    fn test_freq_or_state_label_known_frequency() {
        let core = CoreMetrics {
            frequency_mhz: Some(4500.0),
            ..Default::default()
        };
        assert_eq!(freq_or_state_label(&core), "4500");
    }

    #[test]
    fn test_freq_or_state_label_known_frequency_asleep() {
        let core = CoreMetrics {
            frequency_mhz: Some(0.0),
            ..Default::default()
        };
        assert_eq!(freq_or_state_label(&core), "Sleep");
    }

    #[test]
    fn test_freq_or_state_label_active_from_cstates() {
        let core = core_with(None, Some(99.0), Some(1.0), Some(0.0));
        assert_eq!(freq_or_state_label(&core), "Active");
    }

    #[test]
    fn test_freq_or_state_label_deep_sleep_from_cstates() {
        let core = core_with(None, Some(5.0), Some(20.0), Some(75.0));
        assert_eq!(freq_or_state_label(&core), "Deep Sleep");
    }

    #[test]
    fn test_freq_or_state_label_light_sleep_from_cstates() {
        let core = core_with(None, Some(5.0), Some(80.0), Some(15.0));
        assert_eq!(freq_or_state_label(&core), "Light Sleep");
    }

    #[test]
    fn test_freq_or_state_label_no_data() {
        let core = core_with(None, None, None, None);
        assert_eq!(freq_or_state_label(&core), "-");
    }

    #[test]
    fn test_temp_style_thresholds() {
        assert_eq!(temp_style(Some(40.0)), Style::new().fg(Color::Green));
        assert_eq!(temp_style(Some(70.0)), Style::new().fg(Color::Yellow));
        assert_eq!(temp_style(Some(90.0)), Style::new().fg(Color::Red).bold());
        assert_eq!(temp_style(None), Style::new().fg(Color::DarkGray));
    }

    #[test]
    fn test_power_style_thresholds() {
        assert_eq!(power_style(Some(1.0)), Style::new().fg(Color::Green));
        assert_eq!(power_style(Some(8.0)), Style::new().fg(Color::Yellow));
        assert_eq!(power_style(Some(15.0)), Style::new().fg(Color::Red).bold());
        assert_eq!(power_style(None), Style::new().fg(Color::DarkGray));
    }

    #[test]
    fn test_fmt_opt() {
        assert_eq!(fmt_opt(Some(12.34)), "12.3");
        assert_eq!(fmt_opt(None), "-");
    }

    #[test]
    fn test_fmt_opt_f() {
        assert_eq!(fmt_opt_f(Some(1.23456), 3), "1.235");
        assert_eq!(fmt_opt_f(None, 3), "-");
    }

    #[test]
    fn test_push_history_caps_length() {
        let mut hist: VecDeque<Option<u64>> = VecDeque::new();
        for i in 0..(HISTORY_LEN + 10) {
            push_history(&mut hist, Some(i as f64));
        }
        assert_eq!(hist.len(), HISTORY_LEN);
        // Oldest entries should have been dropped; last pushed value is at the back.
        assert_eq!(hist.back().copied().flatten(), Some((HISTORY_LEN + 9) as u64));
    }

    #[test]
    fn test_push_history_none_stored_as_none() {
        let mut hist: VecDeque<Option<u64>> = VecDeque::new();
        push_history(&mut hist, None);
        assert_eq!(hist.back().copied().flatten(), None);
    }
}
