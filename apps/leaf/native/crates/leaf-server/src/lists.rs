//! Your own lists (the owner, 2026-11-26: "a wait list for the next series"), kept whole in the
//! kv store as `lists` ([`leaf_proto::List`]): `GET /v1/lists`, and `POST /v1/lists` with every
//! list, tidied ([`leaf_proto::tidy_lists`]). A series merged into another is on its lists in its
//! place; one removed from My space leaves them. Nothing here asks a site.

use leaf_proto::{List, tidy_lists};

const KEY: &str = "lists";

/// `lists` with `from` named `to` instead (kept once where `to` already was).
pub fn renamed(mut lists: Vec<List>, from: i64, to: i64) -> Vec<List> {
    for l in &mut lists {
        for s in &mut l.series {
            if *s == from {
                *s = to;
            }
        }
    }
    tidy_lists(lists)
}

/// `lists` without `id`.
pub fn without(mut lists: Vec<List>, id: i64) -> Vec<List> {
    for l in &mut lists {
        l.series.retain(|s| *s != id);
    }
    lists
}

impl crate::autopilot::Ctx {
    /// `GET /v1/lists`.
    pub fn lists(&self) -> Vec<List> {
        self.store().get(KEY).and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default()
    }

    /// `POST /v1/lists`: the whole set, tidied; answers what is kept.
    pub fn set_lists(&self, lists: Vec<List>) -> Vec<List> {
        let lists = tidy_lists(lists);
        self.store().set(KEY, &serde_json::to_string(&lists).unwrap_or_else(|_| "[]".into()));
        lists
    }

    /// A merge: the other series' place on lists is the kept one's.
    pub fn lists_renamed(&self, from: i64, to: i64) {
        let l = self.lists();
        if l.iter().any(|l| l.series.contains(&from)) {
            self.set_lists(renamed(l, from, to));
        }
    }

    /// A series removed from My space leaves the lists.
    pub fn lists_without(&self, id: i64) {
        let l = self.lists();
        if l.iter().any(|l| l.series.contains(&id)) {
            self.set_lists(without(l, id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_merge_keeps_the_place_and_a_removal_lets_go() {
        let l = |series: Vec<i64>| List { id: 1, name: "Next".into(), series, home: true, made_at: 0 };
        assert_eq!(renamed(vec![l(vec![4, 9, 2])], 9, 7)[0].series, vec![4, 7, 2], "in the other's place");
        assert_eq!(renamed(vec![l(vec![7, 9])], 9, 7)[0].series, vec![7], "once, where the kept one was");
        assert_eq!(without(vec![l(vec![4, 9])], 4)[0].series, vec![9]);
    }
}
