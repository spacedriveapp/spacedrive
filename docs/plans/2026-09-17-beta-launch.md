# Spacedrive 2.0 Beta: release identity and launch narrative

Status: release direction agreed with James on September 17, 2026. Beta is the
target, not a claim that the current build has passed its release gates. The
existing October 1 target remains subject to those gates.

## The decision

The next public milestone is Spacedrive 2.0 Beta. Keep the 2.0 generation.
The launch post must explain why the transition out of alpha matters after
almost five years: Spacedrive has found its direction and is committing to the
data model that supports it.

The version history reflects the development history. The first architecture
was going in the wrong direction. James chose to rewrite it rather than ship a
1.0 around that direction. The 2.0 alpha period was the work of finding the
right foundation. Beta marks the point where that foundation becomes a
commitment to users.

Do not present 1.0 as a stable release that shipped. Do not frame the beta
announcement around another rewrite or the amount of code changed. Explain
what the settled model lets people rely on and why the team will build on it.

## What the commitment means

Sources hold the catalog and durable assertions. Libraries establish membership
and shared intent. Volumes identify the underlying storage. Policies maintain
processing at selected scopes, and Space items organize navigation. These
boundaries give the product a direction that can support ongoing development.

Committing to this model does not prohibit schema changes. Schema evolution
must preserve supported beta data through tested upgrades and explicit
migrations. Users should not have to start their libraries again because a
later release reconsiders the foundation. Irreplaceable assertions and retained
catalogs remain protected when the original storage is unavailable.

This commitment begins at the beta boundary. It does not reverse the agreed
breaking transition from alpha or introduce location migration requirements.
The release notes must identify supported upgrade paths and explain unsupported
alpha formats without implying that existing source stores can be discarded.

Beta still permits bugs, unfinished features and changes to the interface.
Publish the supported scope and known limitations. Demonstrate reliable setup,
indexing, search, device access, restart, disconnection and recovery for the
workflows claimed in the release. The
[release gates](../core/releases.mdx) remain the evidence required to publish.

## Launch-post draft

This is proposed launch-day wording for review. Use it when the beta gates
have passed:

After almost five years in alpha, Spacedrive is entering beta.

Our version history has been unusual. We never shipped a stable 1.0. I rewrote
Spacedrive because the original architecture was taking the project in the
wrong direction, and I didn't want to commit your data to that foundation.
The 2.0 alpha has been the process of finding the right one.

We have now found it. Spacedrive 2.0 Beta marks our commitment to the data model
we will build on, and to preserving the libraries you build with it as
Spacedrive evolves.

There is still work ahead. Features will grow, the interface will improve, and
bugs will be fixed. The beta gives that work a settled foundation. Your files,
catalogs and the information you add to them are what that foundation exists
to protect.

## Evidence to include in the post

Explain the model through one user's experience: adding content to a library,
choosing where its catalog lives, using it from another device and retaining
useful information when a drive is disconnected. Describe features only when
they are implemented and validated in the release candidate.

Use the NAS run and recovery work as concrete evidence, with the tested build
and limits. Explain what survives an interrupted operation and a restart.
Separate tested behavior from features still planned. End with the supported
platforms, known issues, upgrade instructions and how people can report bugs.
