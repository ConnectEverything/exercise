use quickcheck::{Arbitrary, Gen, QuickCheck, RngCore};
use rand::{rngs::StdRng, Rng};

#[derive(Debug, Clone)]
struct SuperclusterConf {
    clusters: Vec<ClusterConf>,
}

impl Arbitrary for SuperclusterConf {
    fn arbitrary<G: Gen>(g: &mut G) -> SuperclusterConf {
        let n_clusters: usize = g.gen_range(0..10);
        SuperclusterConf {
            clusters: g.gen::<Vec<ClusterConf>>(),
        }
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
        Box::new(
            self.clusters
                .shrink()
                .map(|clusters| SuperclusterConf { clusters }),
        )
    }
}

#[derive(Debug, Clone)]
struct ClusterConf {
    servers: Vec<ServerConf>,
}

impl Arbitrary for ClusterConf {
    fn arbitrary<G: Gen>(g: &mut G) -> ClusterConf {
        todo!()
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
        Box::new(self.servers.shrink().map(|servers| ClusterConf { servers }))
    }
}

#[derive(Debug, Clone)]
struct ServerConf {}

impl Arbitrary for ServerConf {
    fn arbitrary<G: Gen>(g: &mut G) -> ServerConf {
        todo!()
    }
}

fn prop_connectivity(sc: SuperclusterConf) -> bool {
    false
}

#[test]
pub fn qc() {
    QuickCheck::new()
        .quickcheck(prop_connectivity as fn(SuperclusterConf) -> bool);
}
