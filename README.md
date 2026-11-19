# AllTheOddments

Small things that helped me once and can help again.

[AllTheTools](https://github.com/Stiven-Gjekaj/AllTheTools) holds the tools
that every computer needs. This repository holds the niche things: scripts
for one job, notes, and fixes for one odd problem.

## Layout

Each topic has its own directory. Each script has a comment at the top that
tells what it does and how to use it.

| Path | What it does |
| --- | --- |
| [ssh/new-machine.sh](ssh/new-machine.sh) | Makes a new macOS or Linux computer ready to use SSH with GitHub. |

## ssh/new-machine.sh

Run it on the new computer:

    sh ssh/new-machine.sh

It makes an ed25519 key, adds a GitHub block to `~/.ssh/config`, gives the
key to the agent, and sends the public key to GitHub if `gh` is signed in.
Then it tests the connection. You can run it again. It keeps the key that
exists.

Run the test. The test uses a temporary home directory, not your `~/.ssh`:

    sh ssh/test-new-machine.sh

## Add a new thing

1. Put it in a directory for its topic.
2. Write what it does in a comment at the top.
3. Add a test if the thing is a script.
4. Add one row to the table above.
