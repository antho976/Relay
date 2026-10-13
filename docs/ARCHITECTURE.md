# Native client boundary

`relay serve` owns SQLite, the operation pipeline, role authorization, worktrees,
provider processes, PTYs and retained scrollback. The native app uses the Unix
socket as actor `user`, never as an agent or `system`.

The native source is split by responsibility:

| File | Responsibility |
| --- | --- |
| `client.rs` | Request IDs, pending replies, bounded writer/notice queues, disconnects and framing limits |
| `terminal.rs` | VTE input/output, epoch/sequence tracking, gap recovery, attachment lifecycle and resize coalescing |
| `app.rs` | Shell, project selection, session reconciliation and paired launch |
| `pages.rs` | Tasks, messages, holds and notes through existing bus operations |
| `editor.rs` | GtkSourceView, bounded reads, dirty-buffer protection and save conflict checks |
| `smoke.rs` | Opt-in screenshot verification, excluded from normal runtime behavior |

## Performance

One persistent control socket and one socket per attached terminal. A terminal
that applies backpressure cannot fill the control socket's queue. Channels are
bounded to 64 items; wire lines are bounded to 2 MiB. The GTK drain yields after
64 KiB of output. Frame sizes can exceed that budget for one engine catch-up
frame. All socket I/O and request deadlines run on a two-thread Tokio runtime.

No repeated refresh clock or filesystem scan. Lifecycle events coalesce while
a refresh is in flight. A switch away from the wall or to another project
closes the unnecessary attachments. VTE widgets remain intact while changing
the wall layout. Resize is scheduled once per GLib idle turn after a real
layout/character-size event, and unchanged dimensions are not sent.

Pending replies resolve on EOF or timeout. A timeout is an unknown mutation
outcome, so the client does not automatically replay it. Disconnect recovery
is explicit. Closing the GUI releases subscriptions without closing sessions.

## Coordination and safety

Paired launch creates both identities, stages the task assignment, then starts
the reviewer and builder. The engine enforces roles and shared-worktree rules.
Partial launch failures preserve allocated sessions for inspection. No cleanup
path silently deletes a worktree, branch or repository: the one that deletes
unprompted, the merged-branch sweep (`git.branch.cleanup`, after a close and
every 20 minutes), takes only closed sessions' `relay/*` branches whose work is
provably merged, and writes an audit row for each.

The native hold screen uses the new user-only `guardrail.hold.get` to inspect
the exact frozen request with its authentication token removed. Only after
inspection can the user select Allow once. Confirmation still goes through the
engine, where other policies continue to apply. Task approval remains a
separate user-only action. The mailbox never acknowledges mail on behalf of
an agent.

The editor reads at most 1 MiB and refuses binary/truncated content for editing.
Unsaved changes block project switches, file switches and window closure. Save
checks the current disk contents before writing through `file.write`. This is
optimistic conflict detection, not an atomic filesystem compare-and-swap; an
external writer can still race the two bus operations. Full worktree selection
and richer conflict handling remain in the Code roadmap.

## Remote door

`relay remote serve` (`crates/relay-remote`) fronts the same socket door for a paired
phone over a WebSocket: directly on the LAN, or through a rendezvous server the person
hosts (`relay remote rendezvous`) that the engine dials out to. Every line a phone sends is
gated, one line at a time even when a frame carries several: the actor must be `user`, and
the op must be in `wire::PHONE_OPS` (the ops the app calls, plus `bus.unsubscribe`); anything
else is answered by the door (`bus.actor` for a wrong actor, `refused` / `remote.op` for an
op outside the list). A line that passes is written to the Unix socket unchanged; every line
back is forwarded as-is, `pty` frames included. The door adds pairing and a per-connection
proof of the device token; it adds no ops. The PC dials the rendezvous with the room secret
in an `Authorization: Bearer` header, retrying once with `?secret=` only when an older server
refuses it in its own words. See
`docs/MOBILE.md`.
