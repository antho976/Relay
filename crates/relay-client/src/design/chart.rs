//! A chart's parts (relay_bus::design::chart): the nodes the engine writes as a chart frame's
//! children, each placed from the data inside the frame's padding. Bars are rects, lines and areas
//! are lines through their points, slices are ellipses' arcs, and every word is a text of a fixed
//! width aligned in the room it labels, so no label needs measuring to line up with its bar. Only
//! the legend's spacing estimates its words (`layout::ESTIMATE_EM`).
//!
//! Every part is `generated`, named `<frame>.<part>` (`spend.bar-0-3`), and given its parent;
//! nothing else is set that the chart does not say, so a text's colour is the canvas's own pick
//! for its ground unless the chart names one.

use super::layout::ESTIMATE_EM;
use relay_bus::design::chart::{Chart, ChartKind};
use relay_bus::design::{Arc, Node, NodeKind, TextAlign};

/// The grid's colour when the chart names none: a grey that reads on light and dark grounds.
pub const GRID: &str = "#8C8C8C40";

/// The parts chart frame `frame` draws, in paint order (grid under bars under labels). None when
/// it holds no chart or has no room.
pub fn parts(frame: &Node) -> Vec<Node> {
    let Some(chart) = frame.chart.as_deref() else { return Vec::new() };
    let [t, r, b, l] = frame.padding_edges();
    let room = Room { x: l, y: t, w: (frame.w - l - r).max(0.0), h: (frame.h - t - b).max(0.0) };
    if room.w < 8.0 || room.h < 8.0 {
        return Vec::new();
    }
    let mut draw = Draw { frame: &frame.id, chart, out: Vec::new(), size: chart.font_size.unwrap_or(12.0) };
    let legend = chart.legend.unwrap_or(chart.series.len() > 1 || !chart.kind.has_axes());
    let room = if legend { draw.legend(room) } else { room };
    match chart.kind {
        ChartKind::Donut | ChartKind::Pie => draw.ring(room),
        ChartKind::Bar if chart.horizontal == Some(true) => draw.across(room),
        _ => draw.axes(room),
    }
    // A bar of no length (a value of 0, or one too small to show) is not drawn: a rect stored
    // 0 tall takes the 100 px a shape added without a size gets (B49), and would cover its stack.
    draw.out.retain(|n| n.kind != NodeKind::Rect || (n.w > 0.0 && n.h > 0.0));
    draw.out
}

#[derive(Clone, Copy)]
struct Room {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

struct Draw<'a> {
    frame: &'a str,
    chart: &'a Chart,
    out: Vec<Node>,
    size: f64,
}

/// The value axis: its ends, and the steps labelled along it.
struct Axis {
    lo: f64,
    hi: f64,
    steps: Vec<f64>,
    decimals: usize,
}

impl Axis {
    /// Where value `v` falls from 0 (`lo`) to 1 (`hi`).
    fn at(&self, v: f64) -> f64 {
        (v - self.lo) / (self.hi - self.lo)
    }
}

impl Draw<'_> {
    fn node(&mut self, kind: NodeKind, part: String, [x, y, w, h]: [f64; 4]) -> &mut Node {
        let mut n = Node::new(kind);
        n.id = format!("{}.{part}", self.frame);
        n.parent = Some(self.frame.to_string());
        (n.x, n.y, n.w, n.h) = (round(x), round(y), round(w.max(0.0)), round(h.max(0.0)));
        n.generated = Some(true);
        self.out.push(n);
        self.out.last_mut().expect("just pushed")
    }

    /// A one-line text `words` in a box `w` wide at (`x`, `y`), aligned as `align` says.
    fn words(&mut self, part: String, words: &str, x: f64, y: f64, w: f64, align: TextAlign) -> &mut Node {
        let (size, color) = (self.size, self.chart.color.clone());
        let n = self.node(NodeKind::Text, part, [x, y, w, 0.0]);
        n.text = Some(words.to_string());
        (n.font_size, n.align, n.max_lines, n.color) = (Some(size), Some(align), Some(1), color);
        n
    }

    fn line_h(&self) -> f64 {
        (self.size * 1.3).ceil()
    }

    /// A key along the top, wrapping onto more rows as it needs: a swatch and a name per series
    /// (per slice, for a donut or pie). The room below it.
    fn legend(&mut self, room: Room) -> Room {
        let ring = !self.chart.kind.has_axes();
        let names: Vec<(String, String)> = if ring {
            self.chart.labels.iter().enumerate().map(|(i, l)| (l.clone(), self.chart.slice_color(i))).collect()
        } else {
            self.chart.series.iter().enumerate().map(|(i, s)| (s.name.clone(), self.chart.series_color(i))).collect()
        };
        let (lh, swatch) = (self.line_h(), (self.size * 0.75).round());
        let (mut x, mut y) = (room.x, room.y);
        for (i, (name, color)) in names.iter().enumerate() {
            let words = estimate(name, self.size);
            let item = swatch + 6.0 + words;
            if x > room.x && x + item > room.x + room.w {
                (x, y) = (room.x, y + lh + 4.0);
            }
            let key = self.node(NodeKind::Rect, format!("key-{i}"), [x, y + (lh - swatch) / 2.0, swatch, swatch]);
            (key.fill, key.radius) = (Some(color.clone()), Some(2.0));
            self.words(format!("name-{i}"), name, x + swatch + 6.0, y, words + 2.0, TextAlign::Left);
            x += item + 16.0;
        }
        let used = y + lh + 12.0 - room.y;
        Room { y: room.y + used, h: (room.h - used).max(0.0), ..room }
    }

    /// The value axis over what the chart draws: its ends (round numbers past the data unless
    /// given) and its steps.
    fn axis(&self) -> Axis {
        let c = self.chart;
        let stacked = c.kind == ChartKind::Bar && c.stacked == Some(true);
        let n = c.labels.len();
        let (mut lo, mut hi) = (0.0_f64, 0.0_f64);
        for i in 0..n {
            if stacked {
                let up: f64 = c.series.iter().map(|s| s.values[i].max(0.0)).sum();
                let down: f64 = c.series.iter().map(|s| s.values[i].min(0.0)).sum();
                (lo, hi) = (lo.min(down), hi.max(up));
            } else {
                for s in &c.series {
                    (lo, hi) = (lo.min(s.values[i]), hi.max(s.values[i]));
                }
            }
        }
        let (lo, hi) = (c.min.unwrap_or(lo), c.max.unwrap_or(hi));
        let (lo, hi) = if hi > lo { (lo, hi) } else { (lo, lo + 1.0) };
        let ticks = c.ticks.unwrap_or(4);
        if ticks == 0 {
            return Axis { lo, hi, steps: Vec::new(), decimals: c.decimals.unwrap_or(0).into() };
        }
        let step = nice((hi - lo) / f64::from(ticks));
        let lo = if c.min.is_some() { lo } else { (lo / step).floor() * step };
        let hi = if c.max.is_some() { hi } else { (hi / step).ceil() * step };
        let mut steps = Vec::new();
        let mut v = (lo / step).ceil() * step;
        while v <= hi + step * 1e-9 && steps.len() < 64 {
            steps.push(v + 0.0);
            v += step;
        }
        let decimals = c.decimals.map_or_else(|| decimals_of(step), usize::from);
        Axis { lo, hi, steps, decimals }
    }

    /// The room a value written by its bar or point takes: the widest of them as estimated, with
    /// room to spare, and at least 48.
    fn value_room(&self) -> f64 {
        let c = self.chart;
        if c.values != Some(true) {
            return 48.0;
        }
        let mut shown: Vec<f64> = c.series.iter().flat_map(|s| s.values.iter().copied()).collect();
        if c.stacked == Some(true) {
            shown.extend((0..c.labels.len()).map(|i| c.series.iter().map(|s| s.values[i]).sum::<f64>()));
        }
        let words = |v: f64| self.value(v, c.decimals.map_or(decimals_of_value(v), usize::from));
        shown.into_iter().map(|v| estimate(&words(v), self.size)).fold(48.0, f64::max)
    }

    fn value(&self, v: f64, decimals: usize) -> String {
        let c = self.chart;
        format!("{}{}{}", c.prefix.as_deref().unwrap_or(""), grouped(v, decimals), c.suffix.as_deref().unwrap_or(""))
    }

    /// A bar, line or area chart with labels along the bottom and values up the left.
    fn axes(&mut self, room: Room) {
        let axis = self.axis();
        let lh = self.line_h();
        let gutter = axis.steps.iter().map(|v| estimate(&self.value(*v, axis.decimals), self.size)).fold(0.0, f64::max);
        let gutter = if axis.steps.is_empty() { 0.0 } else { (gutter + 8.0).min(room.w * 0.4) };
        let top = if self.chart.values == Some(true) { lh + 2.0 } else { lh / 2.0 };
        let plot = Room { x: room.x + gutter, y: room.y + top, w: room.w - gutter, h: (room.h - top - lh - 6.0).max(1.0) };
        let y_of = |v: f64| plot.y + plot.h * (1.0 - axis.at(v));
        self.grid(&axis, |v| (plot.x, y_of(v), plot.w, 0.0), |d, k, v, label| {
            d.words(format!("tick-{k}"), label, room.x, y_of(v) - lh / 2.0, gutter - 8.0, TextAlign::Right);
        });
        let n = self.chart.labels.len();
        let band = plot.w / n as f64;
        for i in 0..n {
            let label = self.chart.labels[i].clone();
            self.words(format!("label-{i}"), &label, plot.x + band * i as f64, plot.y + plot.h + 6.0, band, TextAlign::Center);
        }
        match self.chart.kind {
            ChartKind::Bar => self.bars(&axis, plot, band, false),
            _ => self.lines(&axis, plot, band),
        }
    }

    /// A bar chart lying across: labels down the left, values along the bottom.
    fn across(&mut self, room: Room) {
        let axis = self.axis();
        let lh = self.line_h();
        let gutter = self.chart.labels.iter().map(|l| estimate(l, self.size)).fold(0.0, f64::max);
        let gutter = (gutter + 8.0).min(room.w * 0.4);
        // Room past the longest bar for its value, 4 px from its end.
        let right = if self.chart.values == Some(true) { self.value_room() + 4.0 } else { 12.0 };
        let plot = Room { x: room.x + gutter, y: room.y, w: (room.w - gutter - right).max(1.0), h: (room.h - lh - 6.0).max(1.0) };
        let x_of = |v: f64| plot.x + plot.w * axis.at(v);
        self.grid(&axis, |v| (x_of(v), plot.y, 0.0, plot.h), |d, k, v, label| {
            // Centred under its step, unless that would pass an end of the room: then flush with it.
            let x = x_of(v);
            let (at, align) = if x - 40.0 < room.x {
                (room.x, TextAlign::Left)
            } else if x + 40.0 > room.x + room.w {
                ((room.x + room.w - 80.0).max(room.x), TextAlign::Right)
            } else {
                (x - 40.0, TextAlign::Center)
            };
            d.words(format!("tick-{k}"), label, at, plot.y + plot.h + 6.0, 80.0_f64.min(room.w), align);
        });
        let n = self.chart.labels.len();
        let band = plot.h / n as f64;
        for i in 0..n {
            let label = self.chart.labels[i].clone();
            self.words(format!("label-{i}"), &label, room.x, plot.y + band * (i as f64 + 0.5) - lh / 2.0, gutter - 8.0, TextAlign::Right);
        }
        self.bars(&axis, plot, band, true);
    }

    /// A line at each step of `axis` (`at` gives its start and its length across or down) and its
    /// label (`label`), unless the chart has no grid.
    fn grid(&mut self, axis: &Axis, at: impl Fn(f64) -> (f64, f64, f64, f64), label: impl Fn(&mut Self, usize, f64, &str)) {
        let grid = self.chart.grid != Some(false);
        let color = self.chart.grid_color.clone().unwrap_or_else(|| GRID.to_string());
        for (k, v) in axis.steps.iter().enumerate() {
            let (x, y, w, h) = at(*v);
            if grid {
                let line = self.node(NodeKind::Line, format!("grid-{k}"), [x, y, w, h]);
                line.points = Some(vec![[0.0, 0.0], [round(w), round(h)]]);
                (line.stroke, line.stroke_width) = (Some(color.clone()), Some(1.0));
            }
            let words = self.value(*v, axis.decimals);
            label(self, k, *v, &words);
        }
    }

    /// The bars, side by side or stacked, up from the axis' zero (or its lowest end above zero),
    /// or across when `across`; each value written past its bar when the chart says.
    fn bars(&mut self, axis: &Axis, plot: Room, band: f64, across: bool) {
        let c = self.chart;
        let stacked = c.stacked == Some(true);
        let m = c.series.len();
        let group = band * (1.0 - c.gap.unwrap_or(0.3));
        let thick = if stacked { group } else { group / m as f64 };
        let zero = axis.at(0.0_f64.clamp(axis.lo, axis.hi));
        let (lh, room) = (self.line_h(), self.value_room());
        for i in 0..c.labels.len() {
            let (mut up, mut down) = (0.0_f64, 0.0_f64);
            for j in 0..m {
                let v = c.series[j].values[i];
                let (from, to) = if stacked {
                    let base = if v >= 0.0 { up } else { down };
                    if v >= 0.0 { up += v } else { down += v };
                    (axis.at(base.clamp(axis.lo, axis.hi)), axis.at((base + v).clamp(axis.lo, axis.hi)))
                } else {
                    (zero, axis.at(v.clamp(axis.lo, axis.hi)))
                };
                let (a, b) = (from.min(to), from.max(to));
                let offset = band * i as f64 + (band - group) / 2.0 + if stacked { 0.0 } else { thick * j as f64 };
                let rect = if across {
                    [plot.x + plot.w * a, plot.y + offset, plot.w * (b - a), thick]
                } else {
                    [plot.x + offset, plot.y + plot.h * (1.0 - b), thick, plot.h * (b - a)]
                };
                // A stack rounds only its outer end: the last segment its side of zero.
                let outer = !stacked || c.series[j + 1..].iter().all(|s| (s.values[i] >= 0.0) != (v >= 0.0) || s.values[i] == 0.0);
                let corner = if outer { (thick / 4.0).min(4.0).round() } else { 0.0 };
                // The rounded end is the one away from the axis.
                let radii = match (across, v >= 0.0) {
                    (false, true) => [corner, corner, 0.0, 0.0],
                    (false, false) => [0.0, 0.0, corner, corner],
                    (true, true) => [0.0, corner, corner, 0.0],
                    (true, false) => [corner, 0.0, 0.0, corner],
                };
                let color = c.value_color(j, i);
                let bar = self.node(NodeKind::Rect, format!("bar-{j}-{i}"), rect);
                (bar.fill, bar.radii) = (Some(color), Some(radii));
                if c.values == Some(true) && (!stacked || j + 1 == m) {
                    let shown = if stacked { up + down } else { v };
                    let words = self.value(shown, self.chart.decimals.map_or(decimals_of_value(shown), usize::from));
                    let [x, y, w, h] = rect;
                    if across {
                        let end = if v >= 0.0 { x + w + 4.0 } else { x - 4.0 - room };
                        let align = if v >= 0.0 { TextAlign::Left } else { TextAlign::Right };
                        self.words(format!("value-{j}-{i}"), &words, end, y + h / 2.0 - lh / 2.0, room, align);
                    } else {
                        let wide = thick.max(room);
                        let at = if v >= 0.0 { y - lh - 2.0 } else { y + h + 2.0 };
                        self.words(format!("value-{j}-{i}"), &words, x + thick / 2.0 - wide / 2.0, at, wide, TextAlign::Center);
                    }
                }
            }
        }
    }

    /// A line through each series' points at the middle of each label's room, an area under it
    /// down to zero for an area chart, a dot at each point and each value above it as asked.
    fn lines(&mut self, axis: &Axis, plot: Room, band: f64) {
        let c = self.chart;
        let base = plot.y + plot.h * (1.0 - axis.at(0.0_f64.clamp(axis.lo, axis.hi)));
        let (lh, room) = (self.line_h(), self.value_room());
        for (j, s) in c.series.iter().enumerate() {
            let color = c.series_color(j);
            let points: Vec<[f64; 2]> = s.values.iter().enumerate()
                .map(|(i, v)| [round(plot.x + band * (i as f64 + 0.5)), round(plot.y + plot.h * (1.0 - axis.at(v.clamp(axis.lo, axis.hi))))])
                .collect();
            if c.kind == ChartKind::Area && points.len() > 1 {
                let mut shape = points.clone();
                shape.push([points[points.len() - 1][0], round(base)]);
                shape.push([points[0][0], round(base)]);
                let area = self.line_through(format!("area-{j}"), &shape);
                (area.fill, area.closed, area.stroke_width, area.opacity) = (Some(color.clone()), Some(true), Some(0.0), Some(0.2));
            }
            // One label is one point: a dot, with nothing to run a line between.
            let alone = points.len() < 2;
            if !alone {
                let smooth = c.smooth == Some(true);
                let line = self.line_through(format!("line-{j}"), &points);
                (line.stroke, line.stroke_width) = (Some(color.clone()), Some(2.0));
                line.smooth = smooth.then_some(true);
            }
            for (i, [x, y]) in points.iter().copied().enumerate() {
                // A point with a colour of its own is shown, so a line can mark one.
                let own = s.colors.as_ref().and_then(|cs| cs.get(i)).is_some_and(Option::is_some);
                if c.points == Some(true) || alone || own {
                    let dot = self.node(NodeKind::Ellipse, format!("dot-{j}-{i}"), [x - 3.0, y - 3.0, 6.0, 6.0]);
                    dot.fill = Some(c.value_color(j, i));
                }
                if c.values == Some(true) {
                    let v = s.values[i];
                    let words = self.value(v, c.decimals.map_or(decimals_of_value(v), usize::from));
                    let wide = band.max(room);
                    self.words(format!("value-{j}-{i}"), &words, x - wide / 2.0, y - lh - 4.0, wide, TextAlign::Center);
                }
            }
        }
    }

    /// A line through `points` (in the frame's coordinates), its box around them.
    fn line_through(&mut self, part: String, points: &[[f64; 2]]) -> &mut Node {
        let (x0, y0) = points.iter().fold((f64::MAX, f64::MAX), |(x, y), p| (x.min(p[0]), y.min(p[1])));
        let (x1, y1) = points.iter().fold((f64::MIN, f64::MIN), |(x, y), p| (x.max(p[0]), y.max(p[1])));
        let line = self.node(NodeKind::Line, part, [x0, y0, x1 - x0, y1 - y0]);
        line.points = Some(points.iter().map(|p| [round(p[0] - x0), round(p[1] - y0)]).collect());
        line
    }

    /// A donut or pie: a slice per label of the first series, clockwise from the top, its centre
    /// words in the hole, each slice's share written on it as asked.
    fn ring(&mut self, room: Room) {
        let c = self.chart;
        let d = room.w.min(room.h);
        let (cx, cy, r) = (room.x + room.w / 2.0, room.y + room.h / 2.0, d / 2.0);
        let inner = if c.kind == ChartKind::Pie { 0.0 } else { c.inner.unwrap_or(0.6) };
        let values = &c.series[0].values;
        let total: f64 = values.iter().sum();
        let lh = self.line_h();
        let mut start = 0.0;
        for (i, v) in values.iter().enumerate() {
            let sweep = 360.0 * v / total;
            if sweep <= 0.0 {
                continue;
            }
            let color = c.slice_color(i);
            let slice = self.node(NodeKind::Ellipse, format!("slice-{i}"), [cx - r, cy - r, d, d]);
            slice.fill = Some(color);
            slice.arc = Some(Arc { start, sweep: sweep.min(360.0), inner, cap: None });
            if c.values == Some(true) && sweep >= 12.0 {
                let mid = (start + sweep / 2.0).to_radians();
                let at = r * (1.0 + inner) / 2.0;
                let (x, y) = (cx + at * mid.sin(), cy - at * mid.cos());
                let share = format!("{}%", (v / total * 100.0).round());
                let n = self.words(format!("value-{i}"), &share, x - 24.0, y - lh / 2.0, 48.0, TextAlign::Center);
                n.color = Some(c.color.clone().unwrap_or_else(|| "#FFFFFF".into()));
            }
            start += sweep;
        }
        if let Some(center) = c.center.clone().filter(|_| inner > 0.0) {
            let size = (self.size * 1.6).round();
            let hole = 2.0 * r * inner;
            let n = self.words("center".into(), &center, cx - hole / 2.0, cy - size * 0.65, hole, TextAlign::Center);
            (n.font_size, n.font_weight) = (Some(size), Some(600));
        }
    }
}

/// A round step near `raw`: 1, 2 or 5 times a power of ten.
fn nice(raw: f64) -> f64 {
    if raw.is_nan() || raw <= 0.0 || !raw.is_finite() {
        return 1.0;
    }
    let pow = 10f64.powf(raw.log10().floor());
    let f = raw / pow;
    let n = if f <= 1.0 { 1.0 } else if f <= 2.0 { 2.0 } else if f <= 5.0 { 5.0 } else { 10.0 };
    n * pow
}

/// The decimals a step needs to be written exactly (0.25 needs 2), at most 6.
fn decimals_of(step: f64) -> usize {
    (0..=6).find(|d| ((step * 10f64.powi(*d as i32)).round() - step * 10f64.powi(*d as i32)).abs() < 1e-6).unwrap_or(6)
}

/// The decimals a value needs, at most 2.
fn decimals_of_value(v: f64) -> usize {
    decimals_of(v).min(2)
}

/// `v` with `decimals` decimals and its thousands grouped by commas (`-1,240.50`).
fn grouped(v: f64, decimals: usize) -> String {
    let text = format!("{:.*}", decimals, v.abs());
    let (whole, frac) = text.split_once('.').map_or((text.as_str(), None), |(w, f)| (w, Some(f)));
    let mut out = String::new();
    for (i, ch) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if let Some(frac) = frac {
        out.push('.');
        out.push_str(frac);
    }
    let zero = out.chars().all(|c| matches!(c, '0' | ',' | '.'));
    if v < 0.0 && !zero { format!("-{out}") } else { out }
}

/// About how wide `words` are at `size` px, with room to spare: 4 px over the per-character
/// guess, which short words with wide capitals ("Mon" at 12 px) otherwise outgrow and are cut
/// with an ellipsis.
fn estimate(words: &str, size: f64) -> f64 {
    (words.chars().count() as f64 * size * (ESTIMATE_EM + 0.07)).ceil() + 4.0
}

fn round(v: f64) -> f64 {
    (v * 2.0).round() / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn frame(chart: serde_json::Value, w: f64, h: f64) -> Node {
        let mut n = Node::new(NodeKind::Frame);
        (n.id, n.w, n.h) = ("spend".into(), w, h);
        n.chart = Some(Box::new(serde_json::from_value(chart).unwrap()));
        n
    }

    fn part<'a>(parts: &'a [Node], id: &str) -> &'a Node {
        parts.iter().find(|n| n.id == format!("spend.{id}")).unwrap_or_else(|| panic!("no {id} in {:?}", parts.iter().map(|n| &n.id).collect::<Vec<_>>()))
    }

    #[test]
    fn numbers_read_as_people_write_them() {
        assert_eq!((nice(23.0), nice(0.3), nice(140.0), nice(7.0)), (50.0, 0.5, 200.0, 10.0));
        assert_eq!((decimals_of(50.0), decimals_of(0.25), decimals_of(0.5)), (0, 2, 1));
        assert_eq!((grouped(1240.5, 2), grouped(-1_234_567.0, 0), grouped(-0.001, 1), grouped(999.0, 0)), ("1,240.50".into(), "-1,234,567".into(), "0.0".into(), "999".into()));
    }

    /// Bars stand on zero in the middle of their label's room, the tallest under a round top,
    /// each label exactly as wide as its room; the grid and its values are round steps.
    #[test]
    fn bars_line_up_with_their_labels() {
        let f = frame(json!({"kind": "bar", "labels": ["Jan", "Feb", "Mar", "Apr"], "series": [{"name": "Spent", "values": [420, 380, 515, 0]}], "prefix": "$"}), 400.0, 240.0);
        let parts = parts(&f);
        assert!(parts.iter().all(|n| n.generated == Some(true) && n.parent.as_deref() == Some("spend")));
        let ticks: Vec<&str> = (0..).map_while(|k| parts.iter().find(|n| n.id == format!("spend.tick-{k}"))).map(|n| n.text.as_deref().unwrap()).collect();
        assert_eq!(ticks, ["$0", "$200", "$400", "$600"]);
        for i in 0..3 {
            let (label, bar) = (part(&parts, &format!("label-{i}")), part(&parts, &format!("bar-0-{i}")));
            let (mid_label, mid_bar) = (label.x + label.w / 2.0, bar.x + bar.w / 2.0);
            assert!((mid_label - mid_bar).abs() <= 0.5, "label {i} at {mid_label}, its bar at {mid_bar}");
            assert_eq!(label.align, Some(TextAlign::Center));
        }
        let (mar, zero) = (part(&parts, "bar-0-2"), part(&parts, "grid-0"));
        assert!((mar.y + mar.h - zero.y).abs() <= 0.5, "it stands on the zero line");
        let top = part(&parts, "grid-3");
        assert!(mar.y > top.y, "under the round top");
        assert!(parts.iter().all(|n| n.id != "spend.bar-0-3"), "a 0 draws no bar");
        assert!(parts.iter().all(|n| !n.id.contains("key-")), "one series, no legend");
    }

    /// A bar lying across leaves room past its end for its value, however long it is written:
    /// the longest bar's "64.2 km" ends inside the frame's padding.
    #[test]
    fn a_value_past_the_longest_bar_stays_inside() {
        for w in [320.0, 220.0] {
            let f = frame(json!({"kind": "bar", "horizontal": true, "values": true, "suffix": " km", "decimals": 1,
                "labels": ["Mon", "Wed", "Sat"], "series": [{"values": [12.5, 30, 64.2]}]}), w, 140.0);
            let parts = parts(&f);
            let value = part(&parts, "value-0-2");
            assert_eq!(value.text.as_deref(), Some("64.2 km"));
            let needs = estimate("64.2 km", 12.0);
            assert!(value.w >= needs, "{w}: {} wide for words about {needs}", value.w);
            assert!(value.x + value.w <= w, "{w}: it ends at {}", value.x + value.w);
            assert!(value.x >= part(&parts, "bar-0-2").x + part(&parts, "bar-0-2").w, "{w}: past its bar");
            // The axis' words stay inside too, its first and last flush with the ends.
            for tick in parts.iter().filter(|n| n.id.contains(".tick-")) {
                assert!(tick.x >= 0.0 && tick.x + tick.w <= w, "{w}: {} from {} to {}", tick.id, tick.x, tick.x + tick.w);
            }
            assert_eq!(part(&parts, "tick-0").align, Some(TextAlign::Left));
        }
    }

    #[test]
    fn grouped_stacked_and_across() {
        let data = json!([{"name": "Food", "values": [3, 4]}, {"name": "Rent", "values": [5, 5]}]);
        let side = parts(&frame(json!({"kind": "bar", "labels": ["A", "B"], "series": data}), 300.0, 200.0));
        let (a, b) = (part(&side, "bar-0-0"), part(&side, "bar-1-0"));
        assert!((a.x + a.w - b.x).abs() <= 0.5 && a.y > b.y, "side by side, the taller higher");
        assert!(part(&side, "key-1").fill.as_deref() == part(&side, "bar-1-0").fill.as_deref(), "the legend's key is the bar's colour");
        let stack = parts(&frame(json!({"kind": "bar", "stacked": true, "values": true, "labels": ["A", "B"], "series": data}), 300.0, 200.0));
        let (food, rent) = (part(&stack, "bar-0-1"), part(&stack, "bar-1-1"));
        assert_eq!((food.radii.unwrap()[0], rent.radii.unwrap()[0] > 0.0), (0.0, true), "only the stack's top is round");
        assert!((rent.y + rent.h - food.y).abs() <= 0.5 && food.x == rent.x, "rent sits on food");
        assert_eq!(part(&stack, "value-1-1").text.as_deref(), Some("9"), "the stack's total");
        let across = parts(&frame(json!({"kind": "bar", "horizontal": true, "labels": ["Groceries", "Rent"], "series": [{"values": [212, 1400]}]}), 360.0, 160.0));
        let (g, r) = (part(&across, "bar-0-0"), part(&across, "bar-0-1"));
        assert!(r.w > g.w && g.x == r.x && r.y > g.y, "across, in label order down");
        let label = part(&across, "label-1");
        assert!((label.y + 8.0 - (r.y + r.h / 2.0)).abs() <= 1.5, "the label at its bar's middle");
    }

    /// A value with a colour of its own stands out from its series: today's bar, a marked point,
    /// a slice; the legend keeps the series' colour.
    #[test]
    fn one_value_can_be_brought_out() {
        let week = json!([{"name": "Minutes", "color": "#3A4A66", "values": [30, 45, 15, 50, 20, 60, 40], "colors": [null, null, null, null, null, null, "#7FB2FF"]}]);
        let bars = parts(&frame(json!({"kind": "bar", "legend": true, "labels": ["M", "T", "W", "T", "F", "S", "S"], "series": week}), 360.0, 200.0));
        let fills: Vec<&str> = (0..7).map(|i| part(&bars, &format!("bar-0-{i}")).fill.as_deref().unwrap()).collect();
        assert_eq!(fills, ["#3A4A66", "#3A4A66", "#3A4A66", "#3A4A66", "#3A4A66", "#3A4A66", "#7FB2FF"]);
        assert_eq!(part(&bars, "key-0").fill.as_deref(), Some("#3A4A66"), "the legend shows the series");
        let line = parts(&frame(json!({"kind": "line", "labels": ["A", "B", "C"], "series": [{"values": [1, 3, 2], "colors": [null, "$accent", null]}]}), 300.0, 200.0));
        assert_eq!(part(&line, "dot-0-1").fill.as_deref(), Some("$accent"), "a marked point is shown");
        assert!(line.iter().all(|n| n.id != "spend.dot-0-0"), "the rest are as asked: no points");
        let ring = parts(&frame(json!({"kind": "donut", "labels": ["A", "B"], "series": [{"values": [1, 2], "colors": ["#111111", null]}]}), 200.0, 200.0));
        assert_eq!((part(&ring, "slice-0").fill.as_deref(), part(&ring, "key-0").fill.as_deref()), (Some("#111111"), Some("#111111")));
        assert_eq!(part(&ring, "slice-1").fill.as_deref(), Some(relay_bus::design::chart::CHART_PALETTE[1]));
    }

    #[test]
    fn lines_areas_and_rings() {
        let lines = parts(&frame(json!({"kind": "area", "points": true, "smooth": true, "labels": ["Mo", "Tu", "We"], "series": [{"values": [1, 3, 2]}]}), 300.0, 200.0));
        let (line, area) = (part(&lines, "line-0"), part(&lines, "area-0"));
        assert_eq!((line.points.as_ref().unwrap().len(), area.points.as_ref().unwrap().len(), area.closed, line.smooth), (3, 5, Some(true), Some(true)));
        let dot = part(&lines, "dot-0-1");
        let label = part(&lines, "label-1");
        assert!((dot.x + 3.0 - (label.x + label.w / 2.0)).abs() <= 0.5);
        let ring = parts(&frame(json!({"kind": "donut", "values": true, "center": "$1,240", "labels": ["Food", "Rent", "Fun"], "series": [{"values": [1, 2, 1]}]}), 240.0, 260.0));
        let sweeps: Vec<f64> = (0..3).map(|i| part(&ring, &format!("slice-{i}")).arc.as_ref().unwrap().sweep).collect();
        assert_eq!(sweeps, [90.0, 180.0, 90.0]);
        assert_eq!(part(&ring, "slice-2").arc.as_ref().unwrap().start, 270.0);
        assert_eq!(part(&ring, "value-1").text.as_deref(), Some("50%"));
        assert_eq!(part(&ring, "center").text.as_deref(), Some("$1,240"));
        assert!(part(&ring, "name-2").text.as_deref() == Some("Fun"), "a ring's legend names its slices");
        let pie = parts(&frame(json!({"kind": "pie", "labels": ["A"], "series": [{"values": [3]}], "center": "x"}), 100.0, 100.0));
        assert_eq!(part(&pie, "slice-0").arc.as_ref().unwrap().inner, 0.0);
        assert!(pie.iter().all(|n| !n.id.ends_with(".center")), "a pie has no hole for words");
    }

    #[test]
    fn every_part_is_a_node_the_rules_take() {
        for chart in [
            json!({"kind": "bar", "values": true, "labels": ["A", "B"], "series": [{"values": [-2, 5]}, {"values": [1, 1]}]}),
            json!({"kind": "bar", "horizontal": true, "values": true, "labels": ["A", "B"], "series": [{"values": [-2, 5]}]}),
            json!({"kind": "line", "points": true, "values": true, "labels": ["A"], "series": [{"values": [2]}]}),
            json!({"kind": "area", "ticks": 0, "labels": ["A", "B", "C"], "series": [{"values": [2, -1, 4]}]}),
            json!({"kind": "donut", "values": true, "labels": ["A", "B"], "series": [{"values": [0, 4]}]}),
        ] {
            let f = frame(chart.clone(), 320.0, 200.0);
            for mut n in parts(&f) {
                n.fill_default_size();
                n.contain_points();
                n.validate().unwrap_or_else(|why| panic!("{chart}: {}: {why}", n.id));
                n.check_new().unwrap_or_else(|why| panic!("{chart}: {}: {why}", n.id));
            }
        }
        assert!(parts(&frame(json!({"kind": "bar", "labels": ["A"], "series": [{"values": [1]}]}), 4.0, 4.0)).is_empty(), "no room");
    }

    /// A stack with a 0 in it: the 0 draws nothing, the segments either side of it meet, every
    /// bar stays between the axis and its top, and the total is still written over the stack.
    #[test]
    fn a_zero_in_a_stack_draws_nothing() {
        let data = json!([{"name": "Run", "values": [20, 0, 15, 0.001]}, {"name": "Ride", "values": [0, 30, 10, 25]}, {"name": "Swim", "values": [10, 5, 0, 0]}]);
        let stack = parts(&frame(json!({"kind": "bar", "stacked": true, "values": true, "decimals": 0, "labels": ["Mo", "Tu", "We", "Th"], "series": data}), 360.0, 220.0));
        for gone in ["bar-1-0", "bar-0-1", "bar-2-2", "bar-2-3", "bar-0-3"] {
            assert!(stack.iter().all(|n| n.id != format!("spend.{gone}")), "{gone} is a 0");
        }
        let zero = part(&stack, "grid-0").y;
        for n in stack.iter().filter(|n| n.kind == NodeKind::Rect && n.id.contains(".bar-")) {
            assert!(n.y + n.h <= zero + 0.5 && n.h > 0.0, "{} from {} to {} under the axis at {zero}", n.id, n.y, n.y + n.h);
        }
        let (run, swim) = (part(&stack, "bar-0-0"), part(&stack, "bar-2-0"));
        assert!((swim.y + swim.h - run.y).abs() <= 0.5, "swim sits on run, past ride's 0");
        assert!(swim.radii.unwrap()[0] > 0.0 && run.radii.unwrap()[0] == 0.0, "the top is round");
        assert!(part(&stack, "bar-1-2").radii.unwrap()[0] > 0.0, "ride tops Wednesday: swim's 0 above it draws nothing");
        assert_eq!(part(&stack, "value-2-3").text.as_deref(), Some("25"), "the total over a stack whose last is 0");
        assert!(part(&stack, "value-2-3").y < part(&stack, "bar-1-3").y, "written above the stack");
    }
}
