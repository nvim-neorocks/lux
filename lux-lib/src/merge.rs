use std::collections::HashMap;
use std::hash::Hash;

use itertools::Itertools;
use nonempty::NonEmpty;

pub trait Merge {
    fn merge(self, other: Self) -> Self;
}

impl<K, V> Merge for HashMap<K, V>
where
    K: Eq + Hash,
    V: Merge,
{
    fn merge(self, other: HashMap<K, V>) -> Self {
        self.into_iter()
            .chain(other)
            .into_group_map()
            .into_iter()
            .map(|(k, values)| (k, values.into_iter().reduce(|a, b| a.merge(b)).unwrap()))
            .collect()
    }
}

impl<T> Merge for Vec<T>
where
    T: PartialEq,
{
    fn merge(self, other: Self) -> Self {
        self.into_iter().chain(other).dedup().collect()
    }
}

impl<T> Merge for NonEmpty<T>
where
    T: PartialEq,
{
    fn merge(self, other: Self) -> Self {
        NonEmpty::from_vec(self.into_iter().chain(other).dedup().collect()).unwrap()
    }
}

impl<T> Merge for Option<T>
where
    T: Merge,
{
    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (None, None) => None,
            (None, b) => b,
            (a, None) => a,
            (Some(a), Some(b)) => Some(a.merge(b)),
        }
    }
}
