# Shares and Remote Libraries

> **Status:** Design, pre-implementation
> **Captured:** 2026-08-19
> **Companions:** `docs/core/design/mounts.md` (how a share is consumed as a drive), `docs/core/design/spacebot-remote-execution.md` (the subtree permission model this extends), `docs/core/design/file-system-intelligence.md` (Access Intelligence)

## The feature

Two operations Spacedrive cannot express today.

**Share a subtree with someone or something else.** Right-click a folder, choose who gets it, and they have it: another person, another device, or an agent running somewhere else. An agent that receives a share sees a new path it is allowed to read. A person who receives one sees a folder in their own library.

**Visit a library that is not yours.** Connect to a Spacedrive running on another machine, with permission, and operate it. Open its Console, browse its files, watch its jobs, then switch back to your own library in the next breath.

Both exist to serve one workflow that is now common and badly served: an agent runs on a machine in the cloud with its own stack of applications, and its operator is somewhere else.

## Why these are separate primitives

They look similar and behave differently, and conflating them would produce a permission model nobody can reason about.

| | Share | Remote library |
| --- | --- | --- |
| Unit | A subtree | A whole library |
| Effect | Content appears inside the recipient's library | The operator's view switches to another library |
| Identity | The recipient stays themselves | The operator is a guest in someone else's world |
| Lifetime | Until revoked | For the session |
| Answer to | "Can my agent read my screenshots?" | "Can I get into that machine?" |

A share brings content to you. A remote library takes you to the content.

## What a share is

A grant, stored as a durable record: a subtree, a recipient, a capability set, and a lifetime.

```
share:
  subtree:     source + subtree root
  recipient:   device | person | agent
  capabilities: read | write | reshare
  lifetime:    until revoked | until <date>
```

The grant is the whole primitive. It says nothing about transport, and it does not move bytes. What consumes a share is separate and already designed:

- **Browsing** it reads the shared snapshot, which is how a share stays fast and works while the owner is offline.
- **Mounting** it presents the subtree as a local volume, per the mounts design, with the byte plane fetching ranges on demand.
- **An agent reading it** gains a new allowed root in its subtree policy, which is the mechanism `spacebot-remote-execution.md` already describes for path permissions.

This is why the share is worth building as its own thing. One grant serves a person browsing, a machine mounting, and an agent reading, without three permission models.

## Recipients

The recipient types differ in how they are addressed, not in what a share means.

- **A device in your library.** Already addressable through the device graph.
- **An agent.** Addressed by its package identity, which is what makes "share this folder with the agent on that cloud machine" expressible. The agent's harness learns it has a new readable root. What it does with that is the harness's business, and Spacedrive's contribution is that the boundary is real and revocable.
- **A person.** Someone else's Spacedrive, identified by their device or account. This is the shared-album case: a folder of photos that several people can see, and with write capability, add to.

- **Anyone with the link.** A public share, described below. The recipient is the link itself.

Human-to-agent and human-to-human sharing are the same operation. Building them as one primitive is what keeps the feature small.

## Public links

The one recipient type that is not another Spacedrive. A public share renders in a browser, which makes it the only consumption path that needs a served page rather than a client.

Two ways to serve the same share, and the grant is identical in both:

1. **Your own domain.** The daemon already runs an Axum server and already serves sidecar bytes over HTTP. A public share is a route on it: point a domain at the machine and links live at `files.example.com/s/<id>`. Nothing of Spacedrive's is in the path, which is the configuration to recommend to anyone who wants the whole feature on their own terms.
2. **The relay.** `sd.app/share/<id>`, served by the paid relay for machines that are not publicly reachable. This is the same relay that carries shares over the open internet, doing the same job for a different client.

The honest tradeoff between them is reachability, and it should be stated rather than hidden. A self-hosted link requires a domain, a certificate, and a machine that answers on the internet. The relay requires none of that and costs money. Both are the same grant, and moving a link from one to the other must not invalidate it.

**The link is the credential.** That has consequences the design must carry rather than discover:

- Identifiers are unguessable, and they are not derived from the share id, the path, or anything else an observer could enumerate.
- Expiry is available, and a default expiry is worth considering for the common case, since a link that lives forever is a link nobody remembers making.
- An optional password gates the page, for the case where the link will travel through somewhere the sender does not trust.
- Access is visible. The owner sees each link, when it was last opened, and how many times. This is the same audit view as every other grant, and it is what makes an unauthenticated URL a defensible thing to hand out.
- Revocation kills the page immediately, including for anyone holding it open.

**What the page is.** A file listing for a subtree, or a preview for a single file, rendered from the same sidecars the Explorer uses. Thumbnails, video proxies, and transcripts already exist as derived artifacts, so a shared folder of photos can present as a gallery without generating anything new. Download is a byte-range read through the same byte plane as a mount.

**Availability follows the source.** A public link serves from the machine that owns the content, so the link is live while that machine is reachable and dead while it is not. This is the correct default and a poor surprise. The relay can hold pinned bytes for a share that must survive the source going offline, which is a storage service rather than a link service, and it is the point where a public link starts costing money for a reason a person can understand.

## Remote libraries

Today a device joins a library and sees everything in it. That is the right model for machines you own and the wrong model for a machine you operate.

A remote library connection is a session against another Spacedrive, authorized by that machine, that does not merge libraries. The visiting client gets a view scoped by what the host grants: its files, its jobs, its Console, its running processes. Nothing about the guest's own library changes, and no device pairing occurs.

Two consequences worth stating plainly. The host decides what a guest may do, because it is the host's machine. And the guest's own library remains the default context, so switching back is a switch rather than a disconnect.

This is the primitive behind operating a cloud machine without adopting it: the agent's environment stays a separate world with its own Pod, its own applications, and its own library, and the operator visits it.

## Transport

Three, in the order they should be built. The share and the remote library are defined above the transport and do not change when one is added.

1. **Local network.** Machines on the same LAN. Already the easiest case.
2. **Tailnet.** If both machines are on the same tailnet, everything works with no further infrastructure. The tailnet is already a host capability in the machine-scope plan, and a share between two machines on it needs no relay, no account, and no Spacedrive service in the path. This is the configuration the docs should recommend for anyone who wants it entirely on their own terms.
3. **Cloud relay.** Over the open internet, a relay operated as a paid service. This is a legitimate thing to charge for, and it does not compromise the custody guarantee: the relay carries bytes between two machines the operator owns or is authorized to reach, the local and tailnet paths remain fully functional without it, and nothing is held hostage when someone stops paying.

**Iroh is deferred.** The transport exists in-tree and the mounts design already plans a byte-range ALPN on it. Building shares on it now adds hole-punching, relay fallback, and connection-state debugging to a feature whose semantics are not settled. Shares should prove themselves over LAN and tailnet first, where the transport question is answered by someone else, and adopt Iroh when the grant model has stopped moving.

This also keeps the feature clear of the sync freeze. A share is not replication. Nothing is reconciled, nothing merges, and no conflict resolution is required, because a share reads from one authority.

## Permissions

A share grants access to a subtree. Every existing permission rule still applies at the point of use: the remote-execution policy for agents, and Access Intelligence's universal permission layer for everything else.

Rules that keep it honest:

- **The owner can always see what is shared.** A single view of every outstanding grant, who holds it, and when it was last used. A permission model nobody can audit is a permission model nobody should trust.
- **Revocation is immediate and total.** The grant record is the authority. Cached blocks on the recipient's side are invalidated, and a mount backed by a revoked share fails legibly.
- **Resharing is a capability, not an assumption.** A recipient cannot pass a share on unless the grant says so.
- **Write is opt-in and separate.** Read is the default and the common case. Write shares are the same record with a different capability, and they inherit the transfer machinery rather than inventing a sync path.

## Ops surface

```
shares.create      grant a subtree to a recipient
shares.list        outstanding grants, both directions, with access records
shares.revoke      end a grant now
shares.link        mint a public link for a share, self-hosted or relayed
libraries.connect  open a session against a remote library
libraries.visiting the current remote session, if any
```

`shares.list` returning both directions matters: what I have shared, and what has been shared with me, are the same question asked from two ends.

## Ordered phases

1. **The grant record and `shares.*`.** Subtree, recipient, capabilities, lifetime, and the audit view. No transport work.
2. **Read shares between devices in one library, over LAN and tailnet.** Browsing served by the snapshot, bytes by the mount byte plane.
3. **Agents as recipients.** The allowed-root wiring into the harness, which is the workflow that motivates the feature.
4. **Self-hosted public links.** A route on the daemon's existing server, the rendered page, and the link credential rules. This is the first recipient outside Spacedrive and the first one a stranger touches, so the audit view and revocation are load-bearing here rather than optional.
5. **Remote library sessions.** Authorization, scoped views, and context switching in the interface.
6. **People as recipients.** Identity between two libraries that are not the same library.
7. **The relay.** `sd.app/share/<id>`, and shares over the open internet, once the local, tailnet, and self-hosted paths are proven.

Phases 1 through 4 require no relay, no account, and no new transport, and they cover both the agent workflow and public links.

## Open questions

- How a person is identified across two libraries, and whether that needs an account or can be device-to-device.
- Whether a share appears in the recipient's library as a location, a source, or a distinct shared-with-me class. The last is likely, since the recipient does not own it and must not treat it as durable.
- What a share of an agent package means, given that a package holds secrets by declaration.
- Whether remote library sessions and Console's existing remote host management converge, since both authorize an operator against a machine.
- Group shares. The photo-album case wants a recipient set rather than repeated grants, and that is a small model change made early or a painful one made late.
