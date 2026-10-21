//! A frame's `chart`: data the engine draws as the frame's children (bars, lines, slices, labels,
//! grid and legend), each placed from the data rather than by hand, so the labels line up with
//! what they label. The drawing is `relay_client::design::chart`; this is the data and its rules.

use super::{color_ref, obj_vec, parse_color};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Most categories (labels) a chart holds, most series, and the longest label.
pub const MAX_CHART_LABELS: usize = 100;
pub const MAX_CHART_SERIES: usize = 8;
pub const MAX_CHART_LABEL: usize = 64;

/// What a chart draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DesignChartKind")]
pub enum ChartKind {
    Bar,
    Line,
    Area,
    Donut,
    Pie,
}

impl ChartKind {
    /// Whether it has axes: a bar, line or area chart.
    pub fn has_axes(self) -> bool {
        matches!(self, ChartKind::Bar | ChartKind::Line | ChartKind::Area)
    }
}

/// One series of a chart: a value per label.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "DesignChartSeries")]
pub struct Series {
    /// Its name in the legend.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[schemars(length(max = MAX_CHART_LABEL))]
    pub name: String,
    /// One per label.
    #[schemars(length(min = 1, max = MAX_CHART_LABELS))]
    pub values: Vec<f64>,
    /// Default: the palette's next.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// One per label, a colour or `null`: that value's bar, point or slice in its own colour, the
    /// rest in the series'. `[null, null, null, "#7FB2FF"]` brings out the last bar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = MAX_CHART_LABELS))]
    pub colors: Option<Vec<Option<String>>>,
}

/// A chart a frame draws: `{"kind": "bar", "labels": ["Jan", "Feb"], "series": [{"name": "Spent", "values": [420, 380]}], "prefix": "$"}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "DesignChart")]
pub struct Chart {
    pub kind: ChartKind,
    /// The categories (a donut's or pie's slices), 1 to 100.
    #[schemars(length(min = 1, max = MAX_CHART_LABELS))]
    pub labels: Vec<String>,
    /// 1 to 8 (a donut or pie draws the first).
    #[serde(deserialize_with = "obj_vec")]
    #[schemars(length(min = 1, max = MAX_CHART_SERIES))]
    pub series: Vec<Series>,
    /// Bars stacked, not side by side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stacked: Option<bool>,
    /// Bars across, labels down the left.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizontal: Option<bool>,
    /// The value axis' ends (default: 0 to a round number past the data).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// About how many axis steps, 0 to 10 (default 4; 0 none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(max = 10))]
    pub ticks: Option<u8>,
    /// Grid lines (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid: Option<bool>,
    /// Each value written by its bar, point or slice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<bool>,
    /// A key along the top (default: several series, or a ring).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legend: Option<bool>,
    /// Written before (`"$"`) and after (`"%"`) each number, and its decimals (0 to 6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 8))]
    pub prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 8))]
    pub suffix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(max = 6))]
    pub decimals: Option<u8>,
    /// Curved lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smooth: Option<bool>,
    /// A dot at each point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<bool>,
    /// A donut's hole, of its radius (default 0.6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.1, max = 0.9))]
    pub inner: Option<f64>,
    /// Words in a donut's hole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = MAX_CHART_LABEL))]
    pub center: Option<String>,
    /// The room between bars (default 0.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(max = 0.9))]
    pub gap: Option<f64>,
    /// The labels' colour, the grid's, and the labels' size (default 12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 6, max = 48))]
    pub font_size: Option<f64>,
    /// Colours for what names none (default: Relay's eight).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 12))]
    pub palette: Option<Vec<String>>,
}

/// The palette a chart takes when it names none: distinct in hue and lightness, each readable on
/// white and on the canvas.
pub const CHART_PALETTE: [&str; 8] = ["#4C7DFF", "#F28C38", "#2EAD6B", "#E5484D", "#8E5CF0", "#14A3B8", "#D4A200", "#D6409F"];

impl Chart {
    /// Its rules, each refusal starting with the field at fault (`chart.series[1].values: …`).
    pub fn check(&self) -> Result<(), String> {
        let n = self.labels.len();
        if !(1..=MAX_CHART_LABELS).contains(&n) {
            return Err(format!("chart.labels: 1 to {MAX_CHART_LABELS} of them"));
        }
        if let Some(label) = self.labels.iter().find(|l| l.chars().count() > MAX_CHART_LABEL) {
            return Err(format!("chart.labels: {label:?} is longer than {MAX_CHART_LABEL} characters"));
        }
        if !(1..=MAX_CHART_SERIES).contains(&self.series.len()) {
            return Err(format!("chart.series: 1 to {MAX_CHART_SERIES} of them"));
        }
        for (i, s) in self.series.iter().enumerate() {
            if s.values.len() != n {
                return Err(format!("chart.series[{i}].values: {} of them, one per label ({n})", s.values.len()));
            }
            if s.values.iter().any(|v| !v.is_finite() || v.abs() > 1e15) {
                return Err(format!("chart.series[{i}].values: every value a number within ±1e15"));
            }
            if s.name.chars().count() > MAX_CHART_LABEL {
                return Err(format!("chart.series[{i}].name: at most {MAX_CHART_LABEL} characters"));
            }
            if let Some(c) = &s.color {
                color(&format!("chart.series[{i}].color"), c)?;
            }
            if let Some(colors) = &s.colors {
                if colors.len() != n {
                    return Err(format!("chart.series[{i}].colors: {} of them, one per label ({n}); null keeps the series' colour", colors.len()));
                }
                for (k, c) in colors.iter().enumerate() {
                    if let Some(c) = c {
                        color(&format!("chart.series[{i}].colors[{k}]"), c)?;
                    }
                }
            }
        }
        if !self.kind.has_axes() {
            if self.series[0].values.iter().any(|v| *v < 0.0) {
                return Err("chart.series[0].values: a donut or pie takes no value below 0".into());
            }
            if self.series[0].values.iter().all(|v| *v == 0.0) {
                return Err("chart.series[0].values: a donut or pie needs a value above 0".into());
            }
        }
        if self.horizontal == Some(true) && self.kind != ChartKind::Bar {
            return Err("chart.horizontal: only a bar chart lies across".into());
        }
        if let (Some(lo), Some(hi)) = (self.min, self.max) {
            if lo >= hi || lo.is_nan() || hi.is_nan() {
                return Err(format!("chart.min: {lo} is not below max {hi}"));
            }
        }
        for (field, v) in [("min", self.min), ("max", self.max)] {
            if v.is_some_and(|v| !v.is_finite() || v.abs() > 1e15) {
                return Err(format!("chart.{field}: a number within ±1e15"));
            }
        }
        if self.ticks.is_some_and(|t| t > 10) {
            return Err("chart.ticks: 0 to 10".into());
        }
        if self.decimals.is_some_and(|d| d > 6) {
            return Err("chart.decimals: 0 to 6".into());
        }
        for (field, text) in [("prefix", &self.prefix), ("suffix", &self.suffix)] {
            if text.as_ref().is_some_and(|t| t.chars().count() > 8) {
                return Err(format!("chart.{field}: at most 8 characters"));
            }
        }
        if self.inner.is_some_and(|v| !(0.1..=0.9).contains(&v)) {
            return Err("chart.inner: 0.1 to 0.9".into());
        }
        if self.gap.is_some_and(|v| !(0.0..=0.9).contains(&v)) {
            return Err("chart.gap: 0 to 0.9".into());
        }
        if self.font_size.is_some_and(|v| !(6.0..=48.0).contains(&v)) {
            return Err("chart.font_size: 6 to 48".into());
        }
        if self.center.as_ref().is_some_and(|c| c.chars().count() > MAX_CHART_LABEL) {
            return Err(format!("chart.center: at most {MAX_CHART_LABEL} characters"));
        }
        for (field, c) in [("color", &self.color), ("grid_color", &self.grid_color)] {
            if let Some(c) = c {
                color(&format!("chart.{field}"), c)?;
            }
        }
        if let Some(palette) = &self.palette {
            if !(1..=12).contains(&palette.len()) {
                return Err("chart.palette: 1 to 12 colours".into());
            }
            for (i, c) in palette.iter().enumerate() {
                color(&format!("chart.palette[{i}]"), c)?;
            }
        }
        Ok(())
    }

    /// Series `i`'s colour: its own, else the palette's, round again past its end.
    pub fn series_color(&self, i: usize) -> String {
        self.series.get(i).and_then(|s| s.color.clone()).unwrap_or_else(|| self.palette_color(i))
    }

    /// Series `j`'s value `i`'s colour: its own in `colors`, else the series'.
    pub fn value_color(&self, j: usize, i: usize) -> String {
        self.own_color(j, i).unwrap_or_else(|| self.series_color(j))
    }

    /// A donut's or pie's slice `i`'s colour: the first series' own for it in `colors`, else the
    /// palette's.
    pub fn slice_color(&self, i: usize) -> String {
        self.own_color(0, i).unwrap_or_else(|| self.palette_color(i))
    }

    fn own_color(&self, j: usize, i: usize) -> Option<String> {
        self.series.get(j)?.colors.as_ref()?.get(i)?.clone()
    }

    /// The palette's colour `i`, round again past its end.
    pub fn palette_color(&self, i: usize) -> String {
        match &self.palette {
            Some(p) if !p.is_empty() => p[i % p.len()].clone(),
            _ => CHART_PALETTE[i % CHART_PALETTE.len()].to_string(),
        }
    }
}

fn color(field: &str, c: &str) -> Result<(), String> {
    if parse_color(c).is_some() || color_ref(c).is_some() {
        Ok(())
    } else {
        Err(format!("{field}: {c:?} is not #RGB, #RRGGBB or #RRGGBBAA, or a $name colour"))
    }
}
