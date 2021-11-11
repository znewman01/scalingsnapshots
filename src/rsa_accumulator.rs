use lazy_static::lazy_static;
use rug::Integer;
use std::collections::HashSet;
// RSA modulus from https://en.wikipedia.org/wiki/RSA_numbers#RSA-2048
// TODO generate a new modulus
lazy_static! {
    static ref MODULUS: Integer = Integer::parse("2519590847565789349402718324004839857142928212620403202777713783604366202070\
           7595556264018525880784406918290641249515082189298559149176184502808489120072\
           8449926873928072877767359714183472702618963750149718246911650776133798590957\
           0009733045974880842840179742910064245869181719511874612151517265463228221686\
           9987549182422433637259085141865462043576798423387184774447920739934236584823\
           8242811981638150106748104516603773060562016196762561338441436038339044149526\
           3443219011465754445417842402092461651572335077870774981712577246796292638635\
           6373289912154831438167899885040445364023527381951378636564391212010397122822\
           120720357").unwrap().into();
    static ref GENERATOR: Integer = Integer::from(65537);
}

struct RsaAccumulatorDigest {
    value: Integer
}
impl Default for RsaAccumulatorDigest {
    fn default() -> Self {
        RsaAccumulatorDigest { value: GENERATOR.clone() }
    }
}
impl From<Integer> for RsaAccumulatorDigest {
    fn from(value: Integer) -> Self {
        RsaAccumulatorDigest { value }
    }
}
impl RsaAccumulatorDigest {
    fn verify(&self, member: &Integer, witness: Integer) -> bool {
        witness.pow_mod(member, &MODULUS).expect("Non negative member") == self.value
    }
}

#[derive(Default)]
struct RsaAccumulator {
    digest: RsaAccumulatorDigest,
    set: HashSet<Integer>
}
impl RsaAccumulator {
    fn digest(&self) -> &RsaAccumulatorDigest {
        &self.digest
    }

    fn add(&mut self, member: Integer) {
        self.digest.value.pow_mod_mut(&member, &MODULUS);
        self.set.insert(member);
    }

    fn proove(&self, member: &Integer) -> Option<Integer> {
        if !self.set.contains(member) {
            return None;
        }
        let mut current = GENERATOR.clone();
        for s in &self.set {
            if s != member {
                current.pow_mod_mut(s, &MODULUS);
            }
        }
        Some(current)
    }
}


#[test]
fn test_rsa_accumulator() {
    let mut acc = RsaAccumulator::default();
    assert_eq!(acc.digest.value, GENERATOR.clone());

    assert_eq!(acc.proove(&5.into()), None);

    acc.add(5.into());

    let witness = acc.proove(&5.into()).unwrap();
    assert_eq!(acc.digest.verify(&5.into(), witness), true);


}
