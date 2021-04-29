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

* Messages sent from each publisher initially form an ordered set, with the global (across all publishers) ordering being unknown.
* Durability for each message is unknown until one of several things happens:
  * if the publish is acked, it is marked as durable
  * if the publish results in a non-timeout error, it is marked as non-present
  * if a consumer reads a message, it is marked as durable and its total ordering for the stream is established
  * if a consumer reads a later message without this one, the expected early message is marked non-present
* Once a message's durability has been ascertained, it must continue to be read in the known durable order by other consumers
* Successfully Purging, deleting, and discarding messages due to surpassing configured limits causes durable messages to permanently be set to non-present
* If a purge or deletion occurs and it is not successful due to a timeout, the related message is moved back to the uncertain state

## invariants

* Durable messages must always be read by consumers in the same order (they form a total order)
* Non-present messages must never be read by consumers
* Replica changes should have zero impact on observed
  consumer orderings, durability, etc...
* Over time, other invariants related to replica assignment,
  (super)cluster liveness, stream creation/deletion, and
  consumer creation/deletion will be added. Many bugs in
  sharded linearizable systems happen in the metadata
  management that configures the linearizable log parts,
  so we will want to exercise these as well once we are
  able to get this tool to pass most of the time.

## fault injection strategy

Works by sending servers streams of SIGKILL/SIGSTOP/SIGCONT signals
that kill/pause/resume their processes in funky orders, while
paying attention to high-level client invariants and progress metrics
to ensure that the cluster does not fail to recover after a deadline,
and to throttle the pauses slowly enough for some progress to happen.
