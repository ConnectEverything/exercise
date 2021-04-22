# exercise

A lightweight black-box fault injection tool for quickly
checking `nats-server` binaries for various invariants
related to (super)cluster liveness and JS durability.

## (super)cluster liveness

Liveness is assessed as it relates to NATS clustering and superclusters.

* If all injected faults have been cleared, the cluster should reach a "functional state" after a maximum time bound.
* "functional state" means that standard NATS messages should flow across leaf, gateway, cluster, etc... boundaries.

## message durability model

Durability is assessed as it relates to JetStream.

* Messages sent from each publisher initially form an ordered set, with the global (across all publishers) ordering being unknown.
* Durability for each message is unknown until one of several things happens:
  * if the publish is acked, it is marked as durable
  * if the publish results in a non-timeout error, it is marked as non-present
  * if a consumer reads a message, it is marked as durable and its total ordering for the stream is established
  * if a consumer reads a later message without this one, the expected early message is marked non-present
* Once a message's durability has been ascertained, it must continue to be read in the known durable order by other consumers
* successfully Purging, deleting, and discarding messages due to surpassing configured limits causes durable messages to permanently be set to non-present
* if a purge or deletion occurs and it is not successful due to a timeout, the related message is moved back to the uncertain state

## message invariants

* Durable messages must always be read by consumers in the same order (they form a total order)
* Non-present messages must never be read by consumers

## stream invariants

* replica changes should have zero impact on observed
  consumer orderings, durability, etc...
* when a stream of a certain name is created and
  deleted sequentially but using separate clients,
  the presence or absence of a stream should always
  match the last action known to complete sequentially.
* when a consumer of a certain name is created and
  deleted sequentially but using separate clients,
  the presence or absence of a stream should always
  match the last action known to complete sequentially.
* when a consumer has processed a subset of messages
  and double-acked their progress to the server, the
  consumer info should reflect this acknowledged progress
  regardless of subsets of servers restarting.
* after a maximum "quiescence" period, consumer and stream
  info must match expectations derived from the message
  durability model.

## fault injection strategy

Works by sending servers streams of SIGKILL/SIGSTOP/SIGCONT signals
that kill/pause/resume their processes in funky orders, while
paying attention to high-level client invariants and progress metrics
to ensure that the cluster does not fail to recover after a deadline,
and to throttle the pauses slowly enough for some progress to happen.
