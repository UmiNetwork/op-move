#!/bin/sh

set -eux

# Create volumes because swarm cannot do it by itself
mkdir -p docker/op-move/volume docker/op-move/volume/db1 docker/op-move/volume/db2 docker/shared/volume docker/shared/volume/1 docker/shared/volume/2

# Initialize local swarm
[ "$(docker info --format '{{.Swarm.LocalNodeState}}')" = "active" ] || docker swarm init

# Create shared network for services deployed to the swarm
docker network inspect localnet -f "Network exists" || docker network create localnet --scope swarm --driver overlay

# Pull and build images
docker compose build --pull

# Deploy the stack
docker stack deploy --resolve-image never -c docker-compose.yml -d umi
