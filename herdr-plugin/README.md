# Quota for Herdr

A Herdr plugin that shows your Claude usage right in the sidebar, next to the panes and workspaces where you're actually using it.

## What it shows

The plugin reports a compact usage token, like `5h 89% · Wk 57%`, onto every pane running a Claude agent and onto the workspace that pane belongs to. Herdr's sidebar can render that token wherever you add `$quota` to a row.

A sidebar row with the token added might look like this:

```text
● claude   my-app   main   $quota
```

`5h` is your rolling 5-hour usage window, `Wk` is your weekly window, and the percentages are how much of each you've used. If no pane in your session is running Claude, the plugin does nothing. It never makes a network call in that case.

## Installing quota-cli

The plugin runs a small binary called `quota-cli` to read your usage and hand it to Herdr. There's no published release binary for `quota-cli` yet, so right now you build it from source.

From the root of the [quota](https://github.com/pinkpixel-dev/quota) repository:

```bash
cd quota-cli
cargo build --release
```

That produces `target/release/quota-cli`. Copy it somewhere on your `PATH`, for example:

```bash
mkdir -p ~/.local/bin
cp target/release/quota-cli ~/.local/bin/quota-cli
```

Make sure `~/.local/bin` (or wherever you put it) is actually on your shell's `PATH`. The plugin manifest invokes `quota-cli` by name, so Herdr needs to be able to find it the same way your shell would.

Once a release binary exists, this section will be updated with a direct download step. For now, building from source is the only supported path.

## Installing the plugin

With `quota-cli` on your `PATH`, link this plugin into Herdr:

```bash
herdr plugin install pinkpixel-dev/quota/herdr-plugin
```

Herdr will report back whether the plugin linked cleanly. If it mentions an unrecognized event name, that's worth a second look. This is still early, and the exact hook Herdr uses to notice a Claude pane's status changing hasn't been confirmed against a live plugin install.

## Adding the sidebar rows

The plugin doesn't touch your Herdr configuration. Adding `$quota` to a sidebar row is entirely your call, and you do it by hand in `~/.config/herdr/config.toml`.

For the agents sidebar:

```toml
[ui.sidebar.agents]
rows = [["state_icon", "machine", "workspace", "tab"], ["agent", "$quota"]]
```

For the spaces sidebar:

```toml
[ui.sidebar.spaces]
rows = [["state_icon", "machine", "workspace", "tab"], ["agent", "$quota"]]
```

Feel free to place `$quota` wherever it fits your layout better. These are just starting points, not requirements. After editing the config, reload it with `herdr server reload-config`.

## Optional: a keybinding to refresh on demand

The plugin refreshes automatically when a Claude pane's status changes, and it also re-checks the cache periodically. If you want a manual refresh you can trigger anytime, add a keybinding for the `refresh` action:

```toml
[[keys.command]]
key = "prefix+u"
type = "plugin_action"
command = "pinkpixel.quota.refresh"
description = "refresh Quota usage"
```

Pick whatever key combination makes sense for your setup. `prefix+u` is just an example.

## Reading your credentials

This plugin reads the Claude Code CLI's locally stored credentials to figure out your usage. That read is strictly read-only. It never refreshes, rewrites, or rotates your Claude Code credentials file. Doing that would risk invalidating the refresh token your own Claude Code CLI depends on, so the plugin stays well away from it.

## When the sidebar shows nothing

A blank `$quota` token usually means one of a few things:

- No pane in the current session is running a Claude agent. The plugin only reports usage for Claude panes, so this is expected.
- `quota-cli` isn't on `PATH`, or the plugin can't find it.
- The last fetch failed and there's no cached value yet to fall back to.

Check what actually happened with:

```bash
herdr plugin log list --plugin pinkpixel.quota
```

That shows the plugin's recent invocations, including exit codes and any error output, which is usually enough to tell you what went wrong.
