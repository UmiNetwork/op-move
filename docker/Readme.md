# Docker

## Development

### Directory layout

```
.                            ◄── Root directory of the repository
└── docker/                  ◄── Root directory for docker context
    ├── <service-name>/      ◄── Directory for <service-name> specific files
    │   ├── volume/          ◄── Directory for <service-name> volume shared with host 
    │   ├── Dockerfile       ◄── Dockerfile that builds <service-name> image (optional)
    │   └── entrypoint.sh    ◄── Entrypoint that launches <service-name> container
    ├── host/                ◄── Directory for files intended to run on host that interact with the services
    └── shared/              ◄── Directory for shared kernel of all docker services
        ├── volume/          ◄── Directory for volume shared with all docker services
        └── Dockerfile       ◄── Dockerfile that builds all images that don't have their own dockerfile
```
