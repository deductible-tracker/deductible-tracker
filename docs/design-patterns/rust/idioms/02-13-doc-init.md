# 2.13 Easy doc initialization

Description

Wrap complex example construction in helper functions to keep documentation concise and compilable without repeating heavy boilerplate.

Advantages

- Shorter examples; keeps `no_run` and `#` boilerplate minimal.

Disadvantages

- Code inside helper functions is not executed as part of the example tests unless explicitly invoked.
