//! The reader's strip: every chapter of a series one after the other, each a slim heading then its
//! pages, and an end mark after the last. The place in it is kept as an item and how far down that
//! item the top of the screen is, never as one offset from the top: a page above the screen whose
//! height arrives late (it was decoded after a placeholder stood in for it) then moves nothing on
//! screen.

/// One thing in the strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Item {
    /// Chapter `c`'s heading ("Chapter 13"); while its page count is unknown it stands for the
    /// whole chapter.
    Head(usize),
    /// Page `n` (0-based) of chapter `c`.
    Page(usize, u32),
    /// After the last chapter.
    End,
}

/// What the strip needs to know of the series: how many chapters, each one's page count (none
/// while unknown) and every item's height in layout units.
pub trait Book {
    fn chapters(&self) -> usize;
    fn pages(&self, chapter: usize) -> Option<u32>;
    fn height(&self, item: Item) -> f32;
}

/// A place: `off` units down `item` is the top of the screen. Always 0 ≤ off < its height once
/// [`normalize`]d.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pos {
    pub item: Item,
    pub off: f32,
}

pub fn next(b: &dyn Book, item: Item) -> Option<Item> {
    match item {
        Item::Head(c) => match b.pages(c) {
            Some(n) if n > 0 => Some(Item::Page(c, 0)),
            _ => Some(if c + 1 < b.chapters() { Item::Head(c + 1) } else { Item::End }),
        },
        Item::Page(c, n) => {
            if b.pages(c).is_some_and(|p| n + 1 < p) {
                Some(Item::Page(c, n + 1))
            } else if c + 1 < b.chapters() {
                Some(Item::Head(c + 1))
            } else {
                Some(Item::End)
            }
        }
        Item::End => None,
    }
}

pub fn prev(b: &dyn Book, item: Item) -> Option<Item> {
    let last_of = |c: usize| match b.pages(c) {
        Some(n) if n > 0 => Item::Page(c, n - 1),
        _ => Item::Head(c),
    };
    match item {
        Item::Head(0) => None,
        Item::Head(c) => Some(last_of(c - 1)),
        Item::Page(c, 0) => Some(Item::Head(c)),
        Item::Page(c, n) => Some(Item::Page(c, n - 1)),
        Item::End => (b.chapters() > 0).then(|| last_of(b.chapters() - 1)),
    }
}

/// Move the place so that 0 ≤ off < the item's height, walking to the items before or after; the
/// strip's top stops it.
pub fn normalize(b: &dyn Book, mut p: Pos) -> Pos {
    // Bounded, so a strip of zero-height items can never spin.
    for _ in 0..100_000 {
        if p.off < 0.0 {
            match prev(b, p.item) {
                Some(i) => {
                    p.item = i;
                    p.off += b.height(i).max(0.0);
                }
                None => {
                    p.off = 0.0;
                    return p;
                }
            }
        } else {
            let h = b.height(p.item).max(0.0);
            if p.off < h || p.item == Item::End {
                return p;
            }
            match next(b, p.item) {
                Some(i) => {
                    p.off -= h;
                    p.item = i;
                }
                None => return p,
            }
        }
    }
    p
}

/// The place moved down by `dy` units (up when negative), kept so the end mark's bottom never
/// rises above the screen's bottom (`view_h` tall).
pub fn scroll(b: &dyn Book, p: Pos, dy: f32, view_h: f32) -> Pos {
    let p = normalize(b, Pos { item: p.item, off: p.off + dy });
    clamp_bottom(b, p, view_h)
}

/// What is left of the strip from the place down, up to `limit` units (more is not needed).
fn below(b: &dyn Book, p: Pos, limit: f32) -> (f32, bool) {
    let mut total = b.height(p.item) - p.off;
    let mut item = p.item;
    while total < limit {
        match next(b, item) {
            Some(i) => {
                item = i;
                total += b.height(i);
            }
            None => return (total, true),
        }
    }
    (total, item == Item::End)
}

/// Pull the place back so the strip's end sits at the screen's bottom rather than above it.
pub fn clamp_bottom(b: &dyn Book, p: Pos, view_h: f32) -> Pos {
    let (left, ended) = below(b, p, view_h);
    if ended && left < view_h {
        normalize(b, Pos { item: p.item, off: p.off - (view_h - left) })
    } else {
        p
    }
}

/// The items on screen (and `extra` units past its bottom), each with its top's y on screen.
pub fn visible(b: &dyn Book, p: Pos, view_h: f32, extra: f32) -> Vec<(Item, f32)> {
    let mut out = vec![(p.item, -p.off)];
    let mut y = b.height(p.item) - p.off;
    let mut item = p.item;
    while y < view_h + extra {
        match next(b, item) {
            Some(i) => {
                out.push((i, y));
                y += b.height(i);
                item = i;
            }
            None => break,
        }
    }
    out
}

/// The items above the screen, up to `extra` units, each with its top's y (negative).
pub fn above(b: &dyn Book, p: Pos, extra: f32) -> Vec<(Item, f32)> {
    let mut out = Vec::new();
    let mut y = -p.off;
    let mut item = p.item;
    while y > -extra {
        match prev(b, item) {
            Some(i) => {
                y -= b.height(i);
                out.push((i, y));
                item = i;
            }
            None => break,
        }
    }
    out
}

/// The chapter a place is in.
pub fn chapter_of(b: &dyn Book, item: Item) -> usize {
    match item {
        Item::Head(c) | Item::Page(c, _) => c,
        Item::End => b.chapters().saturating_sub(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Chapters of known page counts (or none), pages 500 tall unless set, headings 60.
    struct Fake {
        pages: Vec<Option<u32>>,
        tall: HashMap<(usize, u32), f32>,
    }

    impl Book for Fake {
        fn chapters(&self) -> usize {
            self.pages.len()
        }
        fn pages(&self, c: usize) -> Option<u32> {
            self.pages[c]
        }
        fn height(&self, item: Item) -> f32 {
            match item {
                Item::Head(_) => 60.0,
                Item::Page(c, n) => *self.tall.get(&(c, n)).unwrap_or(&500.0),
                Item::End => 100.0,
            }
        }
    }

    fn book() -> Fake {
        Fake { pages: vec![Some(3), Some(2), None], tall: HashMap::new() }
    }

    #[test]
    fn the_strip_chains_chapters() {
        let b = book();
        let mut items = vec![Item::Head(0)];
        while let Some(i) = next(&b, *items.last().unwrap()) {
            items.push(i);
        }
        use Item::*;
        assert_eq!(items, vec![Head(0), Page(0, 0), Page(0, 1), Page(0, 2), Head(1), Page(1, 0), Page(1, 1), Head(2), End]);
        // And back.
        let mut back = vec![End];
        while let Some(i) = prev(&b, *back.last().unwrap()) {
            back.push(i);
        }
        back.reverse();
        assert_eq!(back, items);
    }

    #[test]
    fn scrolling_walks_items_and_stops_at_the_top() {
        let b = book();
        let p = Pos { item: Item::Page(0, 1), off: 0.0 };
        assert_eq!(scroll(&b, p, 700.0, 800.0), Pos { item: Item::Page(0, 2), off: 200.0 });
        assert_eq!(scroll(&b, p, -520.0, 800.0), Pos { item: Item::Head(0), off: 40.0 });
        assert_eq!(scroll(&b, p, -5000.0, 800.0), Pos { item: Item::Head(0), off: 0.0 });
    }

    #[test]
    fn the_end_rests_on_the_bottom_of_the_screen() {
        let b = book();
        // 2780 units in all; the lowest place shows the last 800.
        let p = scroll(&b, Pos { item: Item::Head(0), off: 0.0 }, 1e6, 800.0);
        assert_eq!(p, Pos { item: Item::Page(1, 0), off: 360.0 });
        let shown = visible(&b, p, 800.0, 0.0);
        assert_eq!(shown.last(), Some(&(Item::End, 700.0)));
    }

    #[test]
    fn a_height_arriving_above_the_screen_moves_nothing_on_it() {
        let mut b = book();
        let p = Pos { item: Item::Page(0, 2), off: 100.0 };
        let before = visible(&b, p, 800.0, 0.0);
        b.tall.insert((0, 0), 3000.0);
        b.tall.insert((0, 1), 120.0);
        assert_eq!(visible(&b, p, 800.0, 0.0), before);
        // Only what is above it shifts.
        assert_eq!(above(&b, p, 1000.0), vec![(Item::Page(0, 1), -220.0), (Item::Page(0, 0), -3220.0)]);
        assert_eq!(above(&b, p, 200.0), vec![(Item::Page(0, 1), -220.0)]);
    }

    #[test]
    fn a_chapter_whose_pages_are_unknown_is_its_heading_alone() {
        let b = book();
        assert_eq!(next(&b, Item::Head(2)), Some(Item::End));
        assert_eq!(chapter_of(&b, Item::End), 2);
    }
}
