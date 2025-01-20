# swayvnc adapted for App Controller

This is based on swayvnc by Björn Busse. His original work can be found
[here](https://github.com/bbusse/swayvnc). Any original code is subject to the
LICENSE file in this directory.

This version is adapted for use with App Controller. noVNC integration has been
added, and unecessary features have been removed to simplify the build.

Notable changes:
- Install websockify and neatvnc from alipine repositories
- Removed alpine version selection
- Removed SSL key gen and authentication
- Removed entrypoint script
- Removed socat sway IPC redirection
