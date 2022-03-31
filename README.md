# Dither Sim
Simulation program testing various aspects of dither.

## What is it?
Dither is a (WIP) platform for creating decentralized and privacy-respecting applications to replace the current internet. This repository is for testing the routing protocols of dither such as: [Distance-Based Routing](https://www.dither.link/docs/spec/dither/routing/distance-based-routing/), [Directed Trail Search](https://www.dither.link/docs/spec/dither/routing/directional-trail-search/), and others.

Distance-Based Rounting (DBR) a protocol by which packets can be routed efficiently with variable anonymitity across a network of computers, allowing for flexibility between speed and privacy.

Directed Trail Search (DTS) is a protocol to manage the distribution and routing of data across a network in a manner that can fetch data from its closest source (Similar to IPFS or Kademlia, but way faster and more scalable)

## Running
This package requires `cargo`, `cmake`,`pkg-config`, `fontconfig`, and `freetype` to compile.

dither-sim also requires rust nightly.

If you use the Nix Package Manager, you can do `nix develop` to get a build environment with everything needed to do `cargo run --package gui`.
