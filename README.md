# LNX-runner

- [code-fiddle-client](#lnx-runner)
  - [Architecture](#architecture)
  - [Development](#development)

Runner is a microservice handling code execution requests

## Architecture

The idea is to make the service both safe and easy to tune to various needs,
so I'd like to dispatch user code into docker containers and run it in a
safe and consistent environment. Also, I want to keep it as simple as possible.

## Development

The project is in very (very) early phase. These are milestones for version 0.1:

- [X] reading JSON input from front-end
- [X] running user code in podman container
