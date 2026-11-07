//! History buffers and braille/block graphs.

use std::collections::VecDeque;

use super::canvas::{Canvas, Rect};
use crate::config::GraphSymbol;
use crate::theme::{Gradient, Rgb, Style};

/// A bounded history of samples.
#[derive(Clone, Debug)]
pub struct GraphData {
    values: VecDeque<f64>,
    capacity: usize,
}

impl Default for GraphData {
    fn default() -> Self {
        Self::new(512)
    }
}

impl GraphData {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            values: VecDeque::with_capacity(capacity.min(4096)),
            capacity: capacity.clamp(1, 4096),
        }
    }

    pub fn push(&mut self, value: f64) {
        let value = if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        };
        if self.values.len() == self.capacity {
            self.values.pop_front();
        }
        self.values.push_back(value);
    }

    pub fn resize_capacity(&mut self, capacity: usize) {
        self.capacity = capacity.clamp(1, 4096);
        while self.values.len() > self.capacity {
            self.values.pop_front();
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    #[must_use]
    pub fn last(&self) -> f64 {
        self.values.back().copied().unwrap_or(0.0)
    }

    #[must_use]
    pub fn max(&self) -> f64 {
        self.values.iter().copied().fold(0.0, f64::max)
    }

    #[must_use]
    pub fn average(&self) -> f64 {
        if self.values.is_empty() {
            0.0
        } else {
            self.values.iter().sum::<f64>() / self.values.len() as f64
        }
    }

    /// Most recent `count` values, oldest first; short histories are padded
    /// with zeros on the left so graphs grow from the right edge.
    #[must_use]
    pub fn recent(&self, count: usize) -> Vec<f64> {
        let mut out = vec![0.0; count];
        let available = self.values.len().min(count);
        let start = count - available;
        for (slot, value) in out[start..]
            .iter_mut()
            .zip(self.values.iter().skip(self.values.len() - available))
        {
            *slot = *value;
        }
        out
    }

    pub fn values(&self) -> impl Iterator<Item = f64> + '_ {
        self.values.iter().copied()
    }
}

/// Pick a stable, readable upper bound for an auto-scaled rate graph.
#[must_use]
pub fn nice_rate_scale(max: f64) -> f64 {
    if !max.is_finite() || max <= 0.0 {
        return 1024.0;
    }
    let mut unit = 1.0f64;
    while max > unit * 1024.0 && unit < 1024f64.powi(5) {
        unit *= 1024.0;
    }
    let scaled = max / unit;
    let nice = if scaled <= 1.0 {
        1.0
    } else if scaled <= 2.0 {
        2.0
    } else if scaled <= 5.0 {
        5.0
    } else if scaled <= 10.0 {
        10.0
    } else if scaled <= 20.0 {
        20.0
    } else if scaled <= 50.0 {
        50.0
    } else if scaled <= 100.0 {
        100.0
    } else if scaled <= 200.0 {
        200.0
    } else if scaled <= 500.0 {
        500.0
    } else {
        1024.0
    };
    (nice * unit).max(1024.0)
}

const BRAILLE_LEFT: [u32; 4] = [0x01, 0x02, 0x04, 0x40];
const BRAILLE_RIGHT: [u32; 4] = [0x08, 0x10, 0x20, 0x80];
const BLOCK_LEVELS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
const TTY_LEVELS: [char; 8] = [' ', '.', ':', '-', '=', '+', '*', '#'];

/// Draw a graph into `rect`.
///
/// `max` is the value that maps to the top of the graph. Values above it are
/// clamped, so an unexpected spike can never draw outside the rectangle.
pub fn draw(
    canvas: &mut Canvas,
    rect: Rect,
    data: &GraphData,
    max: f64,
    gradient: Gradient,
    symbol: GraphSymbol,
    baseline: Style,
) {
    let rect = rect.intersect(canvas.area());
    if rect.is_empty() {
        return;
    }
    let max = if max.is_finite() && max > 0.0 {
        max
    } else {
        1.0
    };
    match symbol {
        GraphSymbol::Braille => draw_braille(canvas, rect, data, max, gradient, baseline),
        GraphSymbol::Block => draw_blocks(canvas, rect, data, max, gradient, baseline),
        GraphSymbol::Tty => draw_tty(canvas, rect, data, max, gradient, baseline),
    }
}

fn draw_braille(
    canvas: &mut Canvas,
    rect: Rect,
    data: &GraphData,
    max: f64,
    gradient: Gradient,
    baseline: Style,
) {
    let samples = data.recent(usize::from(rect.w) * 2);
    let total_dots = u32::from(rect.h) * 4;
    for cell_x in 0..rect.w {
        let left = (samples[usize::from(cell_x) * 2] / max).clamp(0.0, 1.0) as f32;
        let right = (samples[usize::from(cell_x) * 2 + 1] / max).clamp(0.0, 1.0) as f32;
        let left_filled = (left * total_dots as f32).ceil() as u32;
        let right_filled = (right * total_dots as f32).ceil() as u32;
        let style = Style::fg(gradient.at(left.max(right)));
        for cell_row in 0..rect.h {
            let base = u32::from(rect.h - 1 - cell_row) * 4;
            let mut mask = 0u32;
            for dot in 0..4u32 {
                if base + dot < left_filled {
                    mask |= BRAILLE_LEFT[3 - dot as usize];
                }
                if base + dot < right_filled {
                    mask |= BRAILLE_RIGHT[3 - dot as usize];
                }
            }
            let y = rect.y + cell_row;
            if mask == 0 {
                if cell_row + 1 == rect.h && left == 0.0 && right == 0.0 {
                    canvas.put(rect.x + cell_x, y, '⣀', baseline);
                } else {
                    canvas.put(rect.x + cell_x, y, ' ', Style::plain());
                }
            } else {
                let ch = char::from_u32(0x2800 | mask).unwrap_or(' ');
                canvas.put(rect.x + cell_x, y, ch, style);
            }
        }
    }
}

fn draw_blocks(
    canvas: &mut Canvas,
    rect: Rect,
    data: &GraphData,
    max: f64,
    gradient: Gradient,
    baseline: Style,
) {
    let samples = data.recent(usize::from(rect.w));
    for (index, sample) in samples.iter().enumerate() {
        let value = (sample / max).clamp(0.0, 1.0) as f32;
        let x = rect.x + index as u16;
        let style = Style::fg(gradient.at(value));
        // Height in cells, bottom-aligned.
        let filled = value * f32::from(rect.h);
        for cell_row in 0..rect.h {
            let y = rect.y + cell_row;
            let cell_from_bottom = u32::from(rect.h - 1 - cell_row);
            let cell_value = (filled - cell_from_bottom as f32).clamp(0.0, 1.0);
            let level = (cell_value * 8.0).ceil() as usize;
            if level == 0 {
                if cell_row + 1 == rect.h {
                    canvas.put(x, y, '▁', baseline);
                } else {
                    canvas.put(x, y, ' ', Style::plain());
                }
            } else {
                canvas.put(x, y, BLOCK_LEVELS[level.min(8)], style);
            }
        }
    }
}

fn draw_tty(
    canvas: &mut Canvas,
    rect: Rect,
    data: &GraphData,
    max: f64,
    gradient: Gradient,
    baseline: Style,
) {
    let samples = data.recent(usize::from(rect.w));
    for (index, sample) in samples.iter().enumerate() {
        let value = (sample / max).clamp(0.0, 1.0) as f32;
        let x = rect.x + index as u16;
        let style = Style::fg(gradient.at(value));
        let filled = value * f32::from(rect.h);
        for cell_row in 0..rect.h {
            let y = rect.y + cell_row;
            let cell_from_bottom = u32::from(rect.h - 1 - cell_row);
            let cell_value = (filled - cell_from_bottom as f32).clamp(0.0, 1.0);
            if cell_value <= 0.0 {
                if cell_row + 1 == rect.h {
                    canvas.put(x, y, '.', baseline);
                } else {
                    canvas.put(x, y, ' ', Style::plain());
                }
            } else {
                let level = (cell_value * 7.0).ceil() as usize;
                canvas.put(x, y, TTY_LEVELS[level.clamp(1, 7)], style);
            }
        }
    }
}

/// Blend a value into the theme's temperature gradient.
#[must_use]
pub fn temperature_color(temperature: f32, gradient: Gradient) -> Rgb {
    let ratio = ((temperature - 30.0) / 70.0).clamp(0.0, 1.0);
    gradient.at(ratio)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Gradient, Rgb};

    fn gradient() -> Gradient {
        Gradient::new(
            Rgb::new(0, 0, 0),
            Rgb::new(100, 100, 100),
            Rgb::new(255, 255, 255),
        )
    }

    #[test]
    fn history_is_bounded_and_right_aligned() {
        let mut data = GraphData::new(3);
        for value in [1.0, 2.0, 3.0, 4.0, 5.0] {
            data.push(value);
        }
        assert_eq!(data.len(), 3);
        assert_eq!(data.recent(5), vec![0.0, 0.0, 3.0, 4.0, 5.0]);
        assert_eq!(data.last(), 5.0);
        assert_eq!(data.max(), 5.0);
    }

    #[test]
    fn history_rejects_nan() {
        let mut data = GraphData::new(2);
        data.push(f64::NAN);
        data.push(-4.0);
        assert_eq!(data.recent(2), vec![0.0, 0.0]);
    }

    #[test]
    fn default_history_is_not_degenerate() {
        let mut data = GraphData::default();
        for value in 1..10 {
            data.push(f64::from(value));
        }
        assert_eq!(data.len(), 9);
        assert!(data.capacity >= 9);
    }

    #[test]
    fn nice_scales_are_round() {
        assert_eq!(nice_rate_scale(0.0), 1024.0);
        assert_eq!(nice_rate_scale(900.0), 1024.0);
        assert_eq!(nice_rate_scale(1536.0), 2048.0);
        assert_eq!(
            nice_rate_scale(5.0 * 1024.0 * 1024.0),
            5.0 * 1024.0 * 1024.0
        );
    }

    #[test]
    fn graphs_draw_inside_their_rectangle() {
        let mut canvas = Canvas::new(20, 4);
        let mut data = GraphData::new(20);
        for value in 0..20 {
            data.push(f64::from(value % 10) * 10.0);
        }
        let rect = Rect::new(2, 1, 10, 2);
        draw(
            &mut canvas,
            rect,
            &data,
            100.0,
            gradient(),
            GraphSymbol::Braille,
            Style::default(),
        );
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                if !rect.contains(x, y) {
                    assert_eq!(canvas.cell(x, y).ch, ' ', "outside cell changed at {x},{y}");
                }
            }
        }
        assert!(canvas.plain().contains('⣀') || canvas.plain().contains('⣿'));
    }

    #[test]
    fn zero_values_show_a_baseline() {
        let mut canvas = Canvas::new(8, 1);
        let data = GraphData::new(8);
        draw(
            &mut canvas,
            Rect::new(0, 0, 8, 1),
            &data,
            100.0,
            gradient(),
            GraphSymbol::Braille,
            Style::default(),
        );
        assert_eq!(canvas.plain(), "⣀⣀⣀⣀⣀⣀⣀⣀");
    }
}
