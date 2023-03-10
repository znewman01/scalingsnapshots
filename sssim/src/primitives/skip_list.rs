use crate::util::DataSized;
use crate::util::Information;

fn find_max_pow(mut index: usize) -> usize {
    if index == 0 {
        return 0;
    }
    let mut max_pow = 0;
    while index % 2 == 0 {
        max_pow += 1;
        index = index >> 1;
    }
    max_pow
}

pub trait Collector {
    type Item: Clone;
    type Proof: Clone;

    fn init(item: &Self::Item) -> Self;
    fn collect(&mut self, item: &Self::Item);
    fn to_proof(&self, item: &Self::Item) -> Self::Proof;
}

#[derive(Debug, Clone)]
pub struct SkipList<C: Collector> {
    entries: Vec<SkipListEntry<C::Item, C::Proof>>,
}

impl<C: Collector> Default for SkipList<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: Collector> DataSized for SkipList<C> {
    fn size(&self) -> Information {
        todo!();
    }
}

//add
//update entry
impl<C: Collector> SkipList<C> {
    pub fn new() -> Self {
        Self { entries: vec![] }
    }

    pub fn add(&mut self, item: C::Item) {
        let entry = SkipListEntry::<C::Item, C::Proof>::new(item);
        let mut collector = C::init(&entry.item);
        let max_pow = find_max_pow(self.entries.len());

        for (e, i) in self.entries.iter_mut().rev().zip(1..=(1 << max_pow)) {
            if i & (i - 1) == 0 {
                e.proofs.push(collector.to_proof(&e.item));
            }
        }
        self.entries.push(entry);
    }

    pub fn read(&self, start: usize, end: usize) -> (Vec<C::Proof>, Vec<C::Item>) {
        assert!(start <= end);
        assert!(end < self.entries.len());

        let mut cur = start;
        let mut proof_list = vec![];
        let mut value_list = vec![];

        while cur < end {
            let cur_entry = &self.entries[cur];
            let (proof, offset) = cur_entry.find_next(end - cur);
            proof_list.push(proof);
            value_list.push(cur_entry.item.clone());
            cur += offset;
        }

        (proof_list, value_list)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[derive(Debug, Clone)]
struct SkipListEntry<I, P> {
    item: I,
    proofs: Vec<P>,
}

impl<I, P> SkipListEntry<I, P>
where
    P: Clone,
{
    fn new(item: I) -> Self {
        Self {
            item,
            proofs: vec![],
        }
    }

    fn find_next(&self, offset: usize) -> (P, usize) {
        let mut i = 0;
        while offset >> i > 0 {
            i += 1
        }
        (self.proofs[i - 1].clone(), 1 << (i - 1))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    impl Collector for usize {
        type Item = usize;
        type Proof = (usize, usize);

        fn init(item: &Self::Item) -> Self {
            *item
        }

        fn collect(&mut self, item: &Self::Item) {
            return;
        }

        fn to_proof(&self, item: &Self::Item) -> Self::Proof {
            return (*item, *self);
        }
    }

    #[test]
    fn test_find_max_pow() {
        assert_eq!(find_max_pow(1), 0);
        assert_eq!(find_max_pow(2), 1);
        assert_eq!(find_max_pow(3), 0);
        assert_eq!(find_max_pow(4), 2);
        assert_eq!(find_max_pow(5), 0);
        assert_eq!(find_max_pow(6), 1);
        assert_eq!(find_max_pow(7), 0);
        assert_eq!(find_max_pow(8), 3);
    }

    #[test]
    fn test_skip_list() {
        let mut list = SkipList::<(usize, usize)>::default();
        list.add(0);
        list.add(1);
        let (p, v) = list.read(0, 1);
        assert!(p.len() == 1);
        assert!(v.len() == 1);
        assert!(p[0] == (0, 1));
        assert!(v[0] == 0);
        list.add(2);
        list.add(3);
        list.add(4);
        list.add(5);
        list.add(6);
        list.add(7);

        println!("{:?}", list);

        let (p, v) = list.read(0, 6);
        assert_eq!(p, vec![(0, 4), (4, 6)]);
        assert_eq!(v, vec![0, 4]);
    }
}
