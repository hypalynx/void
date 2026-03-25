# void

CLI agent for LLM interaction. Tap the infinite.

An agentic tool that brings language model capabilities to your terminal, letting you
explore emergent possibilities through a clean command-line interface.

## TODO

- State of this project, it does communication with LLMs and
  basically streams and parses responses, however to progress we
  will need to add test coverage at the finer details of both llm
  communication and rendering to move forward. We currently
  re-render the entire message history where this could be cached
  and there are a few oddities/nice to haves where we can't see
  the latest line until it's complete (so you can't see streaming
  after enough content has been entered into the message buffer)
  and we don't currently scroll with the latest if you are on the
  last line when new content is streamed in.

- if scrolled at the bottom of the message area, track the bottom
  as new output is streamed
- tool calls: read/write/edit/bash/grep/glob
- add tool definitions
- add default AGENTS.md and ability to pass in your own.
- user input history
- switching URLs/models (and make it configurable?) and test out
  GLM 5 + Kimi K2
- Ctrl + J for input (newline) and expanding input box, also
  up/down arrow to move up/down input BUT at top/bottom instead
  goes up/down user input history.
- parse/render tables!
- Esc/Ctrl + C should warn if LLM is responding and pressing
  again should close the connection/stop receiving the request..
  if no LLM responding then exits as usual.
- allow config of LLMs/urls/api keys etc + switching
- draw just the lines that can be seen to keep consistent 60 fps

---

## Name Origin

The name *void* draws from primordial mythology—the space of infinite potential before creation.
In this sense, a language model is much the same: vast knowledge compressed into emergent
possibilities, a frontier of what's achievable. Void captures that sense of reaching into
something primal and powerful to pull out what's needed.
