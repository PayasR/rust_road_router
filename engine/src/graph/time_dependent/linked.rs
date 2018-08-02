use super::*;
use rank_select_map::BitVec;
use math::RangeExtensions;

#[derive(Debug, Clone, Copy)]
pub struct Linked {
    first: EdgeId,
    second: EdgeId
}

impl Linked {
    pub fn new(first: EdgeId, second: EdgeId) -> Linked {
        Linked { first, second }
    }

    pub fn evaluate(self, departure: Timestamp, shortcut_graph: &ShortcutGraph) -> Weight {
        debug_assert!(departure < period());
        let first_edge = shortcut_graph.get_incoming(self.first);
        let second_edge = shortcut_graph.get_outgoing(self.second);
        let first_edge_value = first_edge.evaluate(departure, shortcut_graph);
        first_edge_value + second_edge.evaluate((departure + first_edge_value) % period(), shortcut_graph)
    }

    pub fn bounds(self, shortcut_graph: &ShortcutGraph) -> (Weight, Weight) {
        debug_assert!(self.is_valid_path(shortcut_graph));
        let (first_min, first_max) = shortcut_graph.get_incoming(self.first).bounds();
        debug_assert!(first_min < INFINITY);
        debug_assert!(first_max < INFINITY);
        let (second_min, second_max) = shortcut_graph.get_outgoing(self.second).bounds();
        debug_assert!(second_min < INFINITY);
        debug_assert!(second_max < INFINITY);

        debug_assert!(first_min + second_min < INFINITY);
        debug_assert!(first_max + second_max < INFINITY);
        (first_min + second_min, first_max + second_max)
    }

    pub fn bounds_for(self, range: &Range<Timestamp>, shortcut_graph: &ShortcutGraph) -> (Weight, Weight) {
        let (in_min, in_max) = shortcut_graph.get_incoming(self.first).bounds_for(range, shortcut_graph);
        let (first_range, second_range) = (range.start + in_min .. range.end + in_max).split(period());
        let (out_first_min, out_first_max) = shortcut_graph.get_outgoing(self.second).bounds_for(&first_range, shortcut_graph);
        let (out_second_min, out_second_max) = shortcut_graph.get_outgoing(self.second).bounds_for(&second_range, shortcut_graph);
        (in_min + min(out_first_min, out_second_min), in_max + max(out_first_max, out_second_max))
    }

    pub fn as_shortcut_data(self) -> ShortcutData {
        ShortcutData::new(ShortcutSource::Shortcut(self.first, self.second))
    }

    pub fn is_valid_path(self, shortcut_graph: &ShortcutGraph) -> bool {
        shortcut_graph.get_incoming(self.first).is_valid_path() && shortcut_graph.get_outgoing(self.second).is_valid_path()
    }

    pub fn unpack(self, shortcut_graph: &ShortcutGraph, unpacked_shortcuts: &mut BitVec, original_edges: &mut BitVec) {
        if !unpacked_shortcuts.get(self.first as usize * 2) {
            unpacked_shortcuts.set(self.first as usize * 2);
            shortcut_graph.get_incoming(self.first).unpack(shortcut_graph, unpacked_shortcuts, original_edges);
        }
        if !unpacked_shortcuts.get(self.second as usize * 2 + 1) {
            unpacked_shortcuts.set(self.second as usize * 2 + 1);
            shortcut_graph.get_outgoing(self.second).unpack(shortcut_graph, unpacked_shortcuts, original_edges);
        }
    }

    pub fn debug_to_s(self, shortcut_graph: &ShortcutGraph, indent: usize) -> String {
        println!("{:?}", self);
        let first_edge = shortcut_graph.get_incoming(self.first);
        let second_edge = shortcut_graph.get_outgoing(self.second);
        format!("Linked:\n{}first({}): {}\n{}second({}): {}",
            String::from(" ").repeat(indent * 2),
            self.first,
            first_edge.debug_to_s(shortcut_graph, indent + 1),
            String::from(" ").repeat(indent * 2),
            self.second,
            second_edge.debug_to_s(shortcut_graph, indent + 1))
    }

    pub fn validate_does_not_contain(self, edge_id: EdgeId, shortcut_graph: &ShortcutGraph) {
        assert_ne!(edge_id, self.first);
        assert_ne!(edge_id, self.second);
        shortcut_graph.get_incoming(self.first).validate_does_not_contain(edge_id, shortcut_graph);
        shortcut_graph.get_incoming(self.second).validate_does_not_contain(edge_id, shortcut_graph);
    }
}

#[derive(Clone, Debug)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct LazyChain<I, F> {
    iter: I,
    produce_second: Option<F>,
}

impl<I, F> Iterator for LazyChain<I, F> where
    I: Iterator,
    F: FnOnce() -> I
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<I::Item> {
        match self.iter.next() {
            elt @ Some(..) => elt,
            None => {
                if let Some(f) = self.produce_second.take() {
                    self.iter = f();
                }
                self.iter.next()
            }
        }
    }
}

trait LazyChainIterExt: Sized {
    fn lazy_chain<F>(self, f: F) -> LazyChain<Self, F> {
        LazyChain { iter: self, produce_second: Some(f) }
    }
}

impl<T: Iterator> LazyChainIterExt for T {}
