# quota-cli

Read your AI usage from the credentials your agent CLIs already stored.

`quota-cli` prints how much quota you have left across Claude, Codex, Cursor, Antigravity, Grok, and Kiro. It reads each provider's token from wherever that provider's own CLI keeps it, so there is no sign-in step, no config file, and no account to connect. If you are logged into Claude Code, you get Claude numbers. If you are not, it says so and moves on.

It is part of [Quota](https://github.com/pinkpixel-dev/quota), a desktop app that does the same thing with a UI. The CLI is standalone and does not need the app installed or running.

## Install

```bash
cargo install quota-cli
```

Kiro's credentials live in a SQLite database, which is compiled from bundled C source, so you need a working C compiler for the build.

## Usage

```bash
quota-cli usage
```

```text
claude       5h 98% (resets in 4h 58m) · Wk 35% (resets in 3d 9h)
codex        5h 100% (resets in 4h 59m) · Wk 95% (resets in 4d 6h)
cursor       Plan 100% (resets in 8d 3h)
antigravity  5h 100% (resets in 4h 59m) · Wk 97% (resets in 6d 17h)
grok         no credit allocation
kiro         Credits 100% · Bonus 100%
```

Percentages are what you have left, not what you have used. Reset times show when a provider reports one, and Kiro currently does not.

For scripting, `--json` prints the same data as an array:

```bash
quota-cli usage --json
```

Providers you are not signed into are not records, so their messages go to stderr instead of into the payload. The command exits non-zero only when no provider at all could report, since one signed-out provider on an otherwise working machine is not a failure.

## Herdr plugin

```bash
quota-cli herdr report [--force]
```

This pushes usage into the [Herdr](https://herdr.dev) sidebar as a metadata token, so each pane shows the numbers for the agent it is actually running. It is meant to be driven by the plugin in [`herdr-plugin/`](https://github.com/pinkpixel-dev/quota/tree/main/herdr-plugin) rather than run by hand. Only providers with a pane on screen are fetched, and without `--force` each one serves a cached value until that cache goes stale.

## Where the credentials come from

| Provider | Source |
|---|---|
| Claude | `~/.claude/.credentials.json` |
| Codex | `~/.codex/auth.json`, honoring `CODEX_HOME` |
| Cursor | `~/.config/cursor/auth.json` |
| Antigravity | OS keyring, falling back to `~/.gemini/oauth_creds.json` |
| Grok | `~/.grok/auth.json`, honoring `GROK_HOME` |
| Kiro | `~/.local/share/kiro-cli/data.sqlite3`, falling back to the AWS SSO cache |

Three of these are worth knowing about. Cursor, Kiro, and Antigravity keep separate credentials for their CLI and their IDE, and the two stores can hold different accounts or lapse independently. `quota-cli` reads the CLI store first, because that is the session an agent in a terminal actually uses.

## What it does not do

Every reader is strictly read-only. Nothing is refreshed, rewritten, or rotated, because those tokens back your own agent sessions and rotating one could leave your real CLI holding a dead credential. Antigravity is the single exception, and only because Google does not rotate its refresh token: a fresh access token is held in memory for one request and nothing is written to disk.

The practical effect shows up with Kiro. Its CLI refreshes its token only when it has a reason to call the service, so an idle machine holds a lapsed token and `quota-cli` reports that instead of a number. Using Kiro refreshes it.

Tokens are never logged or printed. Each credential struct has a hand-written `Debug` that redacts, and parse failures report only a line and column, because serde's own error message can quote the input it failed on.

## License

Apache-2.0. Made with 💖 by Pink Pixel.
