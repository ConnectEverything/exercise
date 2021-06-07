use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::io;
use std::mem;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicU64, Ordering::SeqCst};

use rand::seq::{IteratorRandom, SliceRandom};
use rand::{rngs::StdRng, Rng, SeedableRng};

use nats::jetstream::{ConsumerConfig, RetentionPolicy, StreamConfig};

const STREAM: &str = "exercise_stream";

// generates unique (for this test run) ID
fn idgen() -> u64 {
    static IDGEN: AtomicU64 = AtomicU64::new(0);
    IDGEN.fetch_add(1, SeqCst)
}

pub struct Cluster {
    clients: Vec<Consumer>,
    servers: Vec<Server>,
    paused: HashSet<usize>,
    args: Args,
    rng: StdRng,
    unvalidated_consumers: HashSet<usize>,
    durability_model: DurabilityModel,
}

impl Cluster {
    pub fn start(args: Args) -> Cluster {
        println!("Starting cluster exerciser with seed {}", args.seed);

        let rng = SeedableRng::seed_from_u64(args.seed);

        let servers: Vec<Server> = (0..args.servers)
            .map(|i| server(&args.path, i as u16))
            .collect();

        // let servers come up
        std::thread::sleep(std::time::Duration::from_millis(2000));

        println!("creating testing stream {}", STREAM);

        {
            let nc = servers[0].nc();

            let _ = nc.delete_stream(STREAM);

            nc.create_stream(StreamConfig {
                num_replicas: args.num_replicas,
                name: STREAM.to_string(),
                retention: RetentionPolicy::Limits,
                ..Default::default()
            })
            .expect("couldn't create exercise_stream");
        }

        let clients: Vec<Consumer> = servers
            .iter()
            .cycle()
            .enumerate()
            .take(args.clients as usize)
            .map(|(id, s)| {
                let consumer_name = format!("consumer_{}", id);
                println!("creating testing consumer {}", consumer_name);

                let nc = s.nc();
                let conf = ConsumerConfig {
                    deliver_subject: Some(consumer_name.clone()),
                    durable_name: consumer_name.into(),
                    ..Default::default()
                };
                Consumer {
                    inner: nc
                        .create_consumer(STREAM, conf)
                        .expect("couldn't create consumer"),
                    observed: Default::default(),
                    id,
                }
            })
            .collect();

        Cluster {
            servers,
            clients,
            rng: rng,
            args,
            paused: Default::default(),
            durability_model: Default::default(),
            unvalidated_consumers: Default::default(),
        }
    }

    pub fn step(&mut self) {
        match self.rng.gen_range(0..1000) {
            0..=5 => self.restart_server(),
            6..=40 => self.pause_server(),
            41..=90 => self.resume_server(),
            91..=200 => self.publish(),
            201..=1000 => self.consume(),
            _ => unreachable!("impossible choice"),
        }
        self.validate();
    }

    fn restart_server(&mut self) {
        if self.args.no_kill {
            return;
        }
        let idx = self.rng.gen_range(0..self.servers.len());
        println!("restarting server {}", idx);

        self.servers[idx].restart();
        self.paused.remove(&idx);
    }

    fn pause_server(&mut self) {
        if self.paused.len() == self.servers.len() {
            // all servers already paused
            return;
        }
        let mut idx = self.rng.gen_range(0..self.servers.len());

        while self.paused.contains(&idx) {
            idx = self.rng.gen_range(0..self.servers.len());
        }

        println!("pausing server {}", idx);

        let pid = self.servers[idx].child.as_ref().unwrap().id();

        unsafe {
            if libc::kill(pid as libc::pid_t, libc::SIGSTOP) != 0 {
                panic!("{:?}", io::Error::last_os_error());
            }
        }

        self.paused.insert(idx);
    }

    fn resume_server(&mut self) {
        if self.paused.is_empty() {
            // nothing ot resume
            return;
        }

        let idx = *self.paused.iter().choose(&mut self.rng).unwrap();

        println!("resuming server {}", idx);

        let pid = self.servers[idx].child.as_ref().unwrap().id();

        unsafe {
            if libc::kill(pid as libc::pid_t, libc::SIGCONT) != 0 {
                panic!("{:?}", io::Error::last_os_error());
            }
        }

        self.paused.remove(&idx);
    }

    fn publish(&mut self) {
        let c = self.clients.choose(&mut self.rng).unwrap();
        let data = idgen().to_le_bytes();
        c.inner.nc.publish(STREAM, data).unwrap();
    }

    fn consume(&mut self) {
        let c = self.clients.choose_mut(&mut self.rng).unwrap();
        let proc_ret: io::Result<(u64, u64)> = c.inner.process_timeout(|msg| {
            let info = msg.jetstream_message_info().unwrap();

            let id = u64::from_le_bytes((&*msg.data).try_into().unwrap());
            Ok((info.stream_seq, id))
        });

        if let Ok((seq, id)) = proc_ret {
            c.observed.insert(seq, id);
            self.unvalidated_consumers.insert(c.id);
        }
    }

    fn validate(&mut self) {
        // assert all consumers have witnessed messages in the correct order
        let unvalidated_consumers = mem::take(&mut self.unvalidated_consumers);

        for id in unvalidated_consumers {
            let c = &mut self.clients[id];

            let observed = mem::take(&mut c.observed);

            for (id, value) in observed {
                if let Some(old_value) = self.durability_model.observed.insert(id, value) {
                    if value != old_value {
                        eprintln!(
                            "
                            Correctness violation detected after running for {:?}.
                            Consumers received different values for the same \
                            stream sequence.
                                stream sequence: {}
                                first observed value: {}
                                second observed value: {}
                                schedule replay seed: {}
                            ",
                            self.args.start_time.elapsed(),
                            id,
                            old_value,
                            value,
                            self.args.seed
                        );
                        std::process::exit(1);
                    }
                }
            }
        }
    }
}

struct Server {
    child: Option<Child>,
    port: u16,
    storage_dir: String,
    path: PathBuf,
    idx: u16,
}

impl Server {
    fn nc(&self) -> nats::Connection {
        nats::connect(&format!("localhost:{}", self.port)).unwrap()
    }

    fn restart(&mut self) {
        let mut child = self.child.take().unwrap();
        child.kill().unwrap();
        child.wait().unwrap();

        *self = server(&self.path, self.idx);
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            child.kill().unwrap();
            child.wait().unwrap();
        }
        let _ = std::fs::remove_dir_all(&self.storage_dir);
    }
}

/// Starts a local NATS server that gets killed on drop.
fn server<P: AsRef<Path>>(path: P, idx: u16) -> Server {
    let port = idx + 44000;
    let storage_dir = format!("jetstream_test_{}", idx);
    let _ = std::fs::remove_dir_all(&storage_dir);

    let supercluster_conf = format!("confs/supercluster_{}.conf", idx);

    let child = Command::new(path.as_ref())
        .args(&["--port", &port.to_string()])
        .arg("-js")
        .args(&["-sd", &storage_dir])
        .args(&["-c", &supercluster_conf])
        .arg("-V")
        .arg("-D")
        .spawn()
        .expect("unable to spawn nats-server");

    Server {
        child: Some(child),
        port,
        storage_dir,
        path: path.as_ref().into(),
        idx,
    }
}

struct Consumer {
    inner: nats::jetstream::Consumer,
    observed: HashMap<u64, u64>,
    id: usize,
}

// we record every sid:uuid pair, and
// ensure that consumers never observe
// different uuid's for the same stream id.
#[derive(Default, Debug)]
struct DurabilityModel {
    observed: HashMap<u64, u64>,
}

const USAGE: &str = "
Usage: exercise [--path=</path/to/nats-server>]

Options:
    --path=<p>      Path to nats-server binary [default: nats-server].
    --seed=<#>      Seed for replaying faults [default: None].
    --clients=<#>   Number of concurrent clients [default: 3].
    --servers=<#>   Number of cluster servers [default: 3].
    --steps=<#>     Number of steps to take [default: 10000].
    --replicas=<#>  Number of replicas for the JetStream test stream [default: 1].
    --no-kill       Do not restart servers, just pause/resume them [default: unset].
    --burn-in       Ignore steps and run tests until we crash [default: unset].
";

#[derive(Debug)]
pub struct Args {
    path: PathBuf,
    seed: u64,
    clients: u8,
    servers: u8,
    pub steps: u64,
    num_replicas: usize,
    no_kill: bool,
    pub burn_in: bool,
    start_time: std::time::Instant,
}

impl Default for Args {
    fn default() -> Args {
        Args {
            path: "nats-server".into(),
            seed: rand::thread_rng().gen(),
            clients: 3,
            servers: 3,
            steps: 10000,
            num_replicas: 1,
            no_kill: false,
            burn_in: false,
            start_time: std::time::Instant::now(),
        }
    }
}

fn parse<'a, I, T>(mut iter: I) -> T
where
    I: Iterator<Item = &'a str>,
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Debug,
{
    iter.next().expect(USAGE).parse().expect(USAGE)
}

impl Args {
    pub fn parse() -> Args {
        let mut args = Args::default();
        for raw_arg in std::env::args().skip(1) {
            let mut splits = raw_arg[2..].split('=');
            match splits.next().unwrap() {
                "path" => args.path = parse(&mut splits),
                "seed" => args.seed = parse(&mut splits),
                "clients" => args.clients = parse(&mut splits),
                "servers" => args.servers = parse(&mut splits),
                "steps" => args.steps = parse(&mut splits),
                "replicas" => args.num_replicas = parse(&mut splits),
                "no-kill" => args.no_kill = true,
                "burn-in" => args.burn_in = true,
                other => panic!("unknown option: {}, {}", other, USAGE),
            }
        }
        args
    }
}
