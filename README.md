# exercise

A lightweight fault injection tool for quickly checking
`nats-server` binaries for various invariants:

* if a JS consumer reaches the end of their stream,
  what they have received should always be an ordered
  superset of the messages that have been written
  by publishers, and read by other consumers, unless
  purges and deletions have impacted the message.
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

Works by sending servers streams of SIGSTOP/SIGCONT signals
that pause and resume their processes in funky orders, while
paying attention to high-level client progress metrics to ensure
that the cluster does not fail to recover after a deadline, and
to throttle the pauses slowly enough for some progress to happen.
