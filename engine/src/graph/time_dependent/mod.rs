use super::*;
use std::{
    iter::Peekable,
    cmp::{min, max}
};

use ::sorted_search_slice_ext::SortedSearchSliceExt;

mod piecewise_linear_function;
use self::piecewise_linear_function::*;

mod shortcut_source;
use self::shortcut_source::*;

mod shortcut;
pub use self::shortcut::*;

mod linked;
pub use self::linked::*;

mod wrapping_range;
pub use self::wrapping_range::*;

mod graph;
pub use self::graph::Graph as TDGraph;

pub mod shortcut_graph;
pub use self::shortcut_graph::ShortcutGraph;

mod wrapping_slice_iter;
use self::wrapping_slice_iter::WrappingSliceIter;

mod intersections;
use self::intersections::*;

pub type Timestamp = Weight;

#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static TEST_PERIOD_MOCK: Cell<Option<Timestamp>> = Cell::new(None);
}

#[cfg(test)]
unsafe fn set_period(period: Timestamp) {
    TEST_PERIOD_MOCK.with(|period_cell| period_cell.set(Some(period)))
}

#[cfg(test)]
unsafe fn reset_period() {
    TEST_PERIOD_MOCK.with(|period_cell| period_cell.set(None))
}

#[cfg(test)] use std::panic;
#[cfg(test)]
fn run_test_with_periodicity<T>(period: Timestamp, test: T) -> ()
    where T: FnOnce() -> () + panic::UnwindSafe
{
    unsafe { set_period(period) };

    let result = panic::catch_unwind(|| {
        test()
    });

    unsafe { reset_period() };

    assert!(result.is_ok())
}

#[cfg(test)]
fn period() -> Timestamp {
    return TEST_PERIOD_MOCK.with(|period_cell| period_cell.get().expect("period() used but not set"));
}

#[cfg(not(test))]
#[inline]
const fn period() -> Timestamp {
    86_400_000
}


const TOLERANCE: Weight = 10;

fn abs_diff(x: Weight, y: Weight) -> Weight {
    max(x, y) - min(x, y)
}

use std::ops::Sub;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TTIpp {
    at: Timestamp,
    val: Weight,
}

impl TTIpp {
    fn new(at: Timestamp, val: Weight) -> Ipp {
        Ipp { at, val }
    }

    fn as_tuple(self) -> (Timestamp, Weight) {
        (self.at, self.val)
    }

    fn into_atipp(self) -> ATIpp {
        let TTIpp { at, val } = self;
        debug_assert!(at < period());
        ATIpp { at, val: (at + val) % period() }
    }
}

impl Sub for TTIpp {
    type Output = Point;

    fn sub(self, other: Self) -> Self::Output {
        Point { x: i64::from(self.at) - i64::from(other.at), y: i64::from(self.val) - i64::from(other.val) }
    }
}

#[derive(Debug)]
pub struct Point {
    x: i64, y: i64
}

type Ipp = TTIpp;

#[derive(Debug, Clone, Copy, PartialEq)]
struct ATIpp {
    at: Timestamp,
    val: Timestamp,
}

impl ATIpp {
    fn new(at: Timestamp, val: Weight) -> ATIpp {
        ATIpp { at, val }
    }

    fn as_tuple(self) -> (Timestamp, Weight) {
        (self.at, self.val)
    }

    fn into_ttipp(self) -> TTIpp {
        let ATIpp { at, val } = self;
        debug_assert!(at <= val);
        TTIpp { at, val: val - at }
    }

    fn shift(&mut self) {
        self.at += period();
        self.val += period();
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Line<Point> {
    from: Point,
    to: Point,
}

impl<Point> Line<Point> {
    fn new(from: Point, to: Point) -> Self {
        Line { from, to }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Segment<LineType> {
    line: LineType,
    valid: Range<Timestamp>, // TODO Wrapping - NO
}

type TTFSeg = Segment<Line<Ipp>>; // TODO always monotone

impl TTFSeg {
    fn new((from_at, from_val): (Timestamp, Weight), (to_at, to_val): (Timestamp, Weight)) -> Self {
        Segment { line: Line { from: Ipp::new(from_at, from_val), to: Ipp::new(to_at, to_val) }, valid: Range { start: from_at, end: to_at } }
    }

    fn is_equivalent_to(&self, other: &Self) -> bool {
        if self == other { return true }
        if self.valid != other.valid { return false }
        let self_line = self.line.clone().into_monotone_tt_line();
        let other_line = other.line.clone().into_monotone_tt_line();
        if self_line.delta_x() * other_line.delta_y() != self_line.delta_y() * other_line.delta_x()  { return false }
        let delta = if self.line.from != other.line.from {
            self.line.from - other.line.from
        } else {
            self.line.to - other.line.to
        };
        if delta.x * i64::from(self_line.delta_y()) != delta.y * i64::from(self_line.delta_x()) { return false }
        true
    }

    fn into_monotone_at_segment(self) -> Segment<MonotoneLine<ATIpp>> {
        let Segment { line, mut valid } = self;
        let line = line.into_monotone_at_line();
        if valid.end < valid.start { // TODO <= ?
            valid.end += period();
        }
        Segment { line, valid }
    }

    fn eval(&self, x: Timestamp) -> Weight {
        debug_assert!(self.valid.contains(&x));
        // TODO optimize
        self.line.clone().into_monotone_tt_line().interpolate_tt(x)
    }

    fn start_of_valid_at_val(&self) -> Timestamp {
        (self.line.clone().into_monotone_tt_line().interpolate_tt(self.valid.start) + self.valid.start) % period()
    }

    fn end_of_valid_at_val(&self) -> Timestamp {
        let x = if self.valid.end <= self.valid.start { self.valid.end + period() } else { self.valid.end };
        (self.line.clone().into_monotone_tt_line().interpolate_tt(x) + x) % period()
    }
}

impl Segment<MonotoneLine<ATIpp>> {
    fn valid_value_range(&self) -> Range<Timestamp> {
        Range {
            start: self.line.interpolate_tt(self.valid.start) + self.valid.start,
            end: self.line.interpolate_tt(self.valid.end) + self.valid.end
        }
    }

    fn shift(&mut self) {
        self.valid.start += period();
        self.valid.end += period();
        self.line.shift();
    }
}

impl Line<TTIpp> {
    fn into_monotone_tt_line(self) -> MonotoneLine<TTIpp> {
        let Line { from, mut to } = self;
        if to.at < from.at {
            to.at += period();
        }
        MonotoneLine(Line { from, to })
    }

    fn into_monotone_at_line(self) -> MonotoneLine<ATIpp> {
        let Line { from, mut to } = self;
        if to.at < from.at {
            to.at += period();
        }
        MonotoneLine(Line { from: ATIpp { at: from.at, val: from.at + from.val }, to: ATIpp { at: to.at, val: to.at + to.val } })
    }

    fn delta_x(&self) -> Weight {
        self.to.at - self.from.at
    }

    fn delta_y(&self) -> Weight {
        self.to.val - self.from.val
    }
}

impl Line<ATIpp> {
    fn delta_x(&self) -> Weight {
        self.to.at - self.from.at
    }

    fn delta_y(&self) -> Weight {
        self.to.val - self.from.val
    }

    fn shift(&mut self) {
        self.from.shift();
        self.to.shift();
    }
}

#[derive(Debug, Clone, PartialEq)]
struct MonotoneLine<Point>(Line<Point>);

impl MonotoneLine<ATIpp> {
    #[inline]
    fn interpolate_tt(&self, x: Timestamp) -> Weight {
        debug_assert!(x >= self.0.from.at, "self: {:?}, x: {}", self, x);
        debug_assert!(x <= self.0.to.at, "self: {:?}, x: {}", self, x);
        debug_assert!(self.0.from.at < self.0.to.at, "self: {:?}", self);
        debug_assert!(self.0.from.val <= self.0.to.val, "self: {:?}", self);
        let delta_x = self.0.to.at - self.0.from.at;
        let delta_y = self.0.to.val - self.0.from.val;
        let relative_x = x - self.0.from.at;
        let result = u64::from(self.0.from.val) + (u64::from(relative_x) * u64::from(delta_y) / u64::from(delta_x));
        result as Weight - x
    }

    fn delta_x(&self) -> Weight {
        self.0.delta_x()
    }

    fn delta_y(&self) -> Weight {
        self.0.delta_y()
    }

    fn into_monotone_tt_line(self) -> MonotoneLine<TTIpp> {
        let MonotoneLine(Line { from, to }) = self;
        let from = from.into_ttipp();
        let to = to.into_ttipp();
        MonotoneLine(Line { from, to })
    }

    fn shift(&mut self) {
        self.0.shift();
    }
}

impl MonotoneLine<TTIpp> {
    #[inline]
    fn interpolate_tt(&self, x: Timestamp) -> Weight {
        debug_assert!(self.0.from.at < self.0.to.at, "self: {:?}", self);
        let delta_x = self.0.to.at - self.0.from.at;
        let delta_y = i64::from(self.0.to.val) - i64::from(self.0.from.val);
        let relative_x = i64::from(x) - i64::from(self.0.from.at);
        let result = i64::from(self.0.from.val) + (relative_x * delta_y / i64::from(delta_x));
        debug_assert!(result >= 0);
        debug_assert!(result <= i64::from(INFINITY));
        result as Weight
    }

    fn apply_periodicity(self, period: Timestamp) -> Line<TTIpp> {
        let MonotoneLine(Line { mut from, mut to }) = self;
        from.at %= period;
        from.val %= period;
        to.at %= period;
        to.val %= period;
        Line { from, to }
    }

    fn delta_x(&self) -> Weight {
        self.0.delta_x()
    }

    fn delta_y(&self) -> Weight {
        self.0.delta_y()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ttipp_to_atipp() {
        run_test_with_periodicity(10, || {
            assert_eq!(TTIpp::new(2, 2).into_atipp(), ATIpp::new(2, 4));
            assert_eq!(TTIpp::new(6, 5).into_atipp(), ATIpp::new(6, 1));
        });
    }

    #[test]
    fn test_tt_line_to_monotone_at_line() {
        run_test_with_periodicity(10, || {
            assert_eq!(Line { from: TTIpp::new(2, 2), to: TTIpp::new(4, 5) }.into_monotone_at_line(),
                MonotoneLine(Line { from: ATIpp::new(2, 4), to: ATIpp::new(4, 9) }));
            assert_eq!(Line { from: TTIpp::new(4, 5), to: TTIpp::new(2, 2) }.into_monotone_at_line(),
                MonotoneLine(Line { from: ATIpp::new(4, 9), to: ATIpp::new(12, 14) }));
            assert_eq!(Line { from: TTIpp::new(8, 3), to: TTIpp::new(2, 2) }.into_monotone_at_line(),
                MonotoneLine(Line { from: ATIpp::new(8, 11), to: ATIpp::new(12, 14) }));
        });
    }
}
