#!/bin/bash

# Colors for printing
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

# Pull runtime image
PULL_CMD="podman pull ghcr.io/group-project-gut/lynx-runtime:$1"
PULL_OUT=$($PULL_CMD)
if [ $? == 0 ]; then
    echo -e "${GREEN} [OK]\t ${NC} Succesfully pulled runtime:$1 image from ghcr"
else
    echo -e "${RED} [ERROR]\t\t ${NC} Failed to pull runtime:$1 image from ghcr"
    echo $PULL_OUT
    exit 1
fi

# Run runner exec
echo -e "${GREEN} [OK]\t ${NC} Running runnner:0.4 at port $2"
./target/release/lynx-runner $2