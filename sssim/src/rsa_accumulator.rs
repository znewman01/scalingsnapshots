use lazy_static::lazy_static;
use rug::integer::IsPrime;
use rug::Integer;
use std::collections::HashSet;
// RSA modulus from https://en.wikipedia.org/wiki/RSA_numbers#RSA-2048
// TODO generate a new modulus
lazy_static! {
    pub static ref MODULUS: Integer = Integer::parse(
        "2519590847565789349402718324004839857142928212620403202777713783604366202070\
           7595556264018525880784406918290641249515082189298559149176184502808489120072\
           8449926873928072877767359714183472702618963750149718246911650776133798590957\
           0009733045974880842840179742910064245869181719511874612151517265463228221686\
           9987549182422433637259085141865462043576798423387184774447920739934236584823\
           8242811981638150106748104516603773060562016196762561338441436038339044149526\
           3443219011465754445417842402092461651572335077870774981712577246796292638635\
           6373289912154831438167899885040445364023527381951378636564391212010397122822\
           120720357"
    )
    .unwrap()
    .into();
    static ref GENERATOR: Integer = Integer::from(65537);
}

#[derive(Clone, Debug)]
pub struct RsaAccumulatorDigest {
    value: Integer,
}
impl Default for RsaAccumulatorDigest {
    fn default() -> Self {
        RsaAccumulatorDigest {
            value: GENERATOR.clone(),
        }
    }
}
impl From<Integer> for RsaAccumulatorDigest {
    fn from(value: Integer) -> Self {
        RsaAccumulatorDigest { value }
    }
}
impl RsaAccumulatorDigest {
    pub fn verify(&self, member: &Integer, witness: Integer) -> bool {
        witness
            .pow_mod(member, &MODULUS)
            .expect("Non negative member")
            == self.value
    }

    #[allow(non_snake_case)]
    pub fn verify_nonmember(&self, value: &Integer, witness: (Integer, Integer)) -> bool {
        //TODO clean up clones
        let (a, B) = witness;
        let temp1 = self
            .value
            .clone()
            .pow_mod(&a, &MODULUS)
            .expect("Non negative witness");
        let temp2 = B.pow_mod(value, &MODULUS).expect("Non negative value");
        return (temp1 * temp2) % MODULUS.clone() == GENERATOR.clone();
    }
}

#[derive(Default, Debug, Clone)]
pub struct RsaAccumulator {
    digest: RsaAccumulatorDigest,
    set: HashSet<Integer>,
}
impl RsaAccumulator {
    pub fn digest(&self) -> &RsaAccumulatorDigest {
        &self.digest
    }

    pub fn add(&mut self, member: Integer) {
        assert!(member >= 0);
        let is_prime = member.is_probably_prime(30);
        if is_prime == IsPrime::No {
            panic!("member must be prime");
        }
        self.digest
            .value
            .pow_mod_mut(&member, &MODULUS)
            .expect("member should be >=0");
        self.set.insert(member);
    }

    pub fn new(members: Vec<Integer>) -> Self {
        let mut acc = Self::default();
        for m in members.into_iter() {
            acc.add(m);
        }
        return acc;
    }

    //TODO compute all proofs?
    pub fn prove(&self, member: &Integer) -> Option<Integer> {
        assert!(member >= &0);
        if !self.set.contains(member) {
            return None;
        }
        let mut current = GENERATOR.clone();
        for s in &self.set {
            if s != member {
                current.pow_mod_mut(s, &MODULUS).expect("member > 0");
            }
        }
        Some(current)
    }

    pub fn prove_nonmember(&self, value: Integer) -> Option<(Integer, Integer)> {
        if self.set.contains(&value) {
            return None;
        }

        let mut exp = Integer::from(1);
        //TODO not this
        for s in &self.set {
            exp *= s;
        }

        let (g, s, t) = exp.gcd_cofactors(value, Integer::new());

        if g != 1 {
            return None;
        }

        let x = GENERATOR.clone();

        return Some((s, x.pow_mod(&t, &MODULUS).unwrap()));
    }
}

#[cfg(test)]
use proptest::prelude::*;

#[cfg(test)]
impl Arbitrary for RsaAccumulator {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;
    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        return Just(RsaAccumulator::default()).boxed();
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn primes() -> impl Strategy<Value = Integer> {
        return Just(5.into());
    }

    proptest! {
        #[test]
        fn test_rsa_accumulator_inner(mut acc: RsaAccumulator, value in primes()) {
            assert_eq!(acc.prove(&value), None);

            acc.add(value.clone());

            let witness = acc.prove(&value).unwrap();
            assert_eq!(acc.digest.verify(&value, witness), true);
        }
    }

    #[test]
    fn test_rsa_accumulator() {
        let acc = RsaAccumulator::default();
        assert_eq!(acc.digest.value, GENERATOR.clone());
    }

    #[test]
    #[should_panic]
    fn test_rsa_accumulator_add() {
        let mut acc = RsaAccumulator::default();

        acc.add(4.into());
    }

    #[test]
    fn test_rsa_accumulator_nonmember() {
        let mut acc = RsaAccumulator::default();
        let witness = acc.prove_nonmember(5.into()).unwrap();
        assert_eq!(acc.digest.verify_nonmember(&5.into(), witness), true);

        acc.add(5.into());
        assert_eq!(acc.prove_nonmember(5.into()), None);
    }
}