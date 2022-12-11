#!/bin/bash

podman pull ghcr.io/group-project-gut/lynx-runtime:0.3

./target/release/lynx-runner 9000