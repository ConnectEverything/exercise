# exercise

A lightweight black-box fault injection tool for quickly
checking `nats-server` binaries for various invariants
related to (super)cluster liveness and JS durability.

(currently needs to be run from this repo's root directory)

```
Usage: exercise [--path=</path/to/nats-server>] [--seed=<#>]
                [--clients=<#>] [--servers=<#>] [--steps=<#>]
Options:
    --path=<p>      Path to nats-server binary [default: nats-server].
    --seed=<#>      Seed for replaying faults [default: None].
    --clients=<#>   Number of concurrent clients [default: 3].
    --servers=<#>   Number of cluster servers [default: 3].
    --steps=<#>     Number of steps to take [default: 10000].
    --replicas=<#>  Number of replicas for the JetStream test stream [default: 1].
    --no-kill       Do not restart servers, just pause/resume them [default: unset].
```

## message durability model

Durability is assessed as it relates to JetStream.

* all published messages are given a unique value which is
  monotonic from the publisher's perspective (but is often
  scrambled up by the time it is serialized into a stream
  and given a unique stream seq number)
* any time a consumer receives a message, it stores the
  value and stream seq in a global map
* if consumers ever receive different unique message values
  for the same stream seq number, exercise will panic

## fault injection strategy

Works by sending servers streams of SIGKILL/SIGSTOP/SIGCONT signals
that kill/pause/resume their processes in funky orders, while
paying attention to high-level client invariants and progress metrics
to ensure that the cluster does not fail to recover after a deadline,
and to throttle the pauses slowly enough for some progress to happen.
